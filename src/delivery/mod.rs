//! Finite Vector workload verification and uncalibrated batch timing.
//! A duration is accepted only after full verification and cleanup. Receivers do not depend on
//! Vector's transport envelopes beyond the explicitly selected output format.

mod calibration;
mod environment;
mod fixture;
mod receiver;

pub use calibration::run as calibrate;

use std::{
    error::Error,
    fmt, fs,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};

use crate::{
    cli::{CheckArgs, CheckCase},
    runtime::{check_file, sha256, Invocation, Runtime, INTERRUPTED},
};
use fixture::Fixture;
use receiver::{Receiver, Transport};

const WARMUP: usize = 1_000;
const POLL: Duration = Duration::from_millis(5);
const LOG_LIMIT: usize = 1_048_576;

#[derive(Debug)]
struct Stop(&'static str, String);

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

impl Error for Stop {}

fn stopped(outcome: &'static str, reason: impl Into<String>) -> anyhow::Error {
    Stop(outcome, reason.into()).into()
}

fn budget(deadline: Instant) -> Result<()> {
    if INTERRUPTED.load(Ordering::SeqCst) {
        return Err(stopped("interrupted", "check interrupted"));
    }
    if Instant::now() >= deadline {
        return Err(stopped("timed_out", "case deadline expired"));
    }
    Ok(())
}

struct Target {
    child: Child,
    status: Option<ExitStatus>,
}

impl Target {
    fn spawn(
        runtime: &Runtime,
        invocation: &Invocation,
        output: &Path,
        prefix: &str,
    ) -> Result<Self> {
        let mut command = runtime.command(invocation);
        command
            .stdin(Stdio::null())
            .stdout(fs::File::create(
                output.join(format!("{prefix}.stdout.log")),
            )?)
            .stderr(fs::File::create(
                output.join(format!("{prefix}.stderr.log")),
            )?);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        Ok(Self {
            child: command.spawn().context("start Vector process")?,
            status: None,
        })
    }

    fn status(&mut self) -> Result<Option<ExitStatus>> {
        if self.status.is_none() {
            self.status = self.child.try_wait()?;
        }
        Ok(self.status)
    }

    fn running(&mut self) -> Result<()> {
        if let Some(status) = self.status()? {
            return Err(stopped(
                "failed",
                format!("Vector exited unexpectedly: {status}"),
            ));
        }
        Ok(())
    }

    fn signal(&self, signal: &str) -> Result<bool> {
        Ok(Command::new("/bin/kill")
            .args([
                &format!("-{signal}"),
                "--",
                &format!("-{}", self.child.id()),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?
            .success())
    }

    fn stop(&mut self, deadline: Instant) -> Result<Value> {
        let already_exited = self.status()?.is_some();
        let signaled = !already_exited && self.signal("TERM")?;
        while self.status()?.is_none()
            && Instant::now() < deadline
            && !INTERRUPTED.load(Ordering::SeqCst)
        {
            thread::sleep(POLL);
        }
        let forced = self.status()?.is_none();
        if forced {
            let _ = self.signal("KILL");
            let _ = self.child.kill();
            self.status = Some(self.child.wait()?);
        }
        let status = self.status.context("missing process status")?;
        Ok(json!({
            "pid": self.child.id(),
            "exit_code": status.code(),
            "unexpected_exit": already_exited,
            "forced": forced,
            "graceful": signaled && !forced && status.success(),
        }))
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.signal("KILL");
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[derive(Default)]
struct Logs {
    events: Option<u64>,
    errors: Vec<String>,
}

fn logs(output: &Path) -> Result<Logs> {
    let mut result = Logs::default();
    for name in ["vector.stdout.log", "vector.stderr.log"] {
        let path = output.join(name);
        ensure!(
            fs::metadata(&path)?.len() <= LOG_LIMIT as u64,
            "Vector log exceeds 1 MiB"
        );
        let bytes = fs::read(path)?;
        for line in bytes.split_inclusive(|byte| *byte == b'\n') {
            if !line.ends_with(b"\n") {
                continue;
            }
            let Ok(value) = serde_json::from_slice::<Value>(line) else {
                continue;
            };
            let fields = value.get("fields").unwrap_or(&value);
            if value["level"] == "ERROR" && result.errors.len() < 8 {
                result.errors.push(
                    fields["message"]
                        .as_str()
                        .unwrap_or("Vector logged an error")
                        .to_owned(),
                );
            }
            if fields["message"] == "Collected events."
                && value["target"]
                    .as_str()
                    .is_some_and(|target| target.ends_with("blackhole::sink"))
            {
                let count = fields["events"]
                    .as_u64()
                    .context("invalid blackhole event counter")?;
                ensure!(
                    result.events.is_none_or(|previous| count >= previous),
                    "blackhole counter decreased"
                );
                result.events = Some(count);
            }
        }
    }
    Ok(result)
}

fn observe(
    target: &mut Target,
    receiver: Option<&Receiver>,
    log: &Path,
    deadline: Instant,
) -> Result<usize> {
    budget(deadline)?;
    target.running()?;
    let observed = logs(log)?;
    if !observed.errors.is_empty() {
        return Err(stopped("failed", observed.errors.join("; ")));
    }
    if let Some(receiver) = receiver {
        if receiver.has_errors() {
            return Err(stopped(
                "failed",
                "receiver detected an invalid, duplicate or unexpected record",
            ));
        }
        Ok(receiver.unique())
    } else {
        Ok(observed.events.unwrap_or(0) as usize)
    }
}

fn wait_for(
    target: &mut Target,
    receiver: Option<&Receiver>,
    log: &Path,
    count: usize,
    deadline: Instant,
) -> Result<()> {
    loop {
        let seen = observe(target, receiver, log, deadline)?;
        if seen > count {
            return Err(stopped(
                "failed",
                "received records before their input phase was released",
            ));
        }
        if seen == count {
            return Ok(());
        }
        thread::sleep(POLL);
    }
}

enum Input {
    File(fs::File, usize),
    Tcp(TcpStream, usize),
}

impl Write for Input {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::File(file, _) => file.write(bytes),
            Self::Tcp(stream, _) => stream.write(bytes),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::File(file, _) => file.flush(),
            Self::Tcp(stream, _) => stream.flush(),
        }
    }
}

/// Shared finite publisher for both direct calibration and Vector workloads.
fn publish(
    path: &Path,
    output: &mut impl Write,
    chunk_bytes: usize,
    published: &mut u64,
    deadline: Instant,
    mut observe: impl FnMut() -> Result<()>,
) -> Result<()> {
    let mut input = fs::File::open(path)?;
    let mut chunk = vec![0; chunk_bytes];
    let mut next_observation = Instant::now();
    loop {
        let length = input.read(&mut chunk)?;
        if length == 0 {
            return Ok(());
        }
        let mut sent = 0;
        while sent < length {
            budget(deadline)?;
            if Instant::now() >= next_observation {
                observe()?;
                next_observation = Instant::now() + POLL;
            }
            match output.write(&chunk[sent..length]) {
                Ok(0) => bail!("input writer made no progress"),
                Ok(length) => {
                    sent += length;
                    *published += length as u64;
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error).context("publish fixture input"),
            }
        }
    }
}

impl Input {
    fn send(
        &mut self,
        path: &Path,
        target: &mut Target,
        receiver: Option<&Receiver>,
        log: &Path,
        deadline: Instant,
        published: &mut u64,
    ) -> Result<()> {
        let chunk_bytes = match self {
            Self::File(_, chunk_bytes) | Self::Tcp(_, chunk_bytes) => *chunk_bytes,
        };
        publish(path, self, chunk_bytes, published, deadline, || {
            observe(target, receiver, log, deadline).map(|_| ())
        })
    }
}

fn connect(
    address: SocketAddr,
    target: &mut Target,
    log: &Path,
    deadline: Instant,
) -> Result<TcpStream> {
    loop {
        observe(target, None, log, deadline)?;
        match TcpStream::connect_timeout(&address, Duration::from_millis(50)) {
            Ok(stream) => {
                stream.set_write_timeout(Some(Duration::from_millis(50)))?;
                stream.set_nodelay(true)?;
                return Ok(stream);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(POLL)
            }
            Err(error) => return Err(error).context("connect to Vector source"),
        }
    }
}

fn render(
    template: &str,
    output: &Path,
    source: SocketAddr,
    receiver: SocketAddr,
) -> Result<String> {
    let mut result = template.to_owned();
    for (key, value) in [
        (
            "data_dir",
            output.join("data").to_string_lossy().into_owned(),
        ),
        (
            "input_file",
            output
                .join("input/input.log")
                .to_string_lossy()
                .into_owned(),
        ),
        ("source_address", source.to_string()),
        ("receiver_address", receiver.to_string()),
        ("receiver_uri", format!("http://{receiver}/")),
    ] {
        result = result.replace(&format!("{{{{ {key} }}}}"), &serde_json::to_string(&value)?);
    }
    ensure!(
        !result.contains("{{"),
        "unresolved workload config template"
    );
    Ok(result)
}

fn save(output: &Path, report: &Value) -> Result<()> {
    let temporary = output.join("result.json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(report)?)?;
    fs::rename(temporary, output.join("result.json"))?;
    Ok(())
}

fn phase(output: &Path, report: &mut Value, name: &str) -> Result<()> {
    report["phase"] = json!(name);
    save(output, report)
}

fn execute(
    runtime: &Runtime,
    args: &CheckArgs,
    binary: &Path,
    output: &Path,
    template: &str,
    report: &mut Value,
    deadline: Instant,
) -> Result<()> {
    let token = report["run_id"]
        .as_str()
        .context("missing run identifier")?
        .to_owned();
    let mut fixture = Fixture::new(args.delivery.events as usize, WARMUP, token);
    if !matches!(args.case, CheckCase::TcpToBlackhole) {
        fixture.prepare_text(|| budget(deadline))?;
    }
    for dir in ["input", "fixtures", "data"] {
        fs::create_dir(output.join(dir))?;
    }
    let mut inputs = Vec::new();
    for (name, range) in [
        ("canary", 0..1),
        ("warmup", 1..WARMUP + 1),
        ("measured", WARMUP + 1..fixture.total()),
    ] {
        let path = output.join("fixtures").join(format!("{name}.log"));
        fixture.write(&path, range, || budget(deadline))?;
        inputs.push(path.clone());
        report["inputs"][name] =
            json!({"path":path,"sha256":sha256(&path)?,"bytes":fs::metadata(&path)?.len()});
    }
    report["fixture"] = json!({"version":"apache-records-v1","token":fixture.token,"canary":1,"warmup":WARMUP,"measured":fixture.events,"expected_total":fixture.total()});
    report["fixture"]["verification_reference_bytes"] = json!(fixture.cache_bytes());
    let receiver = match args.case {
        CheckCase::TcpToBlackhole => None,
        case => Some(Receiver::start_with_reads(
            fixture.clone(),
            match case {
                CheckCase::RegexParsing => Transport::ParsedJson,
                CheckCase::TcpToHttp => Transport::HttpJson,
                _ => Transport::Text,
            },
            args.delivery.receiver_reads_per_pass as usize,
        )?),
    };
    let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let source_address = reservation.local_addr()?;
    let receiver_address = SocketAddr::from((
        Ipv4Addr::LOCALHOST,
        receiver.as_ref().map_or(1, Receiver::port),
    ));
    let config = output.join("vector.yaml");
    budget(deadline)?;
    fs::write(
        &config,
        render(template, output, source_address, receiver_address)?,
    )?;
    fs::copy(&inputs[0], output.join("input/input.log"))?;
    report["config_sha256"] = json!(sha256(&config)?);

    phase(output, report, "version")?;
    budget(deadline)?;
    let version_spec = Invocation::new(&binary.to_string_lossy(), output).args(["--version"]);
    let mut version = Target::spawn(runtime, &version_spec, output, "version")?;
    while version.status()?.is_none() {
        budget(deadline)?;
        thread::sleep(POLL);
    }
    ensure!(version.status.unwrap().success(), "Vector --version failed");
    ensure!(
        fs::metadata(output.join("version.stdout.log"))?.len() <= 16 * 1024,
        "Vector version output exceeds 16 KiB"
    );
    let version_text = fs::read_to_string(output.join("version.stdout.log"))?;
    ensure!(
        version_text.starts_with("vector "),
        "supplied binary did not identify itself as Vector"
    );
    report["subject"]["version"] = json!(version_text.trim());
    drop(version);

    let launch = if let Some(cpus) = &args.vector_cpus {
        Invocation::new("taskset", output).args([
            "--cpu-list".to_owned(),
            cpus.clone(),
            binary.to_string_lossy().into_owned(),
        ])
    } else {
        Invocation::new(&binary.to_string_lossy(), output)
    };
    let mut invocation = launch
        .args([
            "--config".to_owned(),
            config.to_string_lossy().into_owned(),
            "--log-format".into(),
            "json".into(),
            "--color".into(),
            "never".into(),
            "--threads".into(),
            "2".into(),
            "--require-healthy".into(),
            "true".into(),
            "--graceful-shutdown-limit-secs".into(),
            args.timeout_secs.to_string(),
        ])
        .env("VECTOR_LOG", "info");
    invocation.unset_env = [
        "VECTOR_CONFIG",
        "VECTOR_CONFIG_DIR",
        "VECTOR_CONFIG_TOML",
        "VECTOR_CONFIG_JSON",
        "VECTOR_CONFIG_YAML",
        "VECTOR_WATCH_CONFIG",
        "VECTOR_NO_GRACEFUL_SHUTDOWN_LIMIT",
        "RUST_LOG",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    report["invocation"] = serde_json::to_value(&invocation)?;
    phase(output, report, "startup")?;
    drop(reservation);
    let mut target = Target::spawn(runtime, &invocation, output, "vector")?;
    report["process"] = json!({"pid":target.child.id(),"state":"running"});
    save(output, report)?;
    let log = output.to_path_buf();
    let mut published = [0_u64; 3];
    let work = (|| -> Result<()> {
        let mut input = if matches!(args.case, CheckCase::FileToTcp) {
            published[0] = fs::metadata(&inputs[0])?.len();
            Input::File(
                fs::OpenOptions::new()
                    .append(true)
                    .open(output.join("input/input.log"))?,
                args.delivery.producer_chunk_bytes as usize,
            )
        } else {
            let mut input = Input::Tcp(
                connect(source_address, &mut target, &log, deadline)?,
                args.delivery.producer_chunk_bytes as usize,
            );
            input.send(
                &inputs[0],
                &mut target,
                receiver.as_ref(),
                &log,
                deadline,
                &mut published[0],
            )?;
            input
        };
        phase(output, report, "canary")?;
        wait_for(&mut target, receiver.as_ref(), &log, 1, deadline)?;
        phase(output, report, "warmup")?;
        input.send(
            &inputs[1],
            &mut target,
            receiver.as_ref(),
            &log,
            deadline,
            &mut published[1],
        )?;
        wait_for(&mut target, receiver.as_ref(), &log, WARMUP + 1, deadline)?;
        report["cpu_placement"] =
            environment::placement(args.vector_cpus.as_deref(), target.child.id())?;
        phase(output, report, "measured_input")?;
        if let Some(receiver) = &receiver {
            receiver.start_measurement().map_err(|error| {
                stopped("failed", format!("cannot start measured input: {error:#}"))
            })?;
        }
        input.send(
            &inputs[2],
            &mut target,
            receiver.as_ref(),
            &log,
            deadline,
            &mut published[2],
        )?;
        wait_for(
            &mut target,
            receiver.as_ref(),
            &log,
            fixture.total(),
            deadline,
        )?;
        Ok(())
    })();
    report["published_input_bytes"] =
        json!({"canary":published[0],"warmup":published[1],"measured":published[2]});
    report["verification_phase"] = report["phase"].clone();
    phase(output, report, "shutdown")?;
    // Even a failed verification reaps Vector before the receiver is drained.
    let cleanup_deadline = if work.is_ok() {
        deadline
    } else {
        deadline.min(Instant::now() + Duration::from_secs(2))
    };
    let cleanup = target.stop(cleanup_deadline);
    // Drop guarantees a kill/reap if the explicit shutdown encountered an I/O error.
    drop(target);
    let mut finalization_errors = Vec::new();
    report["process"] = match cleanup {
        Ok(value) => value,
        Err(error) => {
            finalization_errors.push(format!("process cleanup: {error:#}"));
            json!({"graceful":false,"error":error.to_string()})
        }
    };
    let final_logs = match logs(&log) {
        Ok(logs) => Some(logs),
        Err(error) => {
            finalization_errors.push(format!("log inspection: {error:#}"));
            None
        }
    };
    // Drain and retain receiver diagnostics even when log inspection failed.
    report["verification"] = match receiver {
        Some(receiver) => match receiver.finish() {
            Ok(value) => value,
            Err(error) => {
                finalization_errors.push(format!("receiver drain: {error:#}"));
                json!({"passed":false,"error":error.to_string()})
            }
        },
        None => {
            let count = final_logs.as_ref().and_then(|logs| logs.events);
            json!({
                "kind":"terminal_count_only", "passed":count == Some(fixture.total() as u64),
                "expected":fixture.total(), "processed":count,
                "identities_verified":false, "payloads_verified":false,
            })
        }
    };
    let vector_errors = final_logs.map_or_else(Vec::new, |logs| logs.errors);
    report["vector_errors"] = json!(vector_errors);
    report["finalization_errors"] = json!(finalization_errors);
    // A cleanup/logging problem must not erase the primary verification failure.
    work?;
    ensure!(
        finalization_errors.is_empty(),
        "{}",
        finalization_errors.join("; ")
    );
    budget(deadline)?;
    if report["process"]["graceful"] != true {
        return Err(stopped("failed", "Vector did not shut down gracefully"));
    }
    if report["verification"]["passed"] != true || !vector_errors.is_empty() {
        return Err(stopped("failed", "final workload verification failed"));
    }
    if !matches!(args.case, CheckCase::TcpToBlackhole) {
        ensure!(
            report["verification"]["measurement"]["seconds"]
                .as_f64()
                .is_some_and(|seconds| seconds.is_finite() && seconds > 0.0),
            "verified workload has no complete measurement interval"
        );
    }
    Ok(())
}

fn finish_measurement(report: &mut Value, case: CheckCase, passed: bool) {
    let candidate = report["verification"]
        .as_object_mut()
        .and_then(|verification| verification.remove("measurement"));
    let mut measurement = candidate.unwrap_or_else(|| {
        json!({
            "status":"unavailable", "seconds":null, "calibrated":false,
            "method":"input-release-to-receiver-verification-v1",
        })
    });
    measurement["measured_events"] = report["fixture"]["measured"].clone();
    measurement["input_bytes"] = report["inputs"]["measured"]["bytes"].clone();
    measurement["calibrated"] = json!(false);
    if matches!(case, CheckCase::TcpToBlackhole) {
        measurement["status"] = json!("unavailable");
        measurement["seconds"] = Value::Null;
        measurement["reason"] =
            json!("blackhole verifies a sampled log counter, not receiver completion time");
    } else if passed
        && measurement["seconds"]
            .as_f64()
            .is_some_and(|seconds| seconds.is_finite() && seconds > 0.0)
    {
        measurement["status"] = json!("verified");
    } else {
        measurement["status"] = json!("invalid");
        measurement["seconds"] = Value::Null;
        measurement["invalidated_by"] = report["outcome"].clone();
    }
    report["measurement"] = measurement;
}

pub fn run(runtime: &Runtime, args: CheckArgs) -> Result<i32> {
    ensure!(cfg!(unix), "workload checks currently require a Unix host");
    ensure!(
        args.vector_cpus.is_none() || cfg!(target_os = "linux"),
        "--vector-cpus currently requires Linux and taskset"
    );
    ensure!(
        !matches!(args.case, CheckCase::TcpToHttp | CheckCase::TcpToBlackhole)
            || args.delivery.receiver_reads_per_pass == 16,
        "--receiver-reads-per-pass applies only to TCP receivers"
    );
    let binary = runtime.path(&args.vector_bin)?;
    check_file(&binary)?;
    let source = runtime
        .root
        .join("cases")
        .join(args.case.name())
        .join("vector.yaml");
    let template = fs::read_to_string(&source).context("read workload config")?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let output = args
        .output
        .as_ref()
        .map(|path| runtime.path(path))
        .transpose()?
        .unwrap_or_else(|| {
            runtime
                .root
                .join(".cache/runs")
                .join(format!("check-{}-{nonce}", args.case.name()))
        });
    ensure!(
        !output.exists(),
        "output directory already exists: {}",
        output.display()
    );
    render(
        &template,
        &output,
        SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 2)),
    )?;
    if runtime.dry_run {
        println!(
            "{}",
            json!({"action":"verify_workload","case":args.case.name(),"subject":"vector","binary":binary,"output":output,"config_source":source,"delivery":args.delivery,"vector_cpus":args.vector_cpus,"warmup":WARMUP,"canary":1,"performance_result":false})
        );
        return Ok(0);
    }
    let binary_sha256 = sha256(&binary)?;
    let config_sha256 = sha256(&source)?;
    let runner_sha256 = sha256(&std::env::current_exe()?)?;
    fs::create_dir_all(output.parent().context("output has no parent")?)?;
    fs::create_dir(&output)?;
    println!("{}", output.display());
    let started = Instant::now();
    let deadline = started + Duration::from_secs(args.timeout_secs);
    let mut report = json!({
        "schema_version":1,"case":args.case.name(),"outcome":"running","passed":null,
        "phase":"setup","purpose":"workload_verification","performance_result":false,
        "timeout_secs":args.timeout_secs,"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,
        "run_id":format!("{nonce:032x}"),
        "subject":{"name":"vector","binary":binary,"sha256":binary_sha256},
        "runner":{"version":env!("CARGO_PKG_VERSION"),"sha256":runner_sha256},
        "config_source":source,"config_source_sha256":config_sha256,"inputs":{},
        "delivery":args.delivery,
        "environment":environment::capture(),
        "cpu_placement":{"requested_vector_cpus":args.vector_cpus,"observed_vector_cpus":null,"observed_harness_cpus":environment::affinity(std::process::id()),"exclusive_reservation":false},
        "scope":"same-host finite batch; optional CPU affinity is not exclusive reservation; no sustainable-capacity claim",
        "input_storage":"freshly written fixture files; filesystem cache not evicted",
        "producer_observation_interval_ms":POLL.as_millis(),
    });
    save(&output, &report)?;
    let result = execute(
        runtime,
        &args,
        &binary,
        &output,
        &template,
        &mut report,
        deadline,
    );
    let (outcome, reason, code) = match result {
        Ok(()) => ("passed", None, 0),
        Err(error) => {
            let outcome = error
                .downcast_ref::<Stop>()
                .map_or("errored", |stop| stop.0);
            (
                outcome,
                Some(format!("{error:#}")),
                if outcome == "failed" { 1 } else { 2 },
            )
        }
    };
    report["outcome"] = json!(outcome);
    report["passed"] = json!(code == 0);
    report["reason"] = json!(reason);
    finish_measurement(&mut report, args.case, code == 0);
    report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    save(&output, &report)?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_successful_runs_keep_receiver_completion_times() {
        for outcome in ["passed", "failed", "timed_out", "interrupted", "errored"] {
            let mut report = json!({
                "outcome":outcome, "performance_result":false,
                "inputs":{"measured":{"bytes":1234}},
                "verification":{"passed":true,"measurement":{
                    "method":"input-release-to-receiver-verification-v1",
                    "status":"candidate","seconds":0.25,"calibrated":false,
                }},
            });
            finish_measurement(&mut report, CheckCase::FileToTcp, outcome == "passed");
            assert!(report["verification"].get("measurement").is_none());
            assert_eq!(report["measurement"]["calibrated"], false);
            assert_eq!(report["performance_result"], false);
            if outcome == "passed" {
                assert_eq!(report["measurement"]["status"], "verified");
                assert_eq!(report["measurement"]["seconds"], 0.25);
            } else {
                assert_eq!(report["measurement"]["status"], "invalid");
                assert!(report["measurement"]["seconds"].is_null());
            }
        }
        let mut blackhole = json!({"outcome":"passed","inputs":{}});
        finish_measurement(&mut blackhole, CheckCase::TcpToBlackhole, true);
        assert_eq!(blackhole["measurement"]["status"], "unavailable");
        assert!(blackhole["measurement"]["seconds"].is_null());
    }

    #[test]
    fn reads_processed_counters_and_errors_from_both_log_streams() {
        let dir = tempfile::tempdir().unwrap();
        let counter = |count: Value| {
            format!(
                "{}\n",
                json!({
                    "level":"INFO", "target":"vector::sinks::blackhole::sink",
                    "message":"Collected events.", "events":count,
                })
            )
        };
        fs::write(dir.path().join("vector.stdout.log"), counter(json!(11_001))).unwrap();
        fs::write(
            dir.path().join("vector.stderr.log"),
            "{\"level\":\"ERROR\",\"message\":\"processing failed\"}\n",
        )
        .unwrap();
        let observed = logs(dir.path()).unwrap();
        assert_eq!(observed.events, Some(11_001));
        assert_eq!(observed.errors, ["processing failed"]);

        fs::write(
            dir.path().join("vector.stdout.log"),
            counter(json!("invalid")),
        )
        .unwrap();
        assert!(logs(dir.path()).is_err());
        fs::write(
            dir.path().join("vector.stdout.log"),
            counter(json!(2)) + &counter(json!(1)),
        )
        .unwrap();
        assert!(logs(dir.path()).is_err());
    }
}

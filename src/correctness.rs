//! Small local end-to-end checks. Vector unit tests still own transform-only coverage.
use std::{
    error::Error,
    fmt, fs,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    cli::{CheckArgs, CheckCase},
    runtime::{check_file, sha256, status_code, Invocation, Runtime, INTERRUPTED},
};

const POLL: Duration = Duration::from_millis(20);
const CAPTURE_LIMIT: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Passed,
    Failed,
    TimedOut,
    Errored,
    Interrupted,
}

#[derive(Debug)]
struct Stop(Outcome, String);
impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.1)
    }
}
impl Error for Stop {}

#[derive(Clone, Serialize)]
struct Expected {
    receiver: &'static str,
    fields: Value,
    absent: Vec<&'static str>,
}

#[derive(Serialize)]
struct Received {
    receiver: &'static str,
    event: Value,
    raw: String,
}

struct Connection {
    stream: TcpStream,
    pending: Vec<u8>,
}

struct Collector {
    name: &'static str,
    listener: TcpListener,
    connections: Vec<Connection>,
    raw: fs::File,
    bytes: usize,
    received: Vec<Received>,
}

impl Collector {
    fn bind(name: &'static str, output: &Path) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            name,
            listener,
            connections: vec![],
            raw: fs::File::create(output.join(format!("{name}.raw")))?,
            bytes: 0,
            received: vec![],
        })
    }

    fn poll(&mut self) -> Result<()> {
        // Bound work per poll so a noisy target cannot starve the case deadline.
        for _ in 0..16 {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    ensure!(
                        self.connections.len() < 64,
                        "too many collector connections"
                    );
                    stream.set_nonblocking(true)?;
                    self.connections.push(Connection {
                        stream,
                        pending: vec![],
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
        for index in (0..self.connections.len()).rev() {
            let mut closed = false;
            for _ in 0..16 {
                let mut buffer = [0; 8192];
                match self.connections[index].stream.read(&mut buffer) {
                    Ok(0) => {
                        closed = true;
                        break;
                    }
                    Ok(n) => {
                        self.bytes += n;
                        ensure!(
                            self.bytes <= CAPTURE_LIMIT,
                            "collector capture exceeds 1 MiB"
                        );
                        self.raw.write_all(&buffer[..n])?;
                        self.connections[index]
                            .pending
                            .extend_from_slice(&buffer[..n]);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {
                        closed = true;
                        break;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            while let Some(end) = self.connections[index]
                .pending
                .iter()
                .position(|b| *b == b'\n')
            {
                let line: Vec<_> = self.connections[index].pending.drain(..=end).collect();
                self.record(&line);
            }
            if closed {
                let remaining = std::mem::take(&mut self.connections[index].pending);
                if !remaining.is_empty() {
                    self.record(&remaining);
                }
                self.connections.swap_remove(index);
            }
        }
        Ok(())
    }

    fn record(&mut self, bytes: &[u8]) {
        let raw = String::from_utf8_lossy(bytes).into_owned();
        let event = serde_json::from_slice(bytes).unwrap_or_else(|_| Value::String(raw.clone()));
        self.received.push(Received {
            receiver: self.name,
            event,
            raw,
        });
    }
}

struct Target {
    child: Child,
    status: Option<ExitStatus>,
    forced: bool,
}

impl Target {
    fn status(&mut self) -> Result<Option<ExitStatus>> {
        if self.status.is_none() {
            self.status = self.child.try_wait()?;
        }
        Ok(self.status)
    }

    fn signal(&self, signal: &str) -> Result<()> {
        // HUP targets Vector itself; termination also reclaims its process group.
        let pid = if signal == "HUP" {
            self.child.id().to_string()
        } else {
            format!("-{}", self.child.id())
        };
        let status = Command::new("/bin/kill")
            .args([&format!("-{signal}"), "--", &pid])
            .stdin(Stdio::null())
            .status()?;
        ensure!(status.success(), "could not send SIG{signal} to Vector");
        Ok(())
    }

    fn kill(&mut self) -> Result<()> {
        if self.status()?.is_none() {
            self.forced = true;
            let _ = self.signal("KILL");
            let _ = self.child.kill();
            self.status = Some(self.child.wait()?);
        }
        Ok(())
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

struct Run {
    output: PathBuf,
    deadline: Instant,
    phase: &'static str,
    report: Value,
    collectors: Vec<Collector>,
    target: Option<Target>,
    sent: usize,
}

impl Run {
    fn phase(&mut self, phase: &'static str) -> Result<()> {
        self.phase = phase;
        self.report["phase"] = json!(phase);
        self.report["sent"] = json!(self.sent);
        write_json(&self.output.join("result.json"), &self.report)
    }

    fn poll(&mut self) -> Result<()> {
        for collector in &mut self.collectors {
            collector.poll()?;
        }
        Ok(())
    }

    fn wait(&mut self, mut ready: impl FnMut(&Self) -> Result<bool>) -> Result<()> {
        loop {
            if INTERRUPTED.load(Ordering::SeqCst) {
                return Err(Stop(Outcome::Interrupted, "check interrupted".into()).into());
            }
            if Instant::now() >= self.deadline {
                return Err(Stop(
                    Outcome::TimedOut,
                    format!("case deadline expired during {}", self.phase),
                )
                .into());
            }
            self.poll()?;
            if let Some(status) = self
                .target
                .as_mut()
                .context("Vector was not started")?
                .status()?
            {
                return Err(Stop(
                    Outcome::Failed,
                    format!("Vector exited during {} with {status}", self.phase),
                )
                .into());
            }
            if ready(self)? {
                return Ok(());
            }
            thread::sleep(POLL.min(self.deadline.saturating_duration_since(Instant::now())));
        }
    }

    fn send(&mut self, address: SocketAddr, payload: &Value) -> Result<()> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Stop(
                Outcome::TimedOut,
                "case deadline expired before sending".into(),
            )
            .into());
        }
        let mut stream =
            TcpStream::connect_timeout(&address, remaining.min(Duration::from_secs(1)))?;
        stream.set_write_timeout(Some(remaining.min(Duration::from_secs(1))))?;
        let mut bytes = serde_json::to_vec(payload)?;
        bytes.push(b'\n');
        stream.write_all(&bytes)?;
        self.sent += 1;
        Ok(())
    }

    fn finish_target(&mut self, graceful: bool) -> Result<()> {
        if let Some(target) = &mut self.target {
            if target.status()?.is_none() && graceful {
                target.signal("TERM")?;
            }
        }
        if !graceful {
            if let Some(target) = &mut self.target {
                target.kill()?;
            }
        }
        while self
            .target
            .as_mut()
            .is_some_and(|target| target.status.is_none())
        {
            self.poll()?;
            if self.target.as_mut().unwrap().status()?.is_some() {
                break;
            }
            if Instant::now() >= self.deadline || INTERRUPTED.load(Ordering::SeqCst) {
                self.target.as_mut().unwrap().kill()?;
                if INTERRUPTED.load(Ordering::SeqCst) {
                    return Err(Stop(Outcome::Interrupted, "check interrupted".into()).into());
                }
                return Err(Stop(
                    Outcome::TimedOut,
                    "Vector did not shut down within the case budget".into(),
                )
                .into());
            }
            thread::sleep(POLL);
        }
        // The target is reaped: drain its sockets before assessing the complete output.
        let drain_until = self.deadline.min(Instant::now() + Duration::from_secs(1));
        loop {
            self.poll()?;
            if self.collectors.iter().all(|c| c.connections.is_empty()) {
                return Ok(());
            }
            ensure!(
                Instant::now() < drain_until,
                "collector connections did not close after Vector exited"
            );
            thread::sleep(POLL);
        }
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn render(template: &str, source_port: u16, consumer_port: u16, output: &Path) -> Result<String> {
    ensure!(
        !template.contains("{%") && !template.lines().any(|line| line.starts_with("data_dir:")),
        "local check expects a simple port template without a global data_dir"
    );
    let mut rendered = format!(
        "data_dir: {}\n",
        serde_json::to_string(&output.join("data"))?
    );
    let mut remaining = template;
    while let Some((prefix, rest)) = remaining.split_once("{{") {
        rendered.push_str(prefix);
        let (name, rest) = rest
            .split_once("}}")
            .context("unterminated config template variable")?;
        rendered.push_str(&match name.trim() {
            "subject_port" => source_port.to_string(),
            "consumer_port" | "tcp_consumer_port" => consumer_port.to_string(),
            "consumer_ip" => "127.0.0.1".into(),
            name => bail!("unsupported local config variable {name}"),
        });
        remaining = rest;
    }
    rendered.push_str(remaining);
    Ok(rendered)
}

fn differences(expected: &[Expected], received: &[&Received]) -> Vec<String> {
    let mut differences = vec![];
    for wanted in expected {
        let actual: Vec<_> = received
            .iter()
            .filter(|event| event.receiver == wanted.receiver)
            .collect();
        if actual.len() != 1 {
            differences.push(format!(
                "{}: expected 1 event, received {}",
                wanted.receiver,
                actual.len()
            ));
        }
        for actual in actual {
            for (field, value) in wanted.fields.as_object().unwrap() {
                if actual.event.get(field) != Some(value) {
                    differences.push(format!(
                        "{}.{field}: expected {value}, received {:?}",
                        wanted.receiver,
                        actual.event.get(field)
                    ));
                }
            }
            for field in &wanted.absent {
                if actual.event.get(field).is_some() {
                    differences.push(format!("{}: unexpected field {field}", wanted.receiver));
                }
            }
        }
    }
    differences
}

pub fn run(runtime: &Runtime, args: CheckArgs) -> Result<i32> {
    ensure!(
        cfg!(unix),
        "local correctness checks currently require a Unix host"
    );
    let binary = runtime.path(&args.vector_bin)?;
    check_file(&binary)?;
    let source = runtime
        .root
        .join("cases")
        .join(args.case.name())
        .join("ansible/config_files/vector.yaml");
    let template = fs::read_to_string(&source).context("reading the existing case config")?;
    let output = args
        .output
        .as_ref()
        .map(|p| runtime.path(p))
        .transpose()?
        .unwrap_or_else(|| {
            runtime.root.join(".cache/runs").join(format!(
                "check-{}-{}",
                args.case.name(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ))
        });
    ensure!(
        !output.exists(),
        "output directory already exists: {}",
        output.display()
    );
    render(&template, 1, 2, &output)?;
    let mut invocation = Invocation::new(&binary.to_string_lossy(), &output)
        .args([
            "--config".into(),
            output.join("vector.yaml").to_string_lossy().into_owned(),
            "--log-format".into(),
            "json".into(),
            "--color".into(),
            "never".into(),
            "--require-healthy".into(),
            "true".into(),
            "--graceful-shutdown-limit-secs".into(),
            "2".into(),
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
    if runtime.dry_run {
        runtime.mkdir(&output)?;
        runtime.write(&output.join("vector.yaml"), &[])?;
        runtime.plan(&invocation)?;
        return runtime.write(&output.join("result.json"), &[]).map(|_| 0);
    }
    fs::create_dir_all(output.parent().context("output has no parent")?)?;
    fs::create_dir(&output)?;
    let started = Instant::now();
    let payloads = match args.case {
        CheckCase::WrappedJson => vec![json!({"log":"{\"message\":\"Hello world\"}"})],
        CheckCase::Sighup => vec![json!({"log":"First line"}), json!({"log":"Second line"})],
    };
    let expected = match args.case {
        CheckCase::WrappedJson => vec![Expected {
            receiver: "output",
            fields: json!({"message":"Hello world"}),
            absent: vec!["log"],
        }],
        CheckCase::Sighup => vec![
            Expected {
                receiver: "before_reload",
                fields: payloads[0].clone(),
                absent: vec![],
            },
            Expected {
                receiver: "after_reload",
                fields: payloads[1].clone(),
                absent: vec![],
            },
        ],
    };
    let report = json!({
        "schema_version": 1,
        "case": args.case.name(),
        "state": "running",
        "complete": false,
        "outcome": null,
        "phase": "setup",
        "sent": 0,
        "timeout_secs": args.timeout_secs,
        "vector_binary": binary,
        "harness_version": env!("CARGO_PKG_VERSION"),
        "config_source": source,
        "os": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
    });
    let mut run = Run {
        output: output.clone(),
        deadline: started + Duration::from_secs(args.timeout_secs),
        phase: "setup",
        report,
        collectors: vec![],
        target: None,
        sent: 0,
    };
    run.phase("setup")?;
    let procedure = (|| -> Result<()> {
        let mut input = fs::File::create(output.join("input.jsonl"))?;
        for payload in &payloads {
            writeln!(input, "{payload}")?;
        }
        write_json(&output.join("expected.json"), &expected)?;
        write_json(&output.join("command.json"), &invocation)?;
        run.report["vector_binary_sha256"] = json!(sha256(&binary)?);
        run.report["harness_binary_sha256"] = json!(sha256(&std::env::current_exe()?)?);
        run.report["config_source_sha256"] = json!(sha256(&source)?);
        fs::create_dir(output.join("data"))?;
        for wanted in &expected {
            run.collectors
                .push(Collector::bind(wanted.receiver, &output)?);
        }
        let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = reservation.local_addr()?;
        let initial = render(
            &template,
            address.port(),
            run.collectors[0].listener.local_addr()?.port(),
            &output,
        )?;
        fs::write(output.join("initial.yaml"), &initial)?;
        fs::write(output.join("vector.yaml"), initial)?;
        let mut command = runtime.command(&invocation);
        command
            .stdin(Stdio::null())
            .stdout(fs::File::create(output.join("vector.stdout.log"))?)
            .stderr(fs::File::create(output.join("vector.stderr.log"))?);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        drop(reservation);
        run.target = Some(Target {
            child: command.spawn().context("starting Vector")?,
            status: None,
            forced: false,
        });
        run.report["vector_pid"] = json!(run.target.as_ref().unwrap().child.id());
        run.phase("startup")?;
        run.wait(|_| Ok(TcpStream::connect_timeout(&address, POLL).is_ok()))?;
        run.phase("first_delivery")?;
        run.send(address, &payloads[0])?;
        run.wait(|run| Ok(!run.collectors[0].received.is_empty()))?;
        if matches!(args.case, CheckCase::Sighup) {
            let logs = ["vector.stdout.log", "vector.stderr.log"]
                .map(|name| {
                    let path = output.join(name);
                    let offset = fs::metadata(&path)?.len() as usize;
                    Ok((path, offset))
                })
                .into_iter()
                .collect::<Result<Vec<_>>>()?;
            let reloaded = render(
                &template,
                address.port(),
                run.collectors[1].listener.local_addr()?.port(),
                &output,
            )?;
            fs::write(output.join("reloaded.yaml"), &reloaded)?;
            fs::write(output.join("vector.yaml"), reloaded)?;
            run.phase("reload")?;
            run.target.as_ref().unwrap().signal("HUP")?;
            run.wait(|_| {
                for (path, offset) in &logs {
                    let bytes = fs::read(path)?;
                    if bytes
                        .get(*offset..)
                        .unwrap_or_default()
                        .split(|b| *b == b'\n')
                        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
                        .any(|event| {
                            event["fields"]["message"]
                                .as_str()
                                .or_else(|| event["message"].as_str())
                                == Some("Vector has reloaded.")
                        })
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            })?;
            run.phase("second_delivery")?;
            run.send(address, &payloads[1])?;
            run.wait(|run| {
                Ok(run
                    .collectors
                    .iter()
                    .map(|collector| collector.received.len())
                    .sum::<usize>()
                    >= payloads.len())
            })?;
        }
        Ok(())
    })();
    let reached_shutdown = procedure.is_ok();
    if reached_shutdown {
        run.phase("shutdown")?;
    }
    let cleanup = run.finish_target(reached_shutdown);
    if cleanup.is_err() {
        if let Some(target) = &mut run.target {
            target.kill()?;
        }
    }
    let received: Vec<_> = run
        .collectors
        .iter()
        .flat_map(|collector| &collector.received)
        .collect();
    let diff = differences(&expected, &received);
    let failure = procedure.err().or_else(|| cleanup.err());
    let (outcome, message) = if let Some(error) = &failure {
        (
            error
                .downcast_ref::<Stop>()
                .map_or(Outcome::Errored, |stop| stop.0),
            Some(format!("{error:#}")),
        )
    } else if !run.target.as_ref().unwrap().status.unwrap().success() || !diff.is_empty() {
        (
            Outcome::Failed,
            Some("received output or Vector exit status did not match the case".into()),
        )
    } else {
        (Outcome::Passed, None)
    };
    let mut decoded = fs::File::create(output.join("received.jsonl"))?;
    for record in &received {
        writeln!(decoded, "{}", serde_json::to_string(record)?)?;
    }
    run.report["state"] = json!("finished");
    run.report["complete"] = json!(failure.is_none());
    run.report["outcome"] = json!(outcome);
    run.report["sent"] = json!(run.sent);
    run.report["received"] = json!(received.len());
    run.report["differences"] = json!(diff);
    run.report["error"] = json!(message);
    run.report["duration_seconds"] = json!(started.elapsed().as_secs_f64());
    run.report["cleanup"] = match &run.target {
        Some(target) => {
            json!({"vector_reaped":target.status.is_some(),"forced":target.forced,"exit_code":target.status.map(status_code)})
        }
        None => json!({"vector_reaped":true,"forced":false,"exit_code":null}),
    };
    write_json(&output.join("result.json"), &run.report)?;
    println!("{}", output.join("result.json").display());
    if let Some(message) = message {
        eprintln!("{}: {message}", args.case.name());
    }
    Ok(match outcome {
        Outcome::Passed => 0,
        Outcome::Failed => 1,
        _ => 2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_counts_do_not_hide_wrong_records_or_routes() {
        let expected = [
            Expected {
                receiver: "first",
                fields: json!({"id":1}),
                absent: vec![],
            },
            Expected {
                receiver: "second",
                fields: json!({"id":2}),
                absent: vec![],
            },
        ];
        let first = Received {
            receiver: "first",
            event: json!({"id":1}),
            raw: String::new(),
        };
        let duplicate = Received {
            receiver: "first",
            event: json!({"id":1}),
            raw: String::new(),
        };
        assert_eq!(differences(&expected, &[&first, &duplicate]).len(), 2);
        let misrouted = Received {
            receiver: "second",
            event: json!({"id":1}),
            raw: String::new(),
        };
        assert_eq!(differences(&expected, &[&first, &misrouted]).len(), 1);
    }
}

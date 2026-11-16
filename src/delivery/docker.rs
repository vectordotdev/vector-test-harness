//! A real Docker fragment fixture; reassembly belongs to the subject.

use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    adapters, budget, fixture::Format, logs_final, observe, phase, receiver::Receiver, render,
    save, stopped, subject::Subject, Target, LOG_LIMIT, POLL,
};
use crate::{
    cli::CheckArgs,
    runtime::{sha256, Invocation, Runtime},
};

pub(super) fn name(output: &Path) -> String {
    format!(
        "vth-{:x}",
        Sha256::digest(output.as_os_str().as_encoded_bytes())
    )[..28]
        .to_owned()
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn vector_lifecycle(output: &Path, container_id: &str) -> Result<(u64, u64, bool)> {
    let (mut started, mut stopped, mut died) = (0, 0, false);
    for suffix in ["stdout", "stderr"] {
        let path = output.join(format!("vector.{suffix}.log"));
        ensure!(
            fs::metadata(&path)?.len() <= LOG_LIMIT as u64,
            "subject log exceeds 1 MiB"
        );
        for line in fs::read(path)?.split_inclusive(|byte| *byte == b'\n') {
            if !line.ends_with(b"\n") {
                continue;
            }
            let Ok(event) = serde_json::from_slice::<Value>(line) else {
                continue;
            };
            let fields = event.get("fields").unwrap_or(&event);
            let from_input = |span: &Value| {
                span["component_id"] == "input"
                    && span["component_kind"] == "source"
                    && span["component_type"] == "docker_logs"
            };
            if event["target"] != "vector::internal_events::docker_logs"
                || fields["container_id"] != container_id
                || !(from_input(&event["span"])
                    || event["spans"]
                        .as_array()
                        .is_some_and(|spans| spans.iter().any(from_input)))
            {
                continue;
            }
            match fields["message"].as_str() {
                Some("Started watching for container logs.") => started += 1,
                Some("Stopped watching for container logs.") => stopped += 1,
                Some("Received one container event.") if fields["action"] == "die" => died = true,
                _ => {}
            }
        }
    }
    Ok((started, stopped, died))
}

fn ready(output: &Path, subject: Subject, container_id: &str) -> Result<bool> {
    if subject == Subject::Vector {
        return Ok(vector_lifecycle(output, container_id)?.0 > 0);
    }
    if matches!(subject, Subject::SplunkHf | Subject::SplunkUf) {
        // The retained JSON-file input is read from head even if discovery is
        // later than producer exit. Health remains checked until delivery.
        return Ok(adapters::splunk::extra_logs(output)
            .first()
            .is_some_and(|path| fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0)));
    }
    for suffix in ["stdout", "stderr"] {
        let path = output.join(format!("{}.{suffix}.log", subject.name()));
        ensure!(
            fs::metadata(&path)?.len() <= LOG_LIMIT as u64,
            "subject log exceeds 1 MiB"
        );
        for line in fs::read_to_string(path)?.lines() {
            if (subject == Subject::FluentBit && line.contains("[engine] Shutdown Grace Period="))
                || (subject == Subject::Telegraf && line.contains("[agent] Config: Interval:"))
            {
                return Ok(true);
            }
            let Ok(event) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let message = event["fields"]["message"]
                .as_str()
                .or_else(|| event["logEvent"]["message"].as_str())
                .or_else(|| event["message"].as_str());
            let expected = match subject {
                Subject::Filebeat => "filebeat start running.",
                Subject::Fluentd => "fluentd worker is now running worker=0",
                Subject::Logstash => "Pipeline started",
                _ => continue,
            };
            if message == Some(expected) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn docker(output: &Path, label: &str, args: &[&str], deadline: Instant) -> Result<Vec<u8>> {
    let stdout = output.join(format!("docker-{label}.stdout.log"));
    let stderr = output.join(format!("docker-{label}.stderr.log"));
    let mut process = Running(
        Command::new("docker")
            .args(args)
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout)?)
            .stderr(fs::File::create(&stderr)?)
            .spawn()
            .context("start Docker CLI")?,
    );
    let until = deadline.min(Instant::now() + Duration::from_secs(30));
    loop {
        // Cleanup must still run after interruption, using its separate bound.
        if !matches!(label, "owned" | "remove") {
            budget(deadline)?;
        }
        ensure!(
            fs::metadata(&stdout)?.len() <= LOG_LIMIT as u64
                && fs::metadata(&stderr)?.len() <= LOG_LIMIT as u64,
            "Docker CLI output exceeds 1 MiB"
        );
        if let Some(status) = process.0.try_wait()? {
            ensure!(
                status.success(),
                "Docker {label} failed; inspect {}",
                stderr.display()
            );
            return Ok(fs::read(stdout)?);
        }
        ensure!(Instant::now() < until, "Docker {label} timed out");
        thread::sleep(POLL);
    }
}

struct Container<'a> {
    output: &'a Path,
    name: String,
    removed: bool,
}
impl Container<'_> {
    fn remove(&mut self) -> Result<()> {
        // The unique label restricts cleanup even if create's response was lost.
        let filter = format!("label=vth.run={}", self.name);
        let ids = docker(
            self.output,
            "owned",
            &["ps", "-aq", "--filter", &filter],
            Instant::now() + Duration::from_secs(5),
        )?;
        for id in std::str::from_utf8(&ids)?.lines() {
            ensure!(
                id.len() >= 12 && id.chars().all(|ch| ch.is_ascii_hexdigit()),
                "invalid owned Docker ID"
            );
            docker(
                self.output,
                "remove",
                &["rm", "--force", id],
                Instant::now() + Duration::from_secs(5),
            )?;
        }
        self.removed = true;
        Ok(())
    }
}
impl Drop for Container<'_> {
    fn drop(&mut self) {
        if !self.removed {
            let _ = self.remove();
        }
    }
}

pub(super) fn execute(
    runtime: &Runtime,
    args: &CheckArgs,
    binary: &Path,
    output: &Path,
    template: &str,
    report: &mut Value,
    deadline: Instant,
) -> Result<()> {
    ensure!(
        cfg!(target_os = "linux"),
        "Docker fragment checks require a local Linux Docker daemon"
    );
    let subject = args.selected_subject();
    let image = runtime.required("VTH_DOCKER_IMAGE")?;
    // Require a locally prepared immutable image, rather than an implicit pull.
    ensure!(
        image.starts_with("sha256:") || image.contains("@sha256:"),
        "VTH_DOCKER_IMAGE must be an image ID or digest"
    );
    let docker_version: Value = serde_json::from_slice(&docker(
        output,
        "version",
        &["version", "--format", "{{json .}}"],
        deadline,
    )?)?;
    report["docker"] =
        json!({"version":docker_version,"image_requested":image,"container_name":name(output)});
    for directory in ["input", "data"] {
        fs::create_dir(output.join(directory))?;
    }
    let payload = crate::guest::longlog_payload();
    ensure!(
        payload.len() > 16 * 1024 && payload.len() < 64 * 1024,
        "fixture must exceed Docker's fragment threshold but fit the receiver bound"
    );
    let expected: Value = serde_json::from_str(&payload)?;
    ensure!(
        expected
            .as_object()
            .is_some_and(|fields| fields.len() == 1353)
            && expected["bzz"] == "bzz_value",
        "invalid long-log fixture"
    );
    let fixture = output.join("input/input.log");
    fs::write(&fixture, format!("{payload}\n"))?;
    report["input"] = json!({"records":1,"bytes":payload.len()+1,"sha256":sha256(&fixture)?,"application_fields":1353});
    let mut container = Container {
        output,
        name: name(output),
        removed: false,
    };
    let label = format!("vth.run={}", container.name);
    let mount = format!(
        "type=bind,src={},dst=/vth-input,readonly",
        output.join("input").display()
    );
    docker(
        output,
        "create",
        &[
            "create",
            "--name",
            &container.name,
            "--label",
            &label,
            "--log-driver",
            "json-file",
            "--network",
            "none",
            "--read-only",
            "--cap-drop",
            "ALL",
            "--mount",
            &mount,
            "--entrypoint",
            "/bin/sh",
            &image,
            "-c",
            "while [ ! -e /vth-input/emit ]; do sleep 0.05; done; cat /vth-input/input.log",
        ],
        deadline,
    )?;
    // The logging driver assigns LogPath when the silent container starts.
    docker(output, "start", &["start", &container.name], deadline)?;
    let inspected: Value = serde_json::from_slice(&docker(
        output,
        "inspect",
        &["inspect", &container.name],
        deadline,
    )?)?;
    let info = &inspected[0];
    let container_id = info["Id"].as_str().context("Docker container ID absent")?;
    let log_path = info["LogPath"].as_str().context("Docker LogPath absent")?;
    ensure!(
        !log_path.is_empty(),
        "Docker logging driver has no local LogPath"
    );
    ensure!(
        info["HostConfig"]["LogConfig"]["Type"] == "json-file",
        "Docker fixture must use json-file"
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(log_path, output.join("docker-container.log"))?;
    report["docker"]["container_id"] = info["Id"].clone();
    report["docker"]["image_id"] = info["Image"].clone();
    report["docker"]["log_path"] = json!(log_path);
    let transport = subject.transport(crate::cli::CheckCase::RegexParsing);
    let receiver =
        Receiver::start_values(vec![expected], Format::ApplicationJson, None, transport, 16)?;
    let metrics_reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let metrics_port = metrics_reservation.local_addr()?.port();
    let config = output.join(subject.config_name());
    fs::write(
        &config,
        render(
            template,
            output,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
            SocketAddr::from((Ipv4Addr::LOCALHOST, receiver.port())),
            metrics_port,
            subject,
            args.case,
        )?,
    )?;
    report["config_sha256"] = json!(sha256(&config)?);
    if subject == Subject::Logstash {
        adapters::logstash::prepare(output)?;
    }
    if matches!(subject, Subject::SplunkHf | Subject::SplunkUf) {
        adapters::splunk::prepare(binary, output, &fs::read_to_string(&config)?)?;
    }
    phase(output, report, "version")?;
    let version_spec = Invocation::new(&binary.to_string_lossy(), output)
        .args(subject.version_args().iter().copied());
    let mut version = Target::spawn(runtime, &version_spec, output, "version", subject)?;
    while version.status()?.is_none() {
        budget(deadline)?;
        thread::sleep(POLL);
    }
    ensure!(
        version.status.context("missing version status")?.success(),
        "subject version failed"
    );
    let version_file = output.join("version.stdout.log");
    ensure!(
        fs::metadata(&version_file)?.len() <= 16 * 1024,
        "version exceeds 16 KiB"
    );
    let version_text = fs::read_to_string(version_file)?;
    ensure!(
        subject.identifies(&version_text),
        "binary does not identify as {}",
        subject.name()
    );
    report["subject"]["version"] = json!(version_text.trim());
    drop(version);
    let mut invocation = subject.invocation(
        binary,
        output,
        &config,
        args.vector_cpus.as_deref(),
        args.timeout_secs,
    );
    if subject == Subject::Logstash {
        invocation = invocation.args(["--pipeline.workers", "1", "--pipeline.ordered", "true"]);
    }
    if subject == Subject::Vector {
        // These native events are the fixture's discovery/EOF barriers. Their
        // default per-callsite rate limit would hide repeated watches or die.
        invocation = invocation.args(["--internal-log-rate-limit", "0"]).env(
            "VECTOR_LOG",
            "info,vector::internal_events::docker_logs=debug",
        );
    }
    report["invocation"] = serde_json::to_value(&invocation)?;
    phase(output, report, "startup")?;
    drop(metrics_reservation);
    let mut target = Target::spawn(runtime, &invocation, output, subject.name(), subject)?;
    report["process"] = json!({"pid":target.child.id(),"state":"running"});
    save(output, report)?;
    let work = (|| -> Result<()> {
        while !ready(output, subject, container_id)? {
            observe(&mut target, Some(&receiver), output, deadline)?;
            thread::sleep(POLL);
        }
        // Vector has allocated this exact container's watcher; file readers
        // use their native startup barrier and read the retained file from head.
        fs::write(output.join("input/emit"), [])?;
        phase(output, report, "delivery")?;
        let exit = docker(output, "wait", &["wait", &container.name], deadline)?;
        ensure!(
            std::str::from_utf8(&exit)?.trim() == "0",
            "fixture container exited unsuccessfully"
        );
        report["docker"]["producer_exit_code"] = json!(0);
        report["docker"]["natural_exit"] = json!(true);
        while !fs::metadata(log_path).is_ok_and(|metadata| metadata.len() > payload.len() as u64) {
            budget(deadline)?;
            thread::sleep(POLL);
        }
        // Validate what Docker actually received and split, independently of B.
        let original = docker(output, "logs", &["logs", &container.name], deadline)?;
        ensure!(
            original == format!("{payload}\n").as_bytes(),
            "Docker source did not emit exactly the expected complete record"
        );
        ensure!(
            fs::metadata(log_path)?.len() <= LOG_LIMIT as u64,
            "Docker log exceeds 1 MiB"
        );
        let fragments = fs::read(log_path)?;
        fs::write(output.join("docker-fragments.jsonl"), &fragments)?;
        let mut joined = String::new();
        let mut count = 0;
        for line in fragments
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let fragment: Value = serde_json::from_slice(line)?;
            ensure!(fragment["stream"] == "stdout", "unexpected Docker stream");
            joined.push_str(
                fragment["log"]
                    .as_str()
                    .context("Docker fragment log missing")?,
            );
            count += 1;
        }
        ensure!(
            count > 1 && joined == format!("{payload}\n"),
            "Docker did not produce multiple fragments of exactly one original record"
        );
        report["docker"]["fragments"] = json!(count);
        report["docker"]["source_verified"] = json!(true);
        save(output, report)?;
        while observe(&mut target, Some(&receiver), output, deadline)? < 1 {
            thread::sleep(POLL);
        }
        if subject == Subject::Vector {
            let mut quiescent = None;
            let mut previous = (0, 0, false);
            loop {
                observe(&mut target, Some(&receiver), output, deadline)?;
                let state = vector_lifecycle(output, container_id)?;
                let complete = state.0 > 0 && state.0 == state.1 && state.2;
                if !complete || state != previous {
                    quiescent = None;
                }
                if complete {
                    let since = quiescent.get_or_insert_with(Instant::now);
                    if since.elapsed() >= Duration::from_millis(100) {
                        report["docker"]["native_lifecycle"] = json!({"watchers_started":state.0,"watchers_stopped":state.1,"die_observed":state.2,"scope":"exact container ID and Docker input source; native log rate limiting disabled"});
                        break;
                    }
                }
                previous = state;
                thread::sleep(POLL);
            }
        }
        Ok(())
    })();
    phase(output, report, "shutdown")?;
    let cleanup = target.stop(deadline.min(Instant::now() + Duration::from_secs(10)));
    drop(target);
    let mut finalization_errors = Vec::new();
    report["process"] = match cleanup {
        Ok(process) => process,
        Err(error) => {
            finalization_errors.push(format!("process cleanup: {error:#}"));
            json!({"graceful":false,"error":error.to_string()})
        }
    };
    let subject_errors = match logs_final(output, subject) {
        Ok(logs) => logs.errors,
        Err(error) => {
            finalization_errors.push(format!("log inspection: {error:#}"));
            Vec::new()
        }
    };
    report["verification"] = match receiver.finish() {
        Ok(verification) => verification,
        Err(error) => {
            finalization_errors.push(format!("receiver drain: {error:#}"));
            json!({"passed":false,"error":error.to_string()})
        }
    };
    let removed = container.remove();
    report["docker"]["removed"] = json!(removed.is_ok());
    if let Err(error) = removed {
        finalization_errors.push(format!("container removal: {error:#}"));
    }
    report["subject_errors"] = json!(subject_errors);
    report["finalization_errors"] = json!(finalization_errors);
    work?;
    ensure!(
        finalization_errors.is_empty(),
        "{}",
        finalization_errors.join("; ")
    );
    budget(deadline)?;
    if report["process"]["graceful"] != true
        || report["verification"]["passed"] != true
        || !subject_errors.is_empty()
    {
        return Err(stopped(
            "failed",
            "Docker delivery, native health or shutdown verification failed",
        ));
    }
    Ok(())
}

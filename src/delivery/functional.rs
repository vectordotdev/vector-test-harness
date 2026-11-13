//! Small application contracts using the same subjects, receivers and cleanup
//! as the finite delivery workloads.

use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Instant,
};

use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};

use super::{
    adapters, budget, connect,
    fixture::Format,
    logs_final, observe, phase,
    receiver::{Receiver, Transport},
    render, save, stopped,
    subject::Subject,
    Input, Target, LOG_LIMIT, POLL,
};
use crate::{
    cli::{CheckArgs, CheckCase},
    runtime::{sha256, Invocation, Runtime},
};

fn observe_receivers(
    target: &mut Target,
    receivers: &[(&str, Receiver)],
    output: &Path,
    expected: &[usize],
    deadline: Instant,
) -> Result<bool> {
    let mut complete = true;
    for ((name, receiver), expected) in receivers.iter().zip(expected) {
        let count = observe(target, Some(receiver), output, deadline)?;
        ensure!(
            count <= *expected,
            "{name} received a record before its input phase"
        );
        complete &= count == *expected;
    }
    Ok(complete)
}

fn wait_receivers(
    target: &mut Target,
    receivers: &[(&str, Receiver)],
    output: &Path,
    expected: &[usize],
    deadline: Instant,
) -> Result<()> {
    while !observe_receivers(target, receivers, output, expected, deadline)? {
        thread::sleep(POLL);
    }
    Ok(())
}

fn log_offsets(output: &Path, subject: Subject) -> Result<Vec<(std::path::PathBuf, usize)>> {
    ["stdout", "stderr"]
        .into_iter()
        .map(|stream| {
            let path = output.join(format!("{}.{stream}.log", subject.name()));
            let offset = fs::metadata(&path)?.len().try_into()?;
            Ok((path, offset))
        })
        .collect()
}

fn reloaded(
    subject: Subject,
    source: SocketAddr,
    offsets: &[(std::path::PathBuf, usize)],
) -> Result<bool> {
    for (path, offset) in offsets {
        ensure!(
            fs::metadata(path)?.len() <= LOG_LIMIT as u64,
            "subject log exceeds 1 MiB"
        );
        let bytes = fs::read(path)?;
        let suffix = bytes
            .get(*offset..)
            .context("subject log truncated during reload")?;
        if subject == Subject::FluentBit {
            if String::from_utf8_lossy(suffix)
                .lines()
                .any(|line| line.ends_with("[reload] successful reload done."))
            {
                return Ok(true);
            }
            continue;
        }
        if subject == Subject::Telegraf {
            let text = String::from_utf8_lossy(suffix);
            if let Some((_, restarting)) = text.split_once("Reloading Telegraf config") {
                if restarting.contains(&format!(
                    "[inputs.socket_listener] Listening on tcp://{source}"
                )) {
                    return Ok(true);
                }
            }
            continue;
        }
        let mut reload_started = false;
        for line in suffix.split_inclusive(|byte| *byte == b'\n') {
            if !line.ends_with(b"\n") {
                continue;
            }
            let Ok(event) = serde_json::from_slice::<Value>(line) else {
                continue;
            };
            let message = event["fields"]["message"]
                .as_str()
                .or_else(|| event["logEvent"]["message"].as_str())
                .or_else(|| event["message"].as_str());
            let main_pipeline = event["pipeline.id"]
                .as_str()
                .or_else(|| event["logEvent"]["pipeline.id"].as_str())
                == Some("main");
            if (subject == Subject::Fluentd
                && message == Some("fluentd worker is now stopping worker=0")
                && event["worker_id"].as_u64() == Some(0))
                || (subject == Subject::Logstash
                    && message == Some("Reloading pipeline")
                    && main_pipeline)
            {
                reload_started = true;
            }
            let ready = match subject {
                Subject::Vector => message == Some("Vector has reloaded."),
                Subject::Fluentd => {
                    reload_started
                        && message == Some("fluentd worker is now running worker=0")
                        && event["worker_id"].as_u64() == Some(0)
                }
                Subject::Logstash => {
                    reload_started && message == Some("Pipeline started") && main_pipeline
                }
                _ => false,
            };
            if ready {
                return Ok(true);
            }
        }
    }
    Ok(false)
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
    let subject = args.selected_subject();
    let reload = matches!(args.case, CheckCase::Sighup);
    ensure!(
        matches!(args.case, CheckCase::WrappedJson | CheckCase::Sighup),
        "unknown application contract"
    );
    if reload {
        ensure!(
            matches!(
                subject,
                Subject::Vector
                    | Subject::FluentBit
                    | Subject::Telegraf
                    | Subject::Fluentd
                    | Subject::Logstash
            ),
            "{} cannot switch output configuration through SIGHUP in this contract",
            subject.display_name()
        );
    }
    for directory in ["input", "data"] {
        fs::create_dir(output.join(directory))?;
    }
    let payloads = if reload {
        vec![json!({"log":"First line"}), json!({"log":"Second line"})]
    } else {
        vec![json!({"log":"{\"message\":\"Hello world\"}"})]
    };
    let mut receivers = Vec::new();
    let mut inputs = Vec::new();
    for (index, payload) in payloads.iter().enumerate() {
        let name = if !reload {
            "output"
        } else if index == 0 {
            "before_reload"
        } else {
            "after_reload"
        };
        let (expected, format, transport) = if !reload {
            (
                json!({"message":"Hello world"}),
                Format::ApplicationJson,
                subject.transport(args.case),
            )
        } else if subject == Subject::Fluentd {
            (
                json!({"message":payload.to_string()}),
                Format::ApplicationJson,
                Transport::ForwardMessage,
            )
        } else {
            (
                json!(payload.to_string()),
                Format::ApplicationText,
                Transport::Text,
            )
        };
        let receiver = Receiver::start_values(
            vec![expected.clone()],
            format,
            None,
            transport,
            args.delivery.receiver_reads_per_pass as usize,
        )?;
        report["expected"][name] = json!([expected]);
        receivers.push((name, receiver));
        let path = output.join("input").join(format!("{name}.jsonl"));
        fs::write(&path, format!("{payload}\n"))?;
        report["inputs"][name] =
            json!({"path":path,"sha256":sha256(&path)?,"bytes":fs::metadata(&path)?.len()});
        inputs.push(path);
    }
    report["fixture"] = json!({"version":if reload {"sighup-route-v1"} else {"wrapped-json-v1"},"records":payloads.len()});
    let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let source = reservation.local_addr()?;
    let metrics_reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let metrics_port = metrics_reservation.local_addr()?.port();
    let destination =
        |index: usize| SocketAddr::from((Ipv4Addr::LOCALHOST, receivers[index].1.port()));
    let config = output.join(subject.config_name());
    let initial = render(
        template,
        output,
        source,
        destination(0),
        metrics_port,
        subject,
        args.case,
    )?;
    fs::write(&config, &initial)?;
    fs::write(output.join("initial.config"), initial)?;
    report["config_sha256"] = json!(sha256(&config)?);
    if matches!(subject, Subject::SplunkHf | Subject::SplunkUf) {
        adapters::splunk::prepare(binary, output, &fs::read_to_string(&config)?)?;
        report["subject"]["engine_binary_sha256"] =
            json!(sha256(&adapters::splunk::engine_binary(binary)?)?);
    }
    phase(output, report, "version")?;
    if subject == Subject::Logstash {
        adapters::logstash::prepare(output)?;
    }
    let version_invocation = Invocation::new(&binary.to_string_lossy(), output)
        .args(subject.version_args().iter().copied());
    let mut version = Target::spawn(runtime, &version_invocation, output, "version", subject)?;
    while version.status()?.is_none() {
        budget(deadline)?;
        thread::sleep(POLL);
    }
    ensure!(
        version.status.unwrap().success(),
        "subject version command failed"
    );
    ensure!(
        fs::metadata(output.join("version.stdout.log"))?.len() <= 16 * 1024,
        "subject version output exceeds 16 KiB"
    );
    let version_text = fs::read_to_string(output.join("version.stdout.log"))?;
    ensure!(
        subject.identifies(&version_text),
        "supplied binary did not identify itself as {}",
        subject.display_name()
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
    if reload && subject == Subject::Fluentd {
        invocation = adapters::fluentd::configure_reload(invocation);
    }
    report["invocation"] = serde_json::to_value(&invocation)?;
    phase(output, report, "startup")?;
    drop(reservation);
    drop(metrics_reservation);
    let mut target = Target::spawn(runtime, &invocation, output, subject.name(), subject)?;
    report["process"] = json!({"pid":target.child.id(),"state":"running"});
    save(output, report)?;
    let work = (|| -> Result<()> {
        let mut published = 0;
        phase(output, report, "first_delivery")?;
        let mut input = Input::Tcp(
            connect(source, &mut target, output, deadline)?,
            args.delivery.producer_chunk_bytes as usize,
        );
        input.send(
            &inputs[0],
            &mut target,
            Some(&receivers[0].1),
            output,
            deadline,
            &mut published,
        )?;
        drop(input);
        report["published_input_bytes"] = json!(published);
        wait_receivers(
            &mut target,
            &receivers,
            output,
            if reload { &[1, 0] } else { &[1] },
            deadline,
        )?;
        if reload {
            let offsets = log_offsets(output, subject)?;
            let reloaded_config = render(
                template,
                output,
                source,
                destination(1),
                metrics_port,
                subject,
                args.case,
            )?;
            fs::write(&config, &reloaded_config)?;
            fs::write(output.join("reloaded.config"), reloaded_config)?;
            report["reloaded_config_sha256"] = json!(sha256(&config)?);
            phase(output, report, "reload")?;
            // Signal only the main process. Fluentd's supervisor coordinates
            // its worker replacement; process-group HUP would race that work.
            let status = Command::new("/bin/kill")
                .args(["-HUP", "--", &target.child.id().to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            ensure!(status.success(), "could not send SIGHUP to the subject");
            while !reloaded(subject, source, &offsets)? {
                observe_receivers(&mut target, &receivers, output, &[1, 0], deadline)?;
                thread::sleep(POLL);
            }
            report["reload"] = json!({"signal":"SIGHUP","same_main_pid":target.child.id(),"completion":"native post-signal readiness log; verified second-receiver delivery follows"});
            phase(output, report, "second_delivery")?;
            let mut input = Input::Tcp(
                connect(source, &mut target, output, deadline)?,
                args.delivery.producer_chunk_bytes as usize,
            );
            input.send(
                &inputs[1],
                &mut target,
                Some(&receivers[1].1),
                output,
                deadline,
                &mut published,
            )?;
            drop(input);
            report["published_input_bytes"] = json!(published);
            wait_receivers(&mut target, &receivers, output, &[1, 1], deadline)?;
        }
        Ok(())
    })();
    report["verification_phase"] = report["phase"].clone();
    phase(output, report, "shutdown")?;
    let cleanup_deadline = if work.is_ok() {
        deadline
    } else {
        deadline.min(Instant::now() + std::time::Duration::from_secs(2))
    };
    let cleanup = target.stop(cleanup_deadline);
    drop(target);
    let mut finalization_errors = Vec::new();
    report["process"] = match cleanup {
        Ok(value) => value,
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
    let mut verdicts = serde_json::Map::new();
    let mut passed = true;
    for (name, receiver) in receivers {
        let verdict = match receiver.finish() {
            Ok(value) => value,
            Err(error) => {
                finalization_errors.push(format!("{name} receiver drain: {error:#}"));
                json!({"passed":false,"error":error.to_string()})
            }
        };
        passed &= verdict["passed"] == true;
        verdicts.insert(name.to_owned(), verdict);
    }
    report["verification"] = json!({"passed":passed,"receivers":verdicts});
    report["subject_errors"] = json!(subject_errors);
    report["finalization_errors"] = json!(finalization_errors);
    work?;
    ensure!(
        finalization_errors.is_empty(),
        "{}",
        finalization_errors.join("; ")
    );
    budget(deadline)?;
    if report["process"]["graceful"] != true {
        return Err(stopped("failed", "subject did not shut down gracefully"));
    }
    if !passed || !subject_errors.is_empty() {
        bail!(stopped("failed", "final application verification failed"));
    }
    Ok(())
}

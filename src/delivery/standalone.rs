//! Original finite stdin fixtures, with native terminal counts and owned cleanup.
//! These process durations include startup and client decompression/streaming.

mod klog;
mod profile;

use std::{
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::Path,
    process::{ChildStdin, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{ensure, Context, Result};
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    adapters, budget, environment, logs, logs_final, metrics, phase, render, save, stopped,
    subject::Subject, Stop, Target, POLL,
};
use crate::{
    cli::{CheckArgs, CheckCase},
    runtime::{check_file, sha256, Invocation, Runtime},
};

/// A capability exclusion describes this profile's contract, not the product's
/// abilities through every other input. The CLI and matrix share this registry.
pub(crate) fn prerequisite(
    case: CheckCase,
    subject: Subject,
) -> Option<(&'static str, &'static str)> {
    match case {
        CheckCase::StdinToBlackhole | CheckCase::KlogRemap
            if matches!(subject, Subject::SplunkHf | Subject::SplunkUf) => Some((
                "unsupported",
                "this forwarding-daemon profile has no verified process-stdin input: UF disables the FIFO pipeline, and HF rejected /dev/stdin as a FIFO; a separate named-FIFO/file source would change this contract",
            )),
        CheckCase::StdinToBlackhole | CheckCase::KlogRemap | CheckCase::FileToBlackhole => None,
        _ => Some(("pending_runner", "not a standalone case")),
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct InputStats {
    records: u64,
    bytes: u64,
    sha256: String,
}

fn transfer(
    input: &Path,
    output: &mut impl Write,
    chunk_bytes: usize,
    deadline: Option<Instant>,
) -> Result<InputStats> {
    let mut reader = MultiGzDecoder::new(fs::File::open(input)?);
    let mut chunk = vec![0; chunk_bytes];
    let mut digest = Sha256::new();
    let mut records = 0_u64;
    let mut bytes = 0_u64;
    let mut last = None;
    loop {
        if let Some(deadline) = deadline {
            budget(deadline)?;
        }
        let length = reader.read(&mut chunk)?;
        if length == 0 {
            break;
        }
        let data = &chunk[..length];
        bytes = bytes
            .checked_add(length as u64)
            .context("input size overflow")?;
        ensure!(
            bytes <= 2 * 1024 * 1024 * 1024,
            "standalone input exceeds 2 GiB"
        );
        records += data.iter().filter(|byte| **byte == b'\n').count() as u64;
        ensure!(
            records <= 10_000_000,
            "standalone input exceeds ten million records"
        );
        digest.update(data);
        output.write_all(data)?;
        last = data.last().copied();
    }
    ensure!(last == Some(b'\n'), "standalone fixture must end with LF");
    ensure!(
        records > 0,
        "standalone fixture contains no complete records"
    );
    Ok(InputStats {
        records,
        bytes,
        sha256: format!("{:x}", digest.finalize()),
    })
}

fn native_count(target: &mut Target, output: &Path, deadline: Instant) -> Result<Option<u64>> {
    budget(deadline)?;
    target.running()?;
    let observed = logs(output, target.subject)?;
    if !observed.errors.is_empty()
        && !matches!(target.subject, Subject::SplunkHf | Subject::SplunkUf)
    {
        return Err(stopped("failed", observed.errors.join("; ")));
    }
    let count = match observed.events {
        Some(count) => Some(count),
        None => metrics::count(target.subject, output)?,
    };
    if let Some(count) = count {
        ensure!(
            target.last_count.is_none_or(|previous| count >= previous),
            "native terminal count decreased"
        );
        target.last_count = Some(count);
    }
    Ok(count)
}

fn check_version(
    runtime: &Runtime,
    subject: Subject,
    binary: &Path,
    output: &Path,
    deadline: Instant,
) -> Result<String> {
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
    let version_path = output.join("version.stdout.log");
    ensure!(
        fs::metadata(&version_path)?.len() <= 16 * 1024,
        "version output exceeds 16 KiB"
    );
    let version_text = fs::read_to_string(version_path)?;
    ensure!(
        subject.identifies(&version_text),
        "binary did not identify itself as {}",
        subject.display_name()
    );
    drop(version);
    Ok(version_text.trim().to_owned())
}

fn execute(
    runtime: &Runtime,
    args: &CheckArgs,
    binary: &Path,
    output: &Path,
    report: &mut Value,
    deadline: Instant,
) -> Result<()> {
    if matches!(args.case, CheckCase::KlogRemap) {
        return klog::execute(runtime, args, binary, output, report, deadline);
    }
    if matches!(args.case, CheckCase::FileToBlackhole) {
        return profile::execute(runtime, args, binary, output, report, deadline);
    }
    let subject = args.selected_subject();
    let source = runtime
        .root
        .join("cases/performance/stdin_to_blackhole")
        .join(subject.config_name());
    let template = fs::read_to_string(&source).context("read native stdin config")?;
    let input = runtime
        .root
        .join("cases/performance/file_to_blackhole/baked.log.gz");
    let expected = transfer(&input, &mut std::io::sink(), 64 * 1024, Some(deadline))?;
    budget(deadline)?;
    report["input"] = json!({
        "path":input,"compressed_sha256":sha256(&input)?,"decompressed":expected,
        "format":"LF-delimited original checked-in fixture","warmup_records":0,
        "delivery_events_option_applies":false,
    });
    report["config_source"] = json!(source);
    report["config_source_sha256"] = json!(sha256(&source)?);
    for directory in ["data", "input"] {
        fs::create_dir(output.join(directory))?;
    }
    let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let metrics_port = reservation.local_addr()?.port();
    let config = output.join(subject.config_name());
    fs::write(
        &config,
        render(
            &template,
            output,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
            SocketAddr::from((Ipv4Addr::LOCALHOST, 2)),
            metrics_port,
            subject,
            args.case,
        )?,
    )?;
    report["config_sha256"] = json!(sha256(&config)?);
    if matches!(subject, Subject::FluentBit | Subject::Fluentd) {
        fs::write(
            output.join("metrics-endpoint.json"),
            serde_json::to_vec_pretty(&json!({
                "url":format!("http://127.0.0.1:{metrics_port}/{}",if subject == Subject::FluentBit { "api/v1/metrics" } else { "api/plugins.json" }),
                "basis":"native completed-discard output counter before EOF and shutdown; count-only",
            }))?,
        )?;
    }
    if subject == Subject::Logstash {
        adapters::logstash::prepare(output)?;
    }
    phase(output, report, "version")?;
    report["subject"]["version"] =
        json!(check_version(runtime, subject, binary, output, deadline)?);
    let invocation = subject.invocation(
        binary,
        output,
        &config,
        args.vector_cpus.as_deref(),
        args.timeout_secs,
    );
    report["invocation"] = serde_json::to_value(&invocation)?;
    phase(output, report, "streaming")?;
    drop(reservation);
    let process_started = Instant::now();
    let mut target = Target::spawn_with_stdin(
        runtime,
        &invocation,
        output,
        subject.name(),
        subject,
        Stdio::piped(),
    )?;
    target.count_output = true;
    report["process"] = json!({"pid":target.child.id(),"state":"running"});
    report["cpu_placement"] = environment::placement(
        args.vector_cpus.as_deref(),
        target.child.id(),
        subject.name(),
    )?;
    save(output, report)?;
    let mut stdin = target
        .child
        .stdin
        .take()
        .context("subject stdin pipe missing")?;
    let (sender, completed) = mpsc::sync_channel::<(Result<InputStats>, ChildStdin)>(1);
    let chunk_bytes = args.delivery.producer_chunk_bytes as usize;
    let producer = thread::spawn(move || {
        let result = transfer(&input, &mut stdin, chunk_bytes, Some(deadline));
        let _ = sender.send((result, stdin));
    });
    // Native Filebeat CEL file() evaluates this finite input after EOF.
    // Streaming inputs keep stdin open until the terminal counter is sampled.
    let eof_first = subject == Subject::Filebeat;
    report["input_lifecycle"] = json!({"native_input":if eof_first {"CEL file('/dev/stdin')"}else{"native streaming stdin"},"stdin_eof_before_counter":eof_first,"full_input_buffered_until_eof":eof_first,"streaming_capacity_result":false});
    let mut producer_done = false;
    let mut held_stdin = None;
    let work = (|| -> Result<()> {
        loop {
            let count = native_count(&mut target, output, deadline)?;
            ensure!(
                count.is_none_or(|count| count <= expected.records),
                "terminal counter exceeds fixture count"
            );
            if !producer_done {
                match completed.try_recv() {
                    Ok((result, stdin)) => {
                        if eof_first {
                            drop(stdin);
                        } else {
                            held_stdin = Some(stdin);
                        }
                        producer_done = true;
                        let streamed = result?;
                        ensure!(
                            streamed == expected,
                            "input changed between inventory and streaming"
                        );
                        report["streamed"] = serde_json::to_value(streamed)?;
                        phase(output, report, "terminal_count")?;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        anyhow::bail!("stdin producer ended without a receipt")
                    }
                }
            }
            if producer_done && count == Some(expected.records) {
                report["count_observed_seconds"] = json!(process_started.elapsed().as_secs_f64());
                return Ok(());
            }
            thread::sleep(POLL);
        }
    })();
    report["verification_phase"] = report["phase"].clone();
    phase(output, report, "shutdown")?;
    // On failure, killing the owned process group unblocks a full stdin pipe so
    // the bounded producer can be joined. Streaming readers receive EOF after
    // their counter; EOF-buffered CEL has already received it.
    if work.is_err() {
        let _ = target.signal("KILL");
    }
    drop(held_stdin.take());
    drop(completed);
    let producer_result = producer.join();
    let cleanup_deadline = if work.is_ok() {
        deadline
    } else {
        deadline.min(Instant::now() + Duration::from_secs(2))
    };
    let cleanup = target.stop(cleanup_deadline);
    let count_before_shutdown = target.last_count;
    drop(target);
    let mut finalization_errors = Vec::new();
    report["process"] = match cleanup {
        Ok(process) => process,
        Err(error) => {
            finalization_errors.push(format!("process cleanup: {error:#}"));
            json!({"graceful":false,"error":error.to_string()})
        }
    };
    // Native EOF may win the race with the controlled TERM after all records
    // have already reached the terminal output. This is an expected clean exit.
    if work.is_ok() && report["process"]["exit_code"] == 0 && report["process"]["forced"] == false {
        report["process"]["graceful"] = json!(true);
        report["process"]["unexpected_exit"] = json!(false);
        report["process"]["eof_after_verified_count"] = json!(!eof_first);
        report["process"]["eof_before_counter_observation"] = json!(eof_first);
    }
    if producer_result.is_err() {
        finalization_errors.push("stdin producer panicked".to_owned());
    }
    let final_logs = match logs_final(output, subject) {
        Ok(logs) => Some(logs),
        Err(error) => {
            finalization_errors.push(format!("log inspection: {error:#}"));
            None
        }
    };
    let final_count = final_logs
        .as_ref()
        .and_then(|logs| logs.events)
        .or(count_before_shutdown);
    let subject_errors = final_logs.map_or_else(Vec::new, |logs| logs.errors);
    report["subject_errors"] = json!(subject_errors);
    report["finalization_errors"] = json!(finalization_errors);
    let passed = work.is_ok()
        && finalization_errors.is_empty()
        && final_count == Some(expected.records)
        && subject_errors.is_empty()
        && report["process"]["graceful"] == true;
    report["verification"] = json!({
        "kind":"terminal_count_only","passed":passed,"expected":expected.records,
        "processed":final_count,"identities_verified":false,"payloads_verified":false,
        "counter_basis":"native terminal-discard completion counter; final logs when available, otherwise last sample before controlled shutdown",
    });
    report["measurement"] = json!({
        "method":"process-lifecycle-counter-observation-v1",
        "status":"unavailable","seconds":null,
        "reason":"terminal counts do not prove record identity or payload delivery; lifecycle observations are diagnostic only",
        "input_bytes":expected.bytes,"records":expected.records,"calibrated":false,
        "includes":["process startup","client gzip decompression and streaming","native output-counter observation","EOF and controlled shutdown"],
        "count_only":true,"sustainable_capacity":false,
    });
    report["diagnostic_process_elapsed_seconds"] = json!(process_started.elapsed().as_secs_f64());
    work?;
    ensure!(
        finalization_errors.is_empty(),
        "{}",
        finalization_errors.join("; ")
    );
    if !passed {
        return Err(stopped(
            "failed",
            "terminal count or native shutdown health failed",
        ));
    }
    budget(deadline)?;
    Ok(())
}

pub(super) fn run(runtime: &Runtime, args: CheckArgs) -> Result<i32> {
    ensure!(cfg!(unix), "standalone checks require a Unix host");
    ensure!(
        args.vector_cpus.is_none() || cfg!(target_os = "linux"),
        "CPU affinity requires Linux taskset"
    );
    let subject = args.selected_subject();
    let binary = runtime.path(args.binary().context("supply a subject binary")?)?;
    check_file(&binary)?;
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
    if runtime.dry_run {
        println!(
            "{}",
            json!({"action":"standalone_check","case":args.case.name(),"subject":subject.name(),"binary":binary,"output":output,"prerequisite":prerequisite(args.case,subject),"fixture":"original checked-in gzip; no Apache fixture"})
        );
        return Ok(0);
    }
    fs::create_dir_all(output.parent().context("output has no parent")?)?;
    fs::create_dir(&output)?;
    println!("{}", output.display());
    let started = Instant::now();
    let mut report = json!({
        "schema_version":1,"case":args.case.name(),"subject":{"name":subject.name(),"binary":binary,"sha256":sha256(&binary)?},
        "runner":{"version":env!("CARGO_PKG_VERSION"),"sha256":sha256(&std::env::current_exe()?)?},
        "outcome":"running","passed":null,"phase":"setup","performance_result":false,
        "purpose":"native_standalone_contract","environment":environment::capture(),
        "timeout_secs":args.timeout_secs,"scope":"count-only original fixture diagnostic; no payload, identity, or sustainable-capacity claim",
        "measurement":{"status":"unavailable","seconds":null},
    });
    if let Some((outcome, reason)) = prerequisite(args.case, subject) {
        report["outcome"] = json!(outcome);
        report["reason"] = json!(reason);
        report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
        save(&output, &report)?;
        return Ok(2);
    }
    save(&output, &report)?;
    let result = execute(
        runtime,
        &args,
        &binary,
        &output,
        &mut report,
        started + Duration::from_secs(args.timeout_secs),
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
    if code != 0 {
        report["measurement"]["status"] = json!("invalid");
        report["measurement"]["seconds"] = Value::Null;
    }
    report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    save(&output, &report)?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};

    #[test]
    fn streaming_inventory_checks_complete_original_records_and_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.gz");
        let mut gzip = GzEncoder::new(fs::File::create(&path).unwrap(), Compression::fast());
        gzip.write_all(b"first\nsecond\n").unwrap();
        gzip.finish().unwrap();
        let mut output = Vec::new();
        let stats = transfer(&path, &mut output, 3, None).unwrap();
        assert_eq!(stats.records, 2);
        assert_eq!(stats.bytes, 13);
        assert_eq!(output, b"first\nsecond\n");
        let mut gzip = GzEncoder::new(fs::File::create(&path).unwrap(), Compression::fast());
        gzip.write_all(b"partial").unwrap();
        gzip.finish().unwrap();
        assert!(transfer(&path, &mut Vec::new(), 3, None).is_err());
    }
}

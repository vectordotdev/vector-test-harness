//! Independent, streaming verification of the two existing Vector klog pipelines.
//! Full captures remain beside the rendered configs; no discard timing is inferred.

use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{ChildStdin, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{ensure, Context, Result};
use flate2::{read::MultiGzDecoder, write::GzEncoder, Compression};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::super::{
    adapters, budget, logs, logs_final, phase, stopped, subject::Subject, Target, POLL,
};
use super::{check_version, transfer, InputStats};
use crate::{
    cli::CheckArgs,
    runtime::{sha256, Invocation, Runtime},
};

const MAX_LINE: u64 = 1024 * 1024;
const MAX_CAPTURE: u64 = 2 * 1024 * 1024 * 1024;
type Multiset = BTreeMap<String, u64>;

fn line(reader: &mut impl BufRead, buffer: &mut Vec<u8>) -> Result<bool> {
    buffer.clear();
    let length = reader.take(MAX_LINE + 1).read_until(b'\n', buffer)?;
    ensure!(length as u64 <= MAX_LINE, "klog record exceeds 1 MiB");
    if length == 0 {
        return Ok(false);
    }
    ensure!(
        buffer.last() == Some(&b'\n'),
        "klog record is not LF-terminated"
    );
    buffer.pop();
    Ok(true)
}

fn starts_record(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 6
        && matches!(bytes[0], b'I' | b'W' | b'E' | b'F')
        && bytes[1..5].iter().all(u8::is_ascii_digit)
        && text[5..].chars().next().is_some_and(char::is_whitespace)
}

fn insert(multiset: &mut Multiset, event: Value) -> Result<()> {
    let key = serde_json::to_string(&event)?;
    ensure!(
        key.len() <= MAX_LINE as usize,
        "normalized klog record exceeds limit"
    );
    ensure!(
        multiset.len() < 64 || multiset.contains_key(&key),
        "klog reference exceeds its 64 MiB distinct-payload budget"
    );
    *multiset.entry(key).or_default() += 1;
    Ok(())
}

fn expected_event(log: Value, structured: bool, host: &str) -> Result<Value> {
    let mut event = json!({"log":log,"log_type":if structured {"json"} else {"text"},"host":host,"source_type":"stdin"});
    if let Some(timestamp) = event["log"]["ts"].as_f64() {
        ensure!(timestamp.is_finite(), "nonfinite klog timestamp");
        let nanos = timestamp * 1_000_000_000.0;
        ensure!(
            nanos >= 0.0 && nanos < i64::MAX as f64,
            "klog timestamp outside fixture contract"
        );
        event["timestamp"] = json!(nanos as i64);
    }
    Ok(event)
}

fn expected(input: &Path, host: &str, deadline: Instant) -> Result<Multiset> {
    let mut reader = BufReader::new(MultiGzDecoder::new(fs::File::open(input)?));
    let mut data = Vec::new();
    let mut pending = None::<String>;
    let mut output = Multiset::new();
    let mut records = 0_u64;
    while line(&mut reader, &mut data)? {
        if records.is_multiple_of(1024) {
            budget(deadline)?;
        }
        records += 1;
        ensure!(
            records <= 10_000_000,
            "klog input exceeds ten million records"
        );
        let text = std::str::from_utf8(&data).context("non-UTF8 klog input")?;
        match serde_json::from_slice::<Value>(&data) {
            Ok(value) => insert(&mut output, expected_event(value, true, host)?)?,
            Err(_) => {
                if starts_record(text) {
                    if let Some(previous) = pending.take() {
                        insert(&mut output, expected_event(json!(previous), false, host)?)?;
                    }
                }
                match &mut pending {
                    Some(previous) => {
                        previous.push('\n');
                        previous.push_str(text);
                    }
                    None => pending = Some(text.to_owned()),
                }
                ensure!(
                    pending
                        .as_ref()
                        .is_none_or(|value| value.len() <= MAX_LINE as usize),
                    "merged klog record exceeds limit"
                );
            }
        }
    }
    if let Some(previous) = pending {
        insert(&mut output, expected_event(json!(previous), false, host)?)?;
    }
    Ok(output)
}

// Parse the UTC RFC3339 timestamps emitted by this fixture's JSON encoders.
// Integers avoid silently accepting a millisecond-only conversion of input ts.
fn timestamp(text: &str) -> Result<i64> {
    let (body, offset) = if let Some(body) = text.strip_suffix('Z') {
        (body, "Z")
    } else if let Some(body) = text.strip_suffix("+00:00") {
        (body, "+00:00")
    } else if let Some(body) = text.strip_suffix("+0000") {
        (body, "+0000")
    } else {
        anyhow::bail!("klog timestamp must use UTC")
    };
    let _ = offset;
    let bytes = body.as_bytes();
    ensure!(
        bytes.len() >= 19
            && [4, 7].iter().all(|i| bytes[*i] == b'-')
            && bytes[10] == b'T'
            && bytes[13] == b':'
            && bytes[16] == b':',
        "invalid klog timestamp"
    );
    let number = |range: std::ops::Range<usize>| -> Result<i64> {
        let value = body.get(range).context("invalid timestamp boundary")?;
        ensure!(
            value.bytes().all(|byte| byte.is_ascii_digit()),
            "invalid timestamp digits"
        );
        Ok(value.parse()?)
    };
    let year = number(0..4)?;
    let month = number(5..7)?;
    let day = number(8..10)?;
    let hour = number(11..13)?;
    let minute = number(14..16)?;
    let second = number(17..19)?;
    ensure!(
        (1970..=2261).contains(&year)
            && (1..=12).contains(&month)
            && hour < 24
            && minute < 60
            && second < 60,
        "timestamp outside klog fixture contract"
    );
    let leap = |year: i64| year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    ensure!(
        (1..=month_days[month as usize - 1]).contains(&day),
        "invalid timestamp day"
    );
    let leaps = |year: i64| year / 4 - year / 100 + year / 400;
    let days = (year - 1970) * 365 + leaps(year - 1) - leaps(1969)
        + month_days[..month as usize - 1].iter().sum::<i64>()
        + day
        - 1;
    let nanos = if body.len() == 19 {
        0
    } else {
        ensure!(
            bytes[19] == b'.' && (21..=29).contains(&body.len()),
            "invalid timestamp fraction"
        );
        let fraction = body
            .get(20..)
            .context("invalid timestamp fraction boundary")?;
        ensure!(
            fraction.bytes().all(|byte| byte.is_ascii_digit()),
            "invalid timestamp fraction digits"
        );
        fraction.parse::<i64>()? * 10_i64.pow((9 - fraction.len()) as u32)
    };
    Ok((((days * 24 + hour) * 60 + minute) * 60 + second) * 1_000_000_000 + nanos)
}

fn normalize(mut event: Value, host: &str) -> Result<Value> {
    let object = event
        .as_object_mut()
        .context("klog output is not an object")?;
    ensure!(
        object.get("host").and_then(Value::as_str) == Some(host),
        "source host metadata differs from the execution host"
    );
    ensure!(
        object.get("source_type") == Some(&json!("stdin")),
        "stdin source metadata missing"
    );
    let structured = object.get("log_type") == Some(&json!("json"));
    ensure!(
        structured || object.get("log_type") == Some(&json!("text")),
        "invalid klog classification"
    );
    ensure!(
        structured || object.get("log").is_some_and(Value::is_string),
        "text klog payload is not a string"
    );
    let generated = object
        .get("log")
        .and_then(|log| log.get("ts"))
        .and_then(Value::as_f64)
        .is_none();
    let observed = object
        .remove("timestamp")
        .context("klog timestamp missing")?;
    let nanos = timestamp(
        observed
            .as_str()
            .context("klog timestamp is not a string")?,
    )?;
    if !generated {
        object.insert("timestamp".into(), json!(nanos));
    }
    if generated {
        if let Some(end) = object.remove("timestamp_end") {
            timestamp(end.as_str().context("timestamp_end is not a string")?)?;
        }
    }
    Ok(event)
}

fn verify(path: &Path, mut remaining: Multiset, host: &str, deadline: Instant) -> Result<Value> {
    ensure!(
        fs::metadata(path)?.len() <= MAX_CAPTURE,
        "klog output exceeds 2 GiB"
    );
    let expected = remaining.values().sum::<u64>();
    let distinct = remaining.len();
    let (mut received, mut duplicates, mut corrupt, mut malformed) = (0_u64, 0_u64, 0_u64, 0_u64);
    let mut examples = Vec::<String>::new();
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut data = Vec::new();
    while line(&mut reader, &mut data)? {
        if received.is_multiple_of(1024) {
            budget(deadline)?;
        }
        received += 1;
        let value = serde_json::from_slice(&data)
            .context("invalid klog output JSON")
            .and_then(|value| normalize(value, host));
        let error = match value {
            Ok(value) => {
                let key = serde_json::to_string(&value)?;
                match remaining.get_mut(&key) {
                    Some(count) if *count > 0 => {
                        *count -= 1;
                        None
                    }
                    Some(_) => {
                        duplicates += 1;
                        Some("duplicate klog output record".to_owned())
                    }
                    None => {
                        corrupt += 1;
                        Some("unexpected or corrupt klog output record".to_owned())
                    }
                }
            }
            Err(error) => {
                malformed += 1;
                Some(format!("{error:#}"))
            }
        };
        if examples.len() < 8 {
            if let Some(error) = error {
                examples.push(error.chars().take(1024).collect());
            }
        }
    }
    let missing = remaining.values().sum::<u64>();
    let passed = missing == 0 && duplicates == 0 && corrupt == 0 && malformed == 0;
    Ok(
        json!({"passed":passed,"expected":expected,"received":received,"missing":missing,"duplicates":duplicates,"corrupt":corrupt,"malformed":malformed,"examples":examples,"distinct_records":distinct,"payloads_verified":passed,"identities_verified":false,"multiset_verified":passed,"capture_sha256":sha256(path)?,"capture":path,"normalization":"preserve exact execution-host metadata, stdin source_type, structured ts-derived nanoseconds and every application field; exclude only source-generated timestamps"}),
    )
}

fn capture_file(target: &Path, directory: bool) -> Result<Option<PathBuf>> {
    if !target.exists() {
        return Ok(None);
    }
    if !directory {
        ensure!(
            fs::symlink_metadata(target)?.file_type().is_file(),
            "native capture is not a regular file"
        );
        return Ok(Some(target.to_path_buf()));
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "ndjson")
        {
            ensure!(
                entry.file_type()?.is_file(),
                "native capture entry is not a regular file"
            );
            files.push(entry.path());
            ensure!(
                files.len() <= 1,
                "native capture rotated into multiple .ndjson files"
            );
        }
    }
    Ok(files.pop())
}

fn capture_progress(path: &Path, reader: &mut Option<fs::File>, records: &mut u64) -> Result<()> {
    if reader.is_none() && path.exists() {
        *reader = Some(fs::File::open(path)?);
    }
    if let Some(reader) = reader {
        let mut chunk = [0_u8; 64 * 1024];
        for _ in 0..16 {
            let length = reader.read(&mut chunk)?;
            if length == 0 {
                break;
            }
            *records = records
                .checked_add(
                    chunk[..length]
                        .iter()
                        .filter(|byte| **byte == b'\n')
                        .count() as u64,
                )
                .context("capture count overflow")?;
        }
    }
    Ok(())
}

struct Replay<'a> {
    runtime: &'a Runtime,
    args: &'a CheckArgs,
    binary: &'a Path,
    host: &'a str,
    deadline: Instant,
}

impl Replay<'_> {
    fn run(&self, folder: &Path, source: &Path, input: &Path) -> Result<Value> {
        let (runtime, args, binary, host, deadline) = (
            self.runtime,
            self.args,
            self.binary,
            self.host,
            self.deadline,
        );
        fs::create_dir(folder)?;
        fs::create_dir(folder.join("data"))?;
        let reference = expected(input, host, deadline)?;
        let expected_count = reference.values().sum::<u64>();
        let directory_capture = args.selected_subject() == Subject::Filebeat;
        let capture_target = folder.join(if directory_capture {
            "capture"
        } else {
            "received.ndjson"
        });
        let mut raw = capture_target.clone();
        let template = fs::read_to_string(source)?;
        let source_sha256 = format!("{:x}", Sha256::digest(template.as_bytes()));
        let subject = args.selected_subject();
        let rendered = if subject == Subject::Vector {
            let anchor = "  blackhole:\n    type: blackhole\n";
            ensure!(
                template.matches(anchor).count() == 1,
                "klog sink anchor changed; inspect config before substituting capture sink"
            );
            template.replacen(
                anchor,
                &format!(
                "  blackhole:\n    type: file\n    path: {}\n    encoding:\n      codec: json\n",
                serde_json::to_string(&capture_target)?
            ),
                1,
            )
        } else {
            ensure!(
                template.matches("@VTH_CAPTURE_FILE@").count() == 1,
                "native klog capture marker must occur once"
            );
            template.replace(
                "@VTH_CAPTURE_FILE@",
                &serde_json::to_string(&capture_target)?,
            )
        };
        let config = folder.join(subject.config_name());
        fs::write(&config, rendered)?;
        if subject == Subject::Logstash {
            adapters::logstash::prepare(folder)?;
        }
        let mut invocation = subject.invocation(
            binary,
            folder,
            &config,
            args.vector_cpus.as_deref(),
            args.timeout_secs,
        );
        if subject == Subject::Logstash {
            invocation = invocation.args(["--pipeline.workers", "1", "--pipeline.ordered", "true"]);
        }
        let process_started = Instant::now();
        let mut target = Target::spawn_with_stdin(
            runtime,
            &invocation,
            folder,
            subject.name(),
            subject,
            Stdio::piped(),
        )?;
        let mut stdin = target
            .child
            .stdin
            .take()
            .context("klog stdin pipe missing")?;
        let input = input.to_path_buf();
        let chunk = args.delivery.producer_chunk_bytes as usize;
        let service_input = matches!(
            subject,
            Subject::Telegraf | Subject::FluentBit | Subject::Filebeat
        );
        let retain_stdin = matches!(subject, Subject::Telegraf | Subject::FluentBit);
        let (produced, completed) =
            mpsc::sync_channel::<(Result<InputStats>, Option<ChildStdin>)>(1);
        let producer = thread::spawn(move || {
            let result = transfer(&input, &mut stdin, chunk, Some(deadline));
            let held = retain_stdin.then_some(stdin);
            let _ = produced.send((result, held));
        });
        let mut captured = 0_u64;
        let mut capture_reader = None;
        let mut controlled_stop = None;
        let mut streamed_result = None;
        let mut held_stdin = None;
        let work = (|| -> Result<()> {
            loop {
                budget(deadline)?;
                if let Some(actual) = capture_file(&capture_target, directory_capture)? {
                    raw = actual;
                    ensure!(
                        fs::metadata(&raw)?.len() <= MAX_CAPTURE,
                        "klog output exceeds 2 GiB"
                    );
                }
                // This private file-output proof lets an EOF-triggered shutdown
                // finish before judging native health, retaining any valid payload
                // evidence even when a plugin reports a shutdown exception.
                let observed = logs(folder, subject)?;
                if service_input && !observed.errors.is_empty() {
                    return Err(stopped("failed", observed.errors.join("; ")));
                }
                if streamed_result.is_none() {
                    match completed.try_recv() {
                        Ok((result, stdin)) => {
                            streamed_result = Some(result);
                            held_stdin = stdin;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => {
                            anyhow::bail!("klog producer ended without a receipt")
                        }
                    }
                }
                if service_input {
                    if raw.is_file() {
                        capture_progress(&raw, &mut capture_reader, &mut captured)?;
                    }
                    ensure!(
                        captured <= expected_count,
                        "native capture exceeds expected record count"
                    );
                    if streamed_result.is_some() && captured == expected_count {
                        drop(held_stdin.take());
                        controlled_stop = Some(target.stop(deadline)?);
                        break;
                    }
                }
                if let Some(status) = target.status()? {
                    ensure!(
                        status.success(),
                        "{} exited with {status}",
                        subject.display_name()
                    );
                    break;
                }
                thread::sleep(POLL);
            }
            Ok(())
        })();
        if work.is_err() {
            let _ = target.signal("KILL");
        }
        let joined = producer.join();
        if streamed_result.is_none() {
            if let Ok((result, stdin)) = completed.recv() {
                streamed_result = Some(result);
                held_stdin = stdin;
            }
        }
        drop(held_stdin.take());
        let streamed = streamed_result
            .unwrap_or_else(|| Err(anyhow::anyhow!("klog producer panicked without a receipt")));
        let cleanup = match controlled_stop {
            Some(process) => Ok(process),
            None => target.stop(if work.is_ok() {
                deadline
            } else {
                deadline.min(Instant::now() + Duration::from_secs(2))
            }),
        };
        drop(target);
        let mut finalization_errors = Vec::new();
        let mut process = match cleanup {
            Ok(process) => process,
            Err(error) => {
                finalization_errors.push(format!("process cleanup: {error:#}"));
                json!({"graceful":false,"error":error.to_string()})
            }
        };
        if work.is_ok() && process["exit_code"] == 0 && process["forced"] == false {
            process["graceful"] = json!(true);
            process["unexpected_exit"] = json!(false);
            if service_input {
                process["controlled_after_capture_count"] = json!(true);
            } else {
                process["expected_eof"] = json!(true);
            }
        }
        let subject_errors = match logs_final(folder, subject) {
            Ok(logs) => logs.errors,
            Err(error) => {
                finalization_errors.push(format!("log inspection: {error:#}"));
                Vec::new()
            }
        };
        let process_elapsed = process_started.elapsed().as_secs_f64();
        // Keep any complete capture proof even when cleanup or native log
        // inspection failed; those failures still invalidate the whole case.
        let verification = (|| -> Result<Value> {
            raw = capture_file(&capture_target, directory_capture)?
                .context("native klog produced no capture file")?;
            verify(&raw, reference, host, deadline)
        })();
        let verification = match verification {
            Ok(verification) => verification,
            Err(error) => {
                finalization_errors.push(format!("capture verification: {error:#}"));
                json!({"passed":false,"error":error.to_string()})
            }
        };
        let config_sha256 = match sha256(&config) {
            Ok(digest) => Some(digest),
            Err(error) => {
                finalization_errors.push(format!("config inspection: {error:#}"));
                None
            }
        };
        let mut receipt = json!({"process":process,"invocation":invocation,"input":streamed.as_ref().ok(),"config_source":source,"config_source_sha256":source_sha256,"rendered_config":config,"rendered_config_sha256":config_sha256,"expected_records":expected_count,"diagnostic_process_elapsed_seconds":process_elapsed,"subject_errors":subject_errors,"finalization_errors":finalization_errors,"verification":verification,"capture_target":capture_target,"captured_lines_before_shutdown":if service_input {Some(captured)}else{None},"input_lifecycle":{"native_input":if subject==Subject::Filebeat {"CEL file('/dev/stdin')"}else{"native streaming stdin"},"stdin_eof_before_capture":subject==Subject::Filebeat,"full_input_buffered_until_eof":subject==Subject::Filebeat,"stdin_held_until_capture":retain_stdin,"streaming_capacity_result":false},"measurement":{"status":"unavailable","seconds":null,"reason":"functional full-fixture capture substitutes file output for discard; no performance measurement"}});
        let result = (|| -> Result<()> {
            work?;
            ensure!(joined.is_ok(), "klog producer panicked");
            streamed?;
            ensure!(
                finalization_errors.is_empty(),
                "{}",
                finalization_errors.join("; ")
            );
            ensure!(
                receipt["process"]["graceful"] == true,
                "klog process did not stop cleanly"
            );
            ensure!(receipt["verification"]["passed"]==true,"klog payload verification failed: missing={}, duplicates={}, corrupt={}, malformed={}",receipt["verification"]["missing"],receipt["verification"]["duplicates"],receipt["verification"]["corrupt"],receipt["verification"]["malformed"]);
            Ok(())
        })();
        receipt["passed"] = json!(result.is_ok() && subject_errors.is_empty());
        receipt["reason"] = json!(result
            .as_ref()
            .err()
            .map(|error| format!("{error:#}"))
            .or_else(|| (!subject_errors.is_empty()).then(|| subject_errors.join("; "))));
        fs::write(
            folder.join("receipt.json"),
            serde_json::to_vec_pretty(&receipt)?,
        )?;
        result?;
        Ok(receipt)
    }
}

pub(super) fn execute(
    runtime: &Runtime,
    args: &CheckArgs,
    binary: &Path,
    output: &Path,
    report: &mut Value,
    deadline: Instant,
) -> Result<()> {
    let subject = args.selected_subject();
    ensure!(
        matches!(
            subject,
            Subject::Vector
                | Subject::Fluentd
                | Subject::Logstash
                | Subject::Telegraf
                | Subject::FluentBit
                | Subject::Filebeat
        ),
        "klog native product contract pending"
    );
    phase(output, report, "version")?;
    report["subject"]["version"] =
        json!(check_version(runtime, subject, binary, output, deadline)?);
    report["scope"]=json!("functional full-fixture native klog payload proof with JSON file capture; no discard-throughput comparison");
    let host = runtime.capture(Invocation::new("uname", output).args(["-n"]))?;
    let host = host.trim();
    ensure!(
        !host.is_empty() && host.len() <= 255 && !host.contains('\n'),
        "invalid execution host identity"
    );
    report["source_host"] = json!(host);
    let dir = runtime
        .root
        .join("cases/performance/klog_remap_to_blackhole");
    let input = dir.join("baked.log.gz");
    report["input"] = json!({"path":input,"compressed_sha256":sha256(&input)?});
    let canary = output.join("continuations.log.gz");
    let mut encoder = GzEncoder::new(fs::File::create(&canary)?, Compression::fast());
    encoder.write_all(b"I1025 first record\n  first continuation\n{\"ts\":1580306777.04728,\"msg\":\"interleaved\"}\n  continuation across JSON\nW1025 final record\n  final continuation\n")?;
    encoder.finish()?;
    report["variants"] = json!([]);
    let variants = if subject == Subject::Vector {
        vec![
            (
                "remap-multihop".to_owned(),
                dir.join("configs/remap-multihop.yaml"),
            ),
            (
                "remap-nohop".to_owned(),
                dir.join("configs/remap-nohop.yaml"),
            ),
        ]
    } else {
        vec![(subject.name().to_owned(), dir.join(subject.config_name()))]
    };
    let replay = Replay {
        runtime,
        args,
        binary,
        host,
        deadline,
    };
    let mut healthy = true;
    for (name, source) in variants {
        let variant = output.join(&name);
        report["current_variant_artifacts"] = json!(variant);
        phase(output, report, &name)?;
        fs::create_dir(&variant)?;
        let small = replay.run(&variant.join("canary"), &source, &canary)?;
        let full = replay.run(&variant.join("full"), &source, &input)?;
        healthy &= small["passed"] == true && full["passed"] == true;
        report["variants"]
            .as_array_mut()
            .context("variant report is not an array")?
            .push(json!({"name":name,"canary":small,"full_fixture":full}));
    }
    report["verification"] = json!({"kind":"full_fixture_application_multiset","passed":healthy,"payloads_verified":true,"identities_verified":false,"continuation_canary_verified":true,"both_vector_variants_verified":subject==Subject::Vector,"native_health_verified":healthy});
    report["measurement"] = json!({"status":"unavailable","seconds":null,"reason":"functional full-fixture proof with file output; no native discard timing or cross-product ranking"});
    if !healthy {
        return Err(stopped(
            "failed",
            "native klog payload proof completed but native health failed",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_timestamp_oracle_rejects_precision_loss_and_retains_json_fields() {
        assert_eq!(
            timestamp("2020-01-29T14:06:17.047280128Z").unwrap(),
            1_580_306_777_047_280_128
        );
        assert!(timestamp("2020-02-30T14:06:17Z").is_err());
        let reference = expected_event(
            json!({"ts":1580306777.04728,"msg":"original"}),
            true,
            "machine",
        )
        .unwrap();
        let mut exact = reference.clone();
        exact["host"] = json!("machine");
        exact["timestamp"] = json!("2020-01-29T14:06:17.047280128Z");
        assert_eq!(normalize(exact.clone(), "machine").unwrap(), reference);
        exact["timestamp"] = json!("2020-01-29T14:06:17.047Z");
        assert_ne!(normalize(exact, "machine").unwrap(), reference);
        let directory = tempfile::tempdir().unwrap();
        let capture = directory.path().join("records.ndjson");
        let mut record = reference.clone();
        record["timestamp"] = json!("2020-01-29T14:06:17.047280128Z");
        fs::write(&capture, format!("{}\n{}\n", record, record)).unwrap();
        let mut expected = Multiset::new();
        insert(&mut expected, reference).unwrap();
        let observed = verify(
            &capture,
            expected,
            "machine",
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(observed["passed"], false);
        assert_eq!(observed["received"], 2);
        assert_eq!(observed["duplicates"], 1);
        assert_eq!(observed["missing"], 0);
        assert_eq!(capture_file(directory.path(), true).unwrap(), Some(capture));
        fs::write(directory.path().join("rotated.ndjson"), b"{}\n").unwrap();
        assert!(capture_file(directory.path(), true).is_err());
    }
}

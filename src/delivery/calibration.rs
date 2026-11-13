//! Direct producer-to-verifier finite batches. These measure the apparatus, not Vector.

use std::{
    fs::{self, File},
    future::Future,
    io::{BufRead, BufReader, BufWriter, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{ensure, Context, Result};
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, client::conn::http1, Request};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};

use super::{
    budget,
    fixture::Fixture,
    publish,
    receiver::{Receiver, Transport},
    save, stopped, Stop, POLL, WARMUP,
};
use crate::{
    cli::{CalibrateArgs, CalibrationTransport},
    runtime::{sha256, Runtime},
};

const HTTP_BATCH_EVENTS: usize = 256;

struct Phase {
    name: &'static str,
    path: PathBuf,
    through: usize,
}

fn observe(receiver: &Receiver, deadline: Instant) -> Result<usize> {
    budget(deadline)?;
    if receiver.has_errors() {
        return Err(stopped(
            "failed",
            "direct receiver detected an invalid, duplicate or unexpected record",
        ));
    }
    Ok(receiver.unique())
}

async fn bounded<F: Future>(future: F, deadline: Instant) -> Result<F::Output> {
    let mut future = Box::pin(future);
    loop {
        budget(deadline)?;
        if let Ok(value) = tokio::time::timeout(POLL, future.as_mut()).await {
            return Ok(value);
        }
    }
}

fn wait_for(receiver: &Receiver, count: usize, deadline: Instant) -> Result<()> {
    loop {
        let seen = observe(receiver, deadline)?;
        ensure!(
            seen <= count,
            "receiver saw records before their input phase"
        );
        if seen == count {
            return Ok(());
        }
        thread::sleep(POLL);
    }
}

fn prepare(
    fixture: &Fixture,
    transport: CalibrationTransport,
    output: &Path,
    report: &mut Value,
    deadline: Instant,
) -> Result<Vec<Phase>> {
    let mut phases = Vec::new();
    for (name, range) in [
        ("canary", 0..1),
        ("warmup", 1..WARMUP + 1),
        ("measured", WARMUP + 1..fixture.total()),
    ] {
        let through = range.end;
        let count = range.len();
        let path = output.join(format!("{name}.wire"));
        let mut writer = BufWriter::new(File::create(&path)?);
        for id in range {
            if id % 1024 == 0 {
                budget(deadline)?;
            }
            match transport {
                CalibrationTransport::Tcp => writer.write_all(&fixture.record(id))?,
                CalibrationTransport::ParsedJson => {
                    serde_json::to_writer(&mut writer, &fixture.parsed(id))?;
                    writer.write_all(b"\n")?;
                }
                CalibrationTransport::Http => {
                    let line = fixture.record(id);
                    let message = std::str::from_utf8(&line[..line.len() - 1])?;
                    serde_json::to_writer(&mut writer, &json!({"message":message}))?;
                    writer.write_all(b"\n")?;
                }
            }
        }
        writer.flush()?;
        drop(writer);
        report["inputs"][name] = json!({
            "path":path,"sha256":sha256(&path)?,
            "encoded_payload_bytes":fs::metadata(&path)?.len(),
            "logical_input_bytes":count * fixture.record(0).len(),
            "events":count,
        });
        phases.push(Phase {
            name,
            path,
            through,
        });
    }
    Ok(phases)
}

fn tcp(
    receiver: &Receiver,
    phases: &[Phase],
    args: &CalibrateArgs,
    output: &Path,
    report: &mut Value,
    deadline: Instant,
) -> Result<()> {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, receiver.port()));
    budget(deadline)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))?;
    stream.set_write_timeout(Some(Duration::from_millis(50)))?;
    stream.set_nodelay(true)?;
    for phase in phases {
        report["phase"] = json!(phase.name);
        save(output, report)?;
        if phase.name == "measured" {
            receiver.start_measurement()?;
        }
        let mut published = 0;
        let sent = publish(
            &phase.path,
            &mut stream,
            args.delivery.producer_chunk_bytes as usize,
            &mut published,
            deadline,
            || observe(receiver, deadline).map(|_| ()),
        );
        report["published_encoded_payload_bytes"][phase.name] = json!(published);
        sent?;
        wait_for(receiver, phase.through, deadline)?;
    }
    // The receiver must observe EOF, including any trailing bytes, before acceptance.
    drop(stream);
    Ok(())
}

fn http(
    receiver: &Receiver,
    phases: &[Phase],
    output: &Path,
    report: &mut Value,
    deadline: Instant,
) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    runtime.block_on(async {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, receiver.port()));
        budget(deadline)?;
        let stream = bounded(tokio::net::TcpStream::connect(address), deadline).await??;
        stream.set_nodelay(true)?;
        let (mut sender, connection) = bounded(
            http1::handshake::<_, Full<Bytes>>(TokioIo::new(stream)),
            deadline,
        )
        .await??;
        let connection = tokio::spawn(connection);
        for phase in phases {
            report["phase"] = json!(phase.name);
            save(output, report)?;
            if phase.name == "measured" {
                receiver.start_measurement()?;
            }
            let mut reader = BufReader::new(File::open(&phase.path)?);
            let mut published = 0_u64;
            loop {
                observe(receiver, deadline)?;
                let mut body = Vec::new();
                for _ in 0..HTTP_BATCH_EVENTS {
                    if reader.read_until(b'\n', &mut body)? == 0 {
                        break;
                    }
                }
                if body.is_empty() {
                    break;
                }
                let length = body.len() as u64;
                let request = Request::post("/")
                    .header(hyper::header::HOST, address.to_string())
                    .header(hyper::header::CONTENT_TYPE, "application/x-ndjson")
                    .header(hyper::header::CONTENT_LENGTH, length)
                    .body(Full::new(Bytes::from(body)))?;
                let response = bounded(sender.send_request(request), deadline).await??;
                let status = response.status();
                bounded(response.into_body().collect(), deadline).await??;
                ensure!(
                    status.is_success(),
                    "direct HTTP receiver rejected batch: {status}"
                );
                published += length;
                report["published_encoded_payload_bytes"][phase.name] = json!(published);
            }
            loop {
                let seen = observe(receiver, deadline)?;
                ensure!(
                    seen <= phase.through,
                    "receiver saw records before their input phase"
                );
                if seen == phase.through {
                    break;
                }
                tokio::time::sleep(POLL).await;
            }
        }
        drop(sender);
        bounded(connection, deadline).await???;
        Ok(())
    })
    // Dropping the runtime also closes any connection task after an error.
}

pub fn run(runtime: &Runtime, args: CalibrateArgs) -> Result<i32> {
    ensure!(
        !matches!(args.transport, CalibrationTransport::Http)
            || args.delivery.receiver_reads_per_pass == 16,
        "--receiver-reads-per-pass applies only to TCP receivers"
    );
    ensure!(
        !matches!(args.transport, CalibrationTransport::Http)
            || args.delivery.producer_chunk_bytes == 16_384,
        "--producer-chunk-bytes applies only to TCP publishers"
    );
    let name = match args.transport {
        CalibrationTransport::Tcp => "tcp",
        CalibrationTransport::ParsedJson => "parsed-json",
        CalibrationTransport::Http => "http",
    };
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
                .join(format!("calibrate-{name}-{nonce}"))
        });
    ensure!(
        !output.exists(),
        "output directory already exists: {}",
        output.display()
    );
    if runtime.dry_run {
        println!(
            "{}",
            json!({
                "action":"calibrate_direct_path","transport":name,"output":output,
                "events":args.delivery.events,"warmup":WARMUP,"canary":1,
                "delivery":args.delivery,
                "producer_chunk_bytes":args.delivery.producer_chunk_bytes,
                "receiver_reads_per_pass":args.delivery.receiver_reads_per_pass,
                "performance_result":false,
            })
        );
        return Ok(0);
    }
    let runner_sha256 = sha256(&std::env::current_exe()?)?;
    fs::create_dir_all(output.parent().context("output has no parent")?)?;
    fs::create_dir(&output)?;
    println!("{}", output.display());
    let started = Instant::now();
    let deadline = started + Duration::from_secs(args.timeout_secs);
    let mut fixture = Fixture::new(
        args.delivery.events as usize,
        WARMUP,
        format!("{nonce:032x}"),
    );
    let mut report = json!({
        "schema_version":1,"purpose":"direct_path_calibration","transport":name,
        "phase":"setup","outcome":"running","passed":null,"performance_result":false,
        "calibrated":false,"subject":null,"run_id":fixture.token,
        "os":std::env::consts::OS,"architecture":std::env::consts::ARCH,
        "timeout_secs":args.timeout_secs,"runner":{"sha256":runner_sha256,"version":env!("CARGO_PKG_VERSION")},
        "fixture":{"version":"apache-records-v1","token":fixture.token,
            "canary":1,"warmup":WARMUP,"measured":args.delivery.events,"expected_total":fixture.total()},
        "io":{"producer_chunk_bytes":args.delivery.producer_chunk_bytes,
            "receiver_reads_per_pass":args.delivery.receiver_reads_per_pass,
            "http_batch_events":if matches!(args.transport,CalibrationTransport::Http) {Some(HTTP_BATCH_EVENTS)} else {None},
            "http_concurrency":if matches!(args.transport,CalibrationTransport::Http) {Some(1)} else {None}},
        "inputs":{},"published_encoded_payload_bytes":{},
        "scope":"same-host finite producer-to-verifier baseline; no Vector, CPU isolation or sustainable-capacity claim",
        "environment":super::environment::capture(),
        "wire_bytes_basis":"encoded application payload; HTTP headers excluded",
        "producer_observation_interval_ms":POLL.as_millis(),
    });
    save(&output, &report)?;
    let result = (|| -> Result<()> {
        fixture.prepare_text(|| budget(deadline))?;
        report["fixture"]["verification_reference_bytes"] = json!(fixture.cache_bytes());
        let phases = prepare(&fixture, args.transport, &output, &mut report, deadline)?;
        let transport = match args.transport {
            CalibrationTransport::Tcp => Transport::Text,
            CalibrationTransport::ParsedJson => Transport::ParsedJson,
            CalibrationTransport::Http => Transport::HttpJson,
        };
        let receiver = Receiver::start_with_reads(
            fixture,
            transport,
            args.delivery.receiver_reads_per_pass as usize,
        )?;
        let work = match args.transport {
            CalibrationTransport::Tcp | CalibrationTransport::ParsedJson => {
                tcp(&receiver, &phases, &args, &output, &mut report, deadline)
            }
            CalibrationTransport::Http => http(&receiver, &phases, &output, &mut report, deadline),
        };
        report["verification_phase"] = report["phase"].clone();
        report["phase"] = json!("drain");
        save(&output, &report)?;
        report["verification"] = receiver.finish()?;
        work?;
        budget(deadline)?;
        ensure!(
            report["verification"]["passed"] == true,
            "direct path verification failed"
        );
        ensure!(
            report["verification"]["measurement"]["seconds"]
                .as_f64()
                .is_some_and(|seconds| seconds.is_finite() && seconds > 0.0),
            "direct path has no complete measurement interval"
        );
        Ok(())
    })();
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
    let candidate = report["verification"]
        .as_object_mut()
        .and_then(|verification| verification.remove("measurement"));
    let mut measurement = candidate.unwrap_or_else(|| {
        json!({
            "method":"input-release-to-receiver-verification-v1","seconds":null,
        })
    });
    measurement["calibrated"] = json!(false);
    measurement["measured_events"] = json!(args.delivery.events);
    measurement["input_bytes"] = report["inputs"]["measured"]["logical_input_bytes"].clone();
    measurement["encoded_payload_bytes"] =
        report["inputs"]["measured"]["encoded_payload_bytes"].clone();
    if code == 0 {
        measurement["status"] = json!("verified");
    } else {
        measurement["status"] = json!("invalid");
        measurement["seconds"] = Value::Null;
        measurement["invalidated_by"] = json!(outcome);
    }
    report["measurement"] = measurement;
    report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    report["phase"] = json!("finished");
    save(&output, &report)?;
    Ok(code)
}

//! Bounded loopback receivers for the delivery checks. A successful verdict includes
//! a drain after the subject process has exited, not just receipt of the last ID.

use super::fixture::{Fixture, Format, Verifier};
use anyhow::{Context, Result};
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context as TaskContext, Poll};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::task::JoinSet;

const POLL: Duration = Duration::from_millis(5);
const DRAIN_QUIET: Duration = Duration::from_millis(100);
const DRAIN_LIMIT: Duration = Duration::from_secs(2);
const MAX_CONNECTIONS: usize = 32;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const REQUEST_LIMIT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy)]
pub(super) enum Transport {
    Text,
    ParsedJson,
    HttpJson,
    HttpLogJson,
    BeatsMessage,
    BeatsParsed,
    ForwardMessage,
    ForwardParsed,
    ElasticBulk,
}

impl Transport {
    fn name(self) -> &'static str {
        match self {
            Self::Text => "tcp_text",
            Self::ParsedJson => "tcp_parsed_json",
            Self::HttpJson => "http_json",
            Self::HttpLogJson => "http_log_json",
            Self::BeatsMessage => "beats_message",
            Self::BeatsParsed => "beats_parsed",
            Self::ForwardMessage => "forward_message",
            Self::ForwardParsed => "forward_parsed",
            Self::ElasticBulk => "elastic_bulk",
        }
    }

    fn format(self) -> Format {
        match self {
            Self::Text => Format::Text,
            Self::ParsedJson | Self::BeatsParsed | Self::ForwardParsed => Format::ParsedJson,
            Self::HttpJson | Self::BeatsMessage | Self::ForwardMessage | Self::ElasticBulk => {
                Format::MessageJson("message")
            }
            Self::HttpLogJson => Format::MessageJson("log"),
        }
    }

    fn is_http(self) -> bool {
        matches!(self, Self::HttpJson | Self::HttpLogJson | Self::ElasticBulk)
    }
}

pub(super) struct Receiver {
    port: u16,
    transport: Transport,
    reads_per_pass: usize,
    verifier: Arc<Mutex<Verifier>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<Value>>>,
}

impl Receiver {
    #[cfg(test)]
    pub(super) fn start(fixture: Fixture, transport: Transport) -> Result<Self> {
        Self::start_with_reads(fixture, transport, 16)
    }

    pub(super) fn start_with_reads(
        fixture: Fixture,
        transport: Transport,
        reads_per_pass: usize,
    ) -> Result<Self> {
        Self::start_verifier(
            Verifier::new(fixture, transport.format()),
            transport,
            reads_per_pass,
        )
    }

    pub(super) fn start_values(
        expected: Vec<Value>,
        format: Format,
        projection: Option<Vec<String>>,
        transport: Transport,
        reads_per_pass: usize,
    ) -> Result<Self> {
        Self::start_verifier(
            Verifier::new_values(expected, format, projection)?,
            transport,
            reads_per_pass,
        )
    }

    fn start_verifier(
        verifier: Verifier,
        transport: Transport,
        reads_per_pass: usize,
    ) -> Result<Self> {
        anyhow::ensure!(
            (1..=1024).contains(&reads_per_pass),
            "receiver reads per pass must be between 1 and 1024"
        );
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .context("bind delivery receiver to loopback")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let verifier = Arc::new(Mutex::new(verifier));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_verifier = verifier.clone();
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name(format!("delivery-{}", transport.name()))
            .spawn(move || {
                if transport.is_http() {
                    receive_http(listener, worker_verifier, worker_stop, transport)
                } else {
                    receive_tcp(
                        listener,
                        worker_verifier,
                        worker_stop,
                        reads_per_pass,
                        transport,
                    )
                }
            })
            .context("start delivery receiver worker")?;
        Ok(Self {
            port,
            transport,
            reads_per_pass,
            verifier,
            stop,
            worker: Some(worker),
        })
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    pub(super) fn unique(&self) -> usize {
        verifier_lock(&self.verifier).unique()
    }

    pub(super) fn has_errors(&self) -> bool {
        verifier_lock(&self.verifier).has_errors()
    }

    pub(super) fn start_measurement(&self) -> Result<()> {
        verifier_lock(&self.verifier).start_measurement()
    }

    /// The caller must terminate and reap the subject before asking for a drain.
    pub(super) fn finish(mut self) -> Result<Value> {
        self.stop.store(true, Ordering::Release);
        let observations = match self.worker.take().expect("receiver worker missing").join() {
            Ok(Ok(observations)) => observations,
            Ok(Err(error)) => json!({
                "drained": false,
                "drain_error": format!("delivery receiver worker failed: {error:#}"),
            }),
            Err(panic) => {
                let reason = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic payload");
                json!({
                    "drained": false,
                    "drain_error": format!("delivery receiver worker panicked: {reason}"),
                })
            }
        };
        let verifier = verifier_lock(&self.verifier);
        let mut report = verifier.report();
        let drained = observations["drained"].as_bool().unwrap_or(false);
        report["passed"] = json!(verifier.passed() && drained);
        report["transport"] = json!(self.transport.name());
        report["receiver_reads_per_pass"] = if self.transport.is_http() {
            Value::Null
        } else {
            json!(self.reads_per_pass)
        };
        let seconds = if verifier.passed() && drained {
            verifier.measurement_seconds()
        } else {
            None
        };
        report["measurement"] = json!({
            "method": "input-release-to-receiver-verification-v1",
            "status": if seconds.is_some() { "candidate" } else { "unavailable" },
            "seconds": seconds,
            "calibrated": false,
            "t0": "before publishing measured fixture input",
            "t1": "final expected record fully verified at receiver",
        });
        for (key, value) in observations
            .as_object()
            .expect("receiver observations object")
        {
            report[key] = value.clone();
        }
        Ok(report)
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Every read is nonblocking (TCP) or has a cancellation deadline (HTTP).
        // Setup failures therefore also release the listening port and worker.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Connection {
    stream: TcpStream,
    frame: Vec<u8>,
    beats: super::protocol::beats::Decoder,
    reply: Vec<u8>,
}

fn malformed(verifier: &Mutex<Verifier>, reason: impl AsRef<str>) {
    verifier_lock(verifier).malformed(reason.as_ref());
}

fn verifier_lock(verifier: &Mutex<Verifier>) -> MutexGuard<'_, Verifier> {
    match verifier.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.malformed(
                "receiver verifier lock was poisoned; recovered partial delivery counts",
            );
            verifier.clear_poison();
            guard
        }
    }
}

fn receive_tcp(
    listener: TcpListener,
    verifier: Arc<Mutex<Verifier>>,
    stop: Arc<AtomicBool>,
    reads_per_pass: usize,
    transport: Transport,
) -> Result<Value> {
    let mut connections: Vec<Connection> = Vec::new();
    let mut accepted = 0u64;
    let mut eof = 0u64;
    let mut wire_bytes = 0u64;
    let mut read_calls = 0u64;
    let mut data_reads = 0u64;
    let mut stopped_at = None;
    let mut quiet_since = Instant::now();
    let mut scratch = [0u8; 16 * 1024];

    loop {
        let now = Instant::now();
        if stop.load(Ordering::Acquire) && stopped_at.is_none() {
            stopped_at = Some(now);
            quiet_since = now;
        }
        let mut progress = false;
        // Accept again on every drain pass: the subject may have exited with
        // connections and bytes still queued in the kernel.
        for _ in 0..MAX_CONNECTIONS {
            match listener.accept() {
                Ok((stream, _)) => {
                    progress = true;
                    accepted += 1;
                    if connections.len() == MAX_CONNECTIONS {
                        malformed(&verifier, "TCP connection limit exceeded");
                        continue;
                    }
                    stream.set_nonblocking(true)?;
                    connections.push(Connection {
                        stream,
                        frame: Vec::new(),
                        beats: super::protocol::beats::Decoder::default(),
                        reply: Vec::new(),
                    });
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error).context("accept delivery TCP connection"),
            }
        }
        connections.retain_mut(|connection| {
            while !connection.reply.is_empty() {
                match connection.stream.write(&connection.reply) {
                    Ok(0) => {
                        malformed(&verifier, "native protocol ACK made no progress");
                        return false;
                    }
                    Ok(count) => {
                        connection.reply.drain(..count);
                        progress = true;
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(error) => {
                        malformed(&verifier, format!("native protocol ACK failed: {error}"));
                        return false;
                    }
                }
            }
            for _ in 0..reads_per_pass {
                read_calls += 1;
                match connection.stream.read(&mut scratch) {
                    Ok(0) => {
                        progress = true;
                        eof += 1;
                        if !connection.frame.is_empty()
                            || !connection.reply.is_empty()
                            || !connection.beats.complete()
                        {
                            malformed(&verifier, "TCP EOF before protocol frame or ACK completed");
                        }
                        return false;
                    }
                    Ok(length) => {
                        progress = true;
                        wire_bytes += length as u64;
                        data_reads += 1;
                        if matches!(
                            transport,
                            Transport::BeatsMessage
                                | Transport::BeatsParsed
                                | Transport::ForwardMessage
                                | Transport::ForwardParsed
                        ) {
                            if connection.frame.len() + length > MAX_BODY_BYTES {
                                malformed(&verifier, "native protocol frame exceeds 4 MiB");
                                return false;
                            }
                            connection.frame.extend_from_slice(&scratch[..length]);
                            loop {
                                let decoded = if matches!(
                                    transport,
                                    Transport::BeatsMessage | Transport::BeatsParsed
                                ) {
                                    connection.beats.decode(&connection.frame)
                                } else {
                                    super::protocol::forward::decode(&connection.frame)
                                };
                                match decoded {
                                    Ok(Some(frame)) => {
                                        if frame.consumed == 0
                                            || frame.consumed > connection.frame.len()
                                        {
                                            malformed(&verifier, "invalid native decoder progress");
                                            return false;
                                        }
                                        let mut verifier = verifier_lock(&verifier);
                                        for record in frame.records {
                                            verifier.record_json(&record);
                                        }
                                        if !verifier.has_errors() {
                                            connection.reply.extend_from_slice(&frame.reply);
                                        }
                                        connection.frame.drain(..frame.consumed);
                                    }
                                    Ok(None) => break,
                                    Err(error) => {
                                        malformed(
                                            &verifier,
                                            format!("native protocol decode failed: {error:#}"),
                                        );
                                        return false;
                                    }
                                }
                            }
                            if !connection.reply.is_empty() {
                                break;
                            }
                            continue;
                        }
                        for fragment in scratch[..length].split_inclusive(|byte| *byte == b'\n') {
                            if connection.frame.len() + fragment.len() > MAX_FRAME_BYTES {
                                malformed(&verifier, "TCP frame exceeds 64 KiB");
                                return false;
                            }
                            connection.frame.extend_from_slice(fragment);
                            if fragment.last() == Some(&b'\n') {
                                verifier_lock(&verifier)
                                    .record_line(&connection.frame[..connection.frame.len() - 1]);
                                connection.frame.clear();
                            }
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(error) => {
                        malformed(&verifier, format!("TCP read failed: {error}"));
                        return false;
                    }
                }
            }
            true
        });
        if progress {
            quiet_since = Instant::now();
        }
        if let Some(started) = stopped_at {
            let quiet = quiet_since.elapsed() >= DRAIN_QUIET;
            if (connections.is_empty() && quiet) || started.elapsed() >= DRAIN_LIMIT {
                let pending_bytes: usize = connections
                    .iter()
                    .map(|connection| connection.frame.len())
                    .sum();
                return Ok(json!({
                    "drained": connections.is_empty() && quiet && accepted == eof,
                    "drain_basis": "TCP EOF and quiet accept queue after subject exit",
                    "accepted_connections": accepted,
                    "eof_connections": eof,
                    "open_connections": connections.len(),
                    "pending_frame_bytes": pending_bytes,
                    "wire_bytes": wire_bytes,
                    "read_calls": read_calls,
                    "data_reads": data_reads,
                    "io_counters_scope": "whole_receiver_lifetime",
                }));
            }
        }
        if !progress {
            thread::sleep(POLL);
        }
    }
}

#[derive(Default)]
struct HttpStats {
    requests: u64,
    completed_bodies: u64,
    completed_body_bytes: u64,
    validated_requests: u64,
    active_bodies: usize,
    control_requests: u64,
}

// Also decremented when the connection future is cancelled at the drain deadline.
struct ActiveBody(Arc<Mutex<HttpStats>>);

impl Drop for ActiveBody {
    fn drop(&mut self) {
        self.0.lock().expect("HTTP stats poisoned").active_bodies -= 1;
    }
}

fn receive_http(
    listener: TcpListener,
    verifier: Arc<Mutex<Verifier>>,
    stop: Arc<AtomicBool>,
    transport: Transport,
) -> Result<Value> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .context("create delivery HTTP runtime")?;
    runtime.block_on(async move {
        let stats = Arc::new(Mutex::new(HttpStats::default()));
        let mut connections = JoinSet::new();
        let mut accepted = 0u64;
        let mut completed_connections = 0u64;
        let mut stopped_at = None;
        let mut quiet_since = Instant::now();
        loop {
            let now = Instant::now();
            if stop.load(Ordering::Acquire) && stopped_at.is_none() {
                stopped_at = Some(now);
                quiet_since = now;
            }
            let mut progress = false;
            while let Some(outcome) = connections.try_join_next() {
                completed_connections += 1;
                progress = true;
                if let Err(error) = outcome {
                    malformed(&verifier, format!("HTTP connection task failed: {error}"));
                }
            }
            for _ in 0..MAX_CONNECTIONS {
                match listener.accept() {
                    Ok((stream, _)) => {
                        progress = true;
                        accepted += 1;
                        if connections.len() == MAX_CONNECTIONS {
                            malformed(&verifier, "HTTP connection limit exceeded");
                            continue;
                        }
                        stream.set_nonblocking(true)?;
                        let stream = tokio::net::TcpStream::from_std(stream)?;
                        let verifier = verifier.clone();
                        let stats = stats.clone();
                        connections.spawn(async move {
                            let read_bytes = Arc::new(AtomicU64::new(0));
                            let io = TokioIo::new(ObservedStream {
                                stream,
                                read_bytes: read_bytes.clone(),
                            });
                            let service_verifier = verifier.clone();
                            let service = service_fn(move |request| {
                                http_request(request, service_verifier.clone(), stats.clone(), transport)
                            });
                            let outcome = http1::Builder::new()
                                .half_close(true)
                                .ignore_invalid_headers(false)
                                .max_headers(64)
                                .max_buf_size(MAX_FRAME_BYTES)
                                // An idle keep-alive connection is valid between
                                // canary, warmup and measured requests. The case
                                // deadline and bounded final drain own header
                                // cancellation; each request body is still timed.
                                .header_read_timeout(None)
                                .serve_connection(io, service)
                                .await;
                            // A TCP readiness probe can connect and close without
                            // sending an HTTP request. Other protocol errors fail.
                            if let Err(error) = outcome {
                                if read_bytes.load(Ordering::Relaxed) != 0 {
                                    malformed(&verifier, format!("HTTP protocol error: {error}"));
                                }
                            }
                        });
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error).context("accept delivery HTTP connection"),
                }
            }
            if progress {
                quiet_since = Instant::now();
            }
            if let Some(started) = stopped_at {
                let quiet = quiet_since.elapsed() >= DRAIN_QUIET;
                if (connections.is_empty() && quiet) || started.elapsed() >= DRAIN_LIMIT {
                    let open_connections = connections.len();
                    let observations = {
                        let stats = stats.lock().expect("HTTP stats poisoned");
                        json!({
                        "drained": open_connections == 0 && stats.active_bodies == 0 && stats.completed_bodies == stats.requests && quiet,
                        "drain_basis": "completed HTTP bodies, finished connection tasks and quiet accept queue after subject exit",
                        "accepted_connections": accepted,
                        "completed_connections": completed_connections,
                        "open_connections": open_connections,
                        "requests": stats.requests,
                        "control_requests": stats.control_requests,
                        "completed_request_bodies": stats.completed_bodies,
                        "completed_request_body_bytes": stats.completed_body_bytes,
                        "io_counters_scope": "whole_receiver_lifetime",
                        "validated_requests": stats.validated_requests,
                        "pending_request_bodies": stats.active_bodies,
                        })
                    };
                    // JoinSet aborts and drops its tasks, including all sockets.
                    // No blocking body read can hold up receiver shutdown.
                    connections.abort_all();
                    while connections.join_next().await.is_some() {}
                    return Ok(observations);
                }
            }
            tokio::time::sleep(POLL).await;
        }
    })
}

async fn http_request(
    request: Request<Incoming>,
    verifier: Arc<Mutex<Verifier>>,
    stats: Arc<Mutex<HttpStats>>,
    transport: Transport,
) -> std::result::Result<Response<Full<Bytes>>, Infallible> {
    stats.lock().expect("HTTP stats poisoned").requests += 1;
    let reject = |status, reason: &str| {
        malformed(&verifier, reason);
        response(status)
    };
    let elastic = matches!(transport, Transport::ElasticBulk);
    let control = if elastic && request.method() == Method::GET {
        super::protocol::elastic::handshake(request.uri().path())
    } else {
        None
    };
    let expected_path = if elastic { "/_bulk" } else { "/" };
    if control.is_none()
        && (request.method() != Method::POST || request.uri().path() != expected_path)
    {
        return Ok(reject(
            StatusCode::BAD_REQUEST,
            "unexpected HTTP method or path",
        ));
    }
    if request
        .headers()
        .get_all(hyper::header::CONTENT_ENCODING)
        .iter()
        .any(|encoding| !encoding.as_bytes().eq_ignore_ascii_case(b"identity"))
    {
        return Ok(reject(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "compressed HTTP bodies are unsupported",
        ));
    }
    if request
        .headers()
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok())
        .and_then(|length| length.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_BODY_BYTES as u64)
    {
        return Ok(reject(
            StatusCode::PAYLOAD_TOO_LARGE,
            "HTTP body exceeds 4 MiB",
        ));
    }
    stats.lock().expect("HTTP stats poisoned").active_bodies += 1;
    let _active = ActiveBody(stats.clone());
    let body = match tokio::time::timeout(
        REQUEST_LIMIT,
        Limited::new(request.into_body(), MAX_BODY_BYTES).collect(),
    )
    .await
    {
        Ok(Ok(body)) => body.to_bytes(),
        Ok(Err(error)) => {
            return Ok(reject(
                StatusCode::BAD_REQUEST,
                &format!("HTTP body read failed: {error}"),
            ));
        }
        Err(_) => {
            return Ok(reject(
                StatusCode::REQUEST_TIMEOUT,
                "HTTP body read timed out",
            ));
        }
    };
    {
        let mut stats = stats.lock().expect("HTTP stats poisoned");
        stats.completed_bodies += 1;
        stats.completed_body_bytes += body.len() as u64;
    }
    if let Some(control) = control {
        if !body.is_empty() {
            return Ok(reject(
                StatusCode::BAD_REQUEST,
                "protocol handshake must have an empty body",
            ));
        }
        stats.lock().expect("HTTP stats poisoned").control_requests += 1;
        return Ok(elastic_response(StatusCode::OK, &control));
    }
    let mut verifier = verifier_lock(&verifier);
    if elastic {
        let (records, responses) = match super::protocol::elastic::decode_bulk(&body) {
            Ok(decoded) => decoded,
            Err(error) => {
                verifier.malformed(&format!("invalid Bulk request: {error:#}"));
                return Ok(response(StatusCode::BAD_REQUEST));
            }
        };
        for record in records {
            verifier.record_json(&record);
        }
        if verifier.has_errors() {
            return Ok(response(StatusCode::BAD_REQUEST));
        }
        stats
            .lock()
            .expect("HTTP stats poisoned")
            .validated_requests += 1;
        return Ok(elastic_response(
            StatusCode::OK,
            &json!({"errors":false,"took":0,"items":responses}),
        ));
    }
    let mut records = 0usize;
    // Vector can use one JSON array per request or a stream of JSON values.
    // The body and parser depth are bounded; raw output is discarded immediately.
    for value in serde_json::Deserializer::from_slice(&body).into_iter::<Value>() {
        match value {
            Ok(Value::Array(batch)) => {
                for value in batch {
                    verifier.record_json(&value);
                    records += 1;
                }
            }
            Ok(value) => {
                verifier.record_json(&value);
                records += 1;
            }
            Err(error) => {
                verifier.malformed(&format!("invalid HTTP JSON body: {error}"));
                break;
            }
        }
    }
    if records == 0 {
        verifier.malformed("HTTP body contains no JSON records");
    }
    let status = if verifier.has_errors() {
        StatusCode::BAD_REQUEST
    } else {
        stats
            .lock()
            .expect("HTTP stats poisoned")
            .validated_requests += 1;
        StatusCode::OK
    };
    Ok(response(status))
}

fn response(status: StatusCode) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::new()));
    *response.status_mut() = status;
    if !status.is_success() {
        response.headers_mut().insert(
            hyper::header::CONNECTION,
            hyper::header::HeaderValue::from_static("close"),
        );
    }
    response
}

fn elastic_response(status: StatusCode, value: &Value) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from(
        serde_json::to_vec(value).expect("serializable protocol response"),
    )));
    *response.status_mut() = status;
    response.headers_mut().insert(
        "x-elastic-product",
        hyper::header::HeaderValue::from_static("Elasticsearch"),
    );
    response.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/json"),
    );
    response
}

// Count bytes without retaining traffic, to distinguish an empty readiness
// connection from an incomplete/malformed HTTP request reported by Hyper.
struct ObservedStream {
    stream: tokio::net::TcpStream,
    read_bytes: Arc<AtomicU64>,
}

impl AsyncRead for ObservedStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        let outcome = Pin::new(&mut this.stream).poll_read(cx, buf);
        this.read_bytes
            .fetch_add((buf.filled().len() - before) as u64, Ordering::Relaxed);
        outcome
    }
}

impl AsyncWrite for ObservedStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn http_keepalive_can_idle_but_incomplete_next_request_fails_drain() {
        for partial_next_request in [false, true] {
            let receiver = Receiver::start_values(
                vec![json!({"message":"one"}), json!({"message":"two"})],
                Format::ApplicationJson,
                None,
                Transport::HttpJson,
                16,
            )
            .unwrap();
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, receiver.port())).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            for message in ["one", "two"] {
                let body = json!({"message":message}).to_string();
                write!(
                    stream,
                    "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
                let mut response = Vec::new();
                while !response.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    response.push(byte[0]);
                }
                assert!(response.starts_with(b"HTTP/1.1 200"));
                if message == "one" && !partial_next_request {
                    thread::sleep(REQUEST_LIMIT + Duration::from_millis(100));
                }
            }
            if partial_next_request {
                stream.write_all(b"POST / HTTP/1.1\r\nHost:").unwrap();
            } else {
                drop(stream);
            }
            let report = receiver.finish().unwrap();
            assert_eq!(report["passed"], !partial_next_request, "{report}");
            assert_eq!(report["unique"], 2);
        }
    }

    #[test]
    fn wire_delivery_drains_fragments_and_rejects_late_duplicates_and_http_errors() {
        let fixture = Fixture::new(2, 0, "receiver-wire-test".to_owned());
        assert!(Receiver::start_with_reads(fixture.clone(), Transport::Text, 0).is_err());
        assert!(Receiver::start_with_reads(fixture.clone(), Transport::Text, 1025).is_err());
        for transport in [Transport::Text, Transport::ParsedJson, Transport::HttpJson] {
            let reads_per_pass = match transport {
                Transport::Text => 1,
                Transport::ParsedJson => 64,
                Transport::HttpJson => 16,
                _ => unreachable!("basic transport fixture"),
            };
            let receiver =
                Receiver::start_with_reads(fixture.clone(), transport, reads_per_pass).unwrap();
            let sent_bytes = std::cell::Cell::new(0u64);
            // Empty readiness connections are allowed for every transport.
            drop(TcpStream::connect((Ipv4Addr::LOCALHOST, receiver.port())).unwrap());
            let send = |ids: std::ops::Range<usize>| {
                let mut stream =
                    TcpStream::connect((Ipv4Addr::LOCALHOST, receiver.port())).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut body = Vec::new();
                for id in ids {
                    match transport {
                        Transport::Text => body.extend(fixture.record(id)),
                        Transport::ParsedJson => {
                            body.extend(serde_json::to_vec(&fixture.parsed(id)).unwrap());
                            body.push(b'\n');
                        }
                        Transport::HttpJson => {
                            let record = fixture.record(id);
                            body.extend(
                                serde_json::to_vec(&json!({
                                    "message": std::str::from_utf8(&record[..record.len() - 1]).unwrap()
                                }))
                                .unwrap(),
                            );
                            body.push(b'\n');
                        }
                        _ => unreachable!("basic transport fixture"),
                    }
                }
                sent_bytes.set(sent_bytes.get() + body.len() as u64);
                if matches!(transport, Transport::HttpJson) {
                    stream.write_all(b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
                    for chunk in body.chunks(7) {
                        write!(stream, "{:x}\r\n", chunk.len()).unwrap();
                        stream.write_all(chunk).unwrap();
                        stream.write_all(b"\r\n").unwrap();
                    }
                    stream.write_all(b"0\r\n\r\n").unwrap();
                    let mut response = String::new();
                    stream.read_to_string(&mut response).unwrap();
                    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
                } else {
                    for chunk in body.chunks(7) {
                        stream.write_all(chunk).unwrap();
                    }
                }
            };
            send(0..1);
            let deadline = Instant::now() + Duration::from_secs(2);
            while receiver.unique() != 1 && Instant::now() < deadline {
                thread::sleep(POLL);
            }
            assert_eq!(receiver.unique(), 1);
            assert!(verifier_lock(&receiver.verifier)
                .measurement_seconds()
                .is_none());
            receiver.start_measurement().unwrap();
            assert!(verifier_lock(&receiver.verifier)
                .measurement_seconds()
                .is_none());
            send(1..fixture.total());
            let report = receiver.finish().unwrap();
            assert_eq!(report["passed"], true, "{report}");
            assert_eq!(report["drained"], true, "{report}");
            assert_eq!(report["open_connections"], 0);
            if matches!(transport, Transport::HttpJson) {
                assert!(report["receiver_reads_per_pass"].is_null());
                assert_eq!(report["completed_request_body_bytes"], sent_bytes.get());
            } else {
                assert_eq!(report["receiver_reads_per_pass"], reads_per_pass);
                assert_eq!(report["wire_bytes"], sent_bytes.get());
                let data_reads = report["data_reads"].as_u64().unwrap();
                assert!(data_reads > 0);
                assert!(report["read_calls"].as_u64().unwrap() >= data_reads);
            }
            assert_eq!(report["measurement"]["status"], "candidate", "{report}");
            assert!(
                report["measurement"]["seconds"]
                    .as_f64()
                    .is_some_and(|seconds| seconds > 0.0),
                "{report}"
            );
            assert_eq!(report["measurement"]["calibrated"], false);
        }

        let receiver = Receiver::start(fixture.clone(), Transport::Text).unwrap();
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, receiver.port())).unwrap();
        stream.write_all(&fixture.record(0)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while receiver.unique() != 1 && Instant::now() < deadline {
            thread::sleep(POLL);
        }
        assert_eq!(receiver.unique(), 1);
        receiver.start_measurement().unwrap();
        for id in 1..fixture.total() {
            stream.write_all(&fixture.record(id)).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while receiver.unique() != fixture.total() && Instant::now() < deadline {
            thread::sleep(POLL);
        }
        assert_eq!(receiver.unique(), fixture.total());
        assert!(verifier_lock(&receiver.verifier)
            .measurement_seconds()
            .is_some());
        // The final ID is not sufficient: this queued duplicate must be seen
        // by finish() even though the initial completeness check already passed.
        stream.write_all(&fixture.record(0)).unwrap();
        drop(stream);
        let report = receiver.finish().unwrap();
        assert_eq!(report["passed"], false, "{report}");
        assert_eq!(report["duplicates"], 1, "{report}");
        assert!(report["measurement"]["seconds"].is_null(), "{report}");
        assert_eq!(report["measurement"]["status"], "unavailable");

        let receiver = Receiver::start(fixture.clone(), Transport::HttpJson).unwrap();
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, receiver.port())).unwrap();
        stream
            .write_all(b"POST / HTTP/1.1\r\nmalformed header\r\n\r\n")
            .unwrap();
        drop(stream);
        let report = receiver.finish().unwrap();
        assert_eq!(report["passed"], false, "{report}");
        assert!(report["malformed"].as_u64().unwrap() > 0, "{report}");

        // Finalization preserves the delivery snapshot even on a worker error
        // or a panic while the worker held the verifier lock.
        for panic in [false, true] {
            let verifier = Arc::new(Mutex::new(Verifier::new(fixture.clone(), Format::Text)));
            verifier_lock(&verifier).record_line(&fixture.record(0));
            verifier_lock(&verifier).start_measurement().unwrap();
            for id in 1..fixture.total() {
                verifier_lock(&verifier).record_line(&fixture.record(id));
            }
            let worker_verifier = verifier.clone();
            let receiver = Receiver {
                port: 0,
                transport: Transport::Text,
                reads_per_pass: 16,
                verifier,
                stop: Arc::new(AtomicBool::new(false)),
                worker: Some(thread::spawn(move || {
                    if panic {
                        let _guard = worker_verifier.lock().unwrap();
                        panic!("injected receiver failure");
                    }
                    Err(anyhow::anyhow!("injected receiver failure"))
                })),
            };
            let report = receiver.finish().unwrap();
            assert_eq!(report["unique"], fixture.total(), "{report}");
            assert_eq!(report["passed"], false, "{report}");
            assert_eq!(report["drained"], false, "{report}");
            assert!(report["measurement"]["seconds"].is_null(), "{report}");
            assert!(report["drain_error"]
                .as_str()
                .unwrap()
                .contains("injected receiver failure"));
            if panic {
                assert!(report["malformed"].as_u64().unwrap() > 0, "{report}");
            }
        }
    }
}

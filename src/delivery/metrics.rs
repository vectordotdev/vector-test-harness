//! Bounded reads of native output counters for count-only discard diagnostics.

use std::{fs, net::SocketAddr, path::Path, time::Duration};

use anyhow::{ensure, Context, Result};
use http_body_util::{BodyExt, Empty, Limited};
use hyper::{body::Bytes, Request, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::Value;

use super::subject::Subject;

pub(super) fn count(subject: Subject, output: &Path) -> Result<Option<u64>> {
    if matches!(subject, Subject::SplunkHf | Subject::SplunkUf) {
        return super::adapters::splunk::count(output);
    }
    let path = output.join("metrics-endpoint.json");
    if !path.exists() {
        return Ok(None);
    }
    let endpoint: Value = serde_json::from_slice(&fs::read(path)?)?;
    let url = endpoint["url"]
        .as_str()
        .context("native metrics URL missing")?;
    let (address, path) = url
        .strip_prefix("http://")
        .and_then(|text| text.split_once('/'))
        .context("invalid native metrics URL")?;
    let address: SocketAddr = address.parse()?;
    ensure!(
        address.ip().is_loopback(),
        "native metrics must use loopback"
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let body = runtime.block_on(async {
        tokio::time::timeout(Duration::from_millis(100), async {
            let stream = tokio::net::TcpStream::connect(address).await?;
            let (mut sender, connection) =
                hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
            let task = tokio::spawn(connection);
            let request = Request::builder()
                .uri(format!("/{path}"))
                .header("Host", address.to_string())
                .header("Connection", "close")
                .body(Empty::<Bytes>::new())?;
            let response = sender.send_request(request).await?;
            if response.status() != StatusCode::OK {
                return Ok::<_, anyhow::Error>(None);
            }
            let body = Limited::new(response.into_body(), 1024 * 1024)
                .collect()
                .await
                .map_err(|error| anyhow::anyhow!("native metrics body: {error}"))?
                .to_bytes();
            drop(sender);
            let _ = task.await;
            Ok(Some(body))
        })
        .await
    });
    let body = match body {
        Ok(Ok(Some(body))) => body,
        _ => return Ok(None),
    };
    let value: Value =
        serde_json::from_slice(&body).context("invalid native output metrics JSON")?;
    // Logstash opens its API before the main pipeline registers its plugins.
    // This is a readiness sample, not a terminal count of zero.
    if subject == Subject::Logstash
        && value["pipelines"]["main"]["plugins"]["outputs"]
            .as_array()
            .is_none_or(|outputs| outputs.is_empty())
    {
        return Ok(None);
    }
    let count = match subject {
        Subject::FluentBit => value["output"]
            .as_object()
            .and_then(|outputs| {
                outputs
                    .iter()
                    .find(|(name, _)| name.as_str() == "destination" || name.starts_with("null."))
                    .map(|(_, stats)| stats)
            })
            .and_then(|stats| stats["proc_records"].as_u64()),
        Subject::Fluentd => value["plugins"]
            .as_array()
            .and_then(|plugins| {
                plugins
                    .iter()
                    .find(|plugin| plugin["plugin_id"] == "destination" && plugin["type"] == "null")
            })
            .and_then(|plugin| plugin["emit_records"].as_u64()),
        Subject::Logstash => value["pipelines"]["main"]["plugins"]["outputs"]
            .as_array()
            .and_then(|outputs| {
                outputs
                    .iter()
                    .find(|plugin| plugin["id"] == "destination" && plugin["name"] == "null")
            })
            .and_then(|plugin| plugin["events"]["out"].as_u64()),
        _ => None,
    };
    let count = count.context("native discard output counter missing")?;
    let receipt = serde_json::json!({"count":count,"native":value,"basis":"last native completed-output counter observed before shutdown"});
    let destination = output.join("native-metrics.json");
    let previous = fs::read(&destination)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    if previous.as_ref().and_then(|value| value["count"].as_u64()) != Some(count) {
        let temporary = output.join("native-metrics.json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&receipt)?)?;
        fs::rename(temporary, destination)?;
    }
    Ok(Some(count))
}

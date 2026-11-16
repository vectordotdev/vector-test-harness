use std::path::Path;

use serde_json::Value;

use crate::runtime::Invocation;

pub(in crate::delivery) fn configure(
    launch: Invocation,
    output: &Path,
    config: &Path,
    _timeout: u64,
) -> Invocation {
    launch.args([
        "-e".to_owned(),
        "-c".into(),
        config.to_string_lossy().into_owned(),
        "--path.config".into(),
        output.to_string_lossy().into_owned(),
        "--path.data".into(),
        output.join("data").to_string_lossy().into_owned(),
        "--path.logs".into(),
        output.join("logs").to_string_lossy().into_owned(),
    ])
}

pub(in crate::delivery) fn version_args() -> &'static [&'static str] {
    &["version"]
}

pub(in crate::delivery) fn identifies(version: &str) -> bool {
    version
        .strip_prefix("filebeat version ")
        .is_some_and(|value| {
            value
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        })
}

pub(in crate::delivery) fn error(line: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<Value>(line).ok();
    if let Some(value) = value {
        if matches!(value["log.level"].as_str(), Some("error" | "fatal")) {
            return Some(
                value["message"]
                    .as_str()
                    .unwrap_or("Filebeat logged an error")
                    .chars()
                    .take(1024)
                    .collect(),
            );
        }
    }
    let text = String::from_utf8_lossy(line);
    text.strip_prefix("Exiting:")
        .map(|message| message.trim().chars().take(1024).collect())
}

pub(in crate::delivery) fn count_delta(line: &[u8]) -> Option<u64> {
    let value: Value = serde_json::from_slice(line).ok()?;
    if !value["message"]
        .as_str()?
        .starts_with("Non-zero metrics in the last ")
    {
        return None;
    }
    value
        .pointer("/monitoring/metrics/libbeat/output/events/acked")?
        .as_u64()
}

pub(in crate::delivery) fn count_total(line: &[u8]) -> Option<u64> {
    let value: Value = serde_json::from_slice(line).ok()?;
    if value["message"] != "Total metrics"
        || value
            .pointer("/monitoring/metrics/libbeat/output/type")?
            .as_str()?
            != "discard"
    {
        return None;
    }
    value
        .pointer("/monitoring/metrics/libbeat/output/events/acked")?
        .as_u64()
}

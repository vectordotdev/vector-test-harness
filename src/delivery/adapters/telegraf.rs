use std::path::Path;

use crate::runtime::Invocation;

pub(in crate::delivery) fn configure(
    launch: Invocation,
    _output: &Path,
    config: &Path,
    _timeout: u64,
) -> Invocation {
    launch
        .args(["--config".to_owned(), config.to_string_lossy().into_owned()])
        .env("GOMAXPROCS", "2")
}

pub(in crate::delivery) fn version_args() -> &'static [&'static str] {
    &["version"]
}

pub(in crate::delivery) fn identifies(version: &str) -> bool {
    version.strip_prefix("Telegraf ").is_some_and(|value| {
        value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    })
}

pub(in crate::delivery) fn error(line: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(line);
    let (_, message) = text.split_once(' ')?;
    if message.starts_with("W! [outputs.")
        && message.contains("] Metric buffer overflow; ")
        && message.contains(" metrics have been dropped")
    {
        return Some(message.trim().chars().take(1024).collect());
    }
    message
        .strip_prefix("E! ")
        .or_else(|| message.strip_prefix("F! "))
        .map(|message| message.trim().chars().take(1024).collect())
}

pub(in crate::delivery) fn count_delta(line: &[u8]) -> Option<u64> {
    let text = std::str::from_utf8(line).ok()?;
    let (_, message) = text.split_once(" D! [outputs.discard] Wrote batch of ")?;
    let (count, _) = message.split_once(" metrics in ")?;
    count.parse().ok()
}

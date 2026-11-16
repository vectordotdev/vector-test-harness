use std::path::Path;

use crate::runtime::Invocation;

pub(in crate::delivery) fn configure(
    launch: Invocation,
    _output: &Path,
    config: &Path,
    _timeout_secs: u64,
) -> Invocation {
    let mut invocation = launch.args([
        "--no-supervisor".to_owned(),
        "--suppress-config-dump".into(),
        "--config".into(),
        config.to_string_lossy().into_owned(),
    ]);
    // The caller may supply GEM_HOME/GEM_PATH for an isolated installation.
    invocation.unset_env = ["RUBYOPT", "FLUENT_CONF", "FLUENT_PLUGIN"]
        .map(String::from)
        .into();
    invocation
}

pub(in crate::delivery) fn version_args() -> &'static [&'static str] {
    &["--version"]
}

pub(in crate::delivery) fn configure_reload(mut invocation: Invocation) -> Invocation {
    // SIGHUP is handled by the supervisor; workers are restarted beneath it.
    invocation
        .args
        .retain(|argument| argument != "--no-supervisor");
    invocation
}

pub(in crate::delivery) fn identifies(version: &str) -> bool {
    version.lines().any(|line| {
        line.strip_prefix("fluentd ")
            .and_then(|value| value.chars().next())
            .is_some_and(|character| character.is_ascii_digit())
    })
}

pub(in crate::delivery) fn error(line: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(line).ok()?;
    let level = value.get("level")?.as_str()?;
    let message = value["message"]
        .as_str()
        .unwrap_or("Fluentd logged an error");
    let shutdown_exception = level.eq_ignore_ascii_case("warn")
        && message.starts_with("unexpected error while shutting down on input plugin ")
        && message.contains("error_class=NoMethodError");
    (level.eq_ignore_ascii_case("error")
        || level.eq_ignore_ascii_case("fatal")
        || shutdown_exception)
        .then(|| message.chars().take(1024).collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_shutdown_exception_is_unhealthy_even_when_logged_as_warning() {
        assert!(super::error(br#"{"level":"warn","message":"unexpected error while shutting down on input plugin plugin=Fluent::StdinInput error_class=NoMethodError error=undefined method bind_call for nil"}"#).is_some());
        assert!(super::error(br#"{"level":"warn","message":"super was not called in #shutdown: calling it forcedly plugin=Fluent::StdinInput"}"#).is_none());
        assert!(super::error(br#"{"level":"info","message":"unexpected error while shutting down on input plugin error_class=NoMethodError"}"#).is_none());
    }
}

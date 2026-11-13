//! Product-specific launch and log handling; delivery verification remains shared.

use std::path::Path;

use crate::runtime::Invocation;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Subject {
    Vector,
    FluentBit,
}

impl Subject {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Vector => "vector",
            Self::FluentBit => "fluent-bit",
        }
    }

    pub(super) fn display_name(self) -> &'static str {
        match self {
            Self::Vector => "Vector",
            Self::FluentBit => "Fluent Bit",
        }
    }

    pub(super) fn config_name(self) -> &'static str {
        match self {
            Self::Vector => "vector.yaml",
            Self::FluentBit => "fluent-bit.yaml",
        }
    }

    pub(super) fn identifies(self, version: &str) -> bool {
        let prefix = match self {
            Self::Vector => "vector ",
            Self::FluentBit => "Fluent Bit v",
        };
        version.starts_with(prefix)
            && version[prefix.len()..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
    }

    pub(super) fn invocation(
        self,
        binary: &Path,
        output: &Path,
        config: &Path,
        cpus: Option<&str>,
        timeout_secs: u64,
    ) -> Invocation {
        let launch = if let Some(cpus) = cpus {
            Invocation::new("taskset", output).args([
                "--cpu-list".to_owned(),
                cpus.to_owned(),
                binary.to_string_lossy().into_owned(),
            ])
        } else {
            Invocation::new(&binary.to_string_lossy(), output)
        };
        let mut invocation =
            launch.args(["--config".to_owned(), config.to_string_lossy().into_owned()]);
        match self {
            Self::Vector => {
                invocation = invocation
                    .args([
                        "--log-format".to_owned(),
                        "json".into(),
                        "--color".into(),
                        "never".into(),
                        "--threads".into(),
                        "2".into(),
                        "--require-healthy".into(),
                        "true".into(),
                        "--graceful-shutdown-limit-secs".into(),
                        timeout_secs.to_string(),
                    ])
                    .env("VECTOR_LOG", "info");
                invocation.unset_env = [
                    "VECTOR_CONFIG",
                    "VECTOR_CONFIG_DIR",
                    "VECTOR_CONFIG_TOML",
                    "VECTOR_CONFIG_JSON",
                    "VECTOR_CONFIG_YAML",
                    "VECTOR_WATCH_CONFIG",
                    "VECTOR_NO_GRACEFUL_SHUTDOWN_LIMIT",
                    "RUST_LOG",
                ]
                .map(String::from)
                .into();
            }
            Self::FluentBit => {
                // This environment variable overrides the configured log level.
                invocation.unset_env = vec!["FLB_LOG_LEVEL".into()];
            }
        }
        invocation
    }

    pub(super) fn text_error(self, line: &[u8]) -> Option<String> {
        if self != Self::FluentBit {
            return None;
        }
        let text = String::from_utf8_lossy(line);
        // Fluent Bit's non-TTY logger emits "[timestamp] [level] message".
        let (timestamp, remainder) = text.trim_start().strip_prefix('[')?.split_once(']')?;
        if timestamp.is_empty() {
            return None;
        }
        let (level, message) = remainder.trim_start().strip_prefix('[')?.split_once(']')?;
        matches!(level.trim(), "error" | "fatal")
            .then(|| message.trim().chars().take(1024).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluent_bit_identity_and_error_logs_are_distinct_from_vector() {
        assert!(Subject::FluentBit.identifies("Fluent Bit v5.2.0"));
        assert!(!Subject::FluentBit.identifies("vector 0.61.0"));
        assert!(!Subject::FluentBit.identifies("Fluent Bit v"));
        assert!(!Subject::Vector.identifies("Fluent Bit v5.2.0"));
        assert_eq!(
            Subject::FluentBit.text_error(b"[2026/11/13 17:00:00] [error] failed to flush\n"),
            Some("failed to flush".into())
        );
        assert!(Subject::FluentBit
            .text_error(b"[2026/11/13 17:00:00] [ info] message containing [error]")
            .is_none());
        let invocation = Subject::FluentBit.invocation(
            Path::new("/bin/fluent-bit"),
            Path::new("/tmp/run"),
            Path::new("/tmp/run/fluent-bit.yaml"),
            Some("2,3"),
            30,
        );
        assert_eq!(invocation.program, "taskset");
        assert_eq!(invocation.args[2], "/bin/fluent-bit");
        assert_eq!(invocation.unset_env, ["FLB_LOG_LEVEL"]);
    }
}

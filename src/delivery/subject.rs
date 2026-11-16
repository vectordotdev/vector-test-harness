//! Product-specific launch and log handling; delivery verification remains shared.

use std::path::Path;

use super::{adapters, receiver::Transport};
use crate::cli::CheckCase;
use crate::runtime::Invocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Subject {
    Vector,
    #[value(name = "fluent-bit", alias = "fluentbit")]
    FluentBit,
    Telegraf,
    Filebeat,
    Fluentd,
    Logstash,
    #[value(name = "splunk-hf", alias = "splunk_heavy_forwarder")]
    SplunkHf,
    #[value(name = "splunk-uf", alias = "splunk_universal_forwarder")]
    SplunkUf,
}

impl Subject {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Vector => "vector",
            Self::FluentBit => "fluent-bit",
            Self::Telegraf => "telegraf",
            Self::Filebeat => "filebeat",
            Self::Fluentd => "fluentd",
            Self::Logstash => "logstash",
            Self::SplunkHf => "splunk-hf",
            Self::SplunkUf => "splunk-uf",
        }
    }

    pub(super) fn display_name(self) -> &'static str {
        match self {
            Self::Vector => "Vector",
            Self::FluentBit => "Fluent Bit",
            Self::Telegraf => "Telegraf",
            Self::Filebeat => "Filebeat",
            Self::Fluentd => "Fluentd",
            Self::Logstash => "Logstash",
            Self::SplunkHf => "Splunk Heavy Forwarder",
            Self::SplunkUf => "Splunk Universal Forwarder",
        }
    }

    pub(super) fn config_name(self) -> &'static str {
        match self {
            Self::Vector => "vector.yaml",
            Self::FluentBit => "fluent-bit.yaml",
            Self::Telegraf => "telegraf.toml",
            Self::Filebeat => "filebeat.yml",
            Self::Fluentd => "fluentd.conf",
            Self::Logstash => "logstash.conf",
            Self::SplunkHf => "splunk-hf.conf",
            Self::SplunkUf => "splunk-uf.conf",
        }
    }

    pub(super) fn identifies(self, version: &str) -> bool {
        match self {
            Self::Telegraf => return adapters::telegraf::identifies(version),
            Self::Filebeat => return adapters::filebeat::identifies(version),
            Self::Fluentd => return adapters::fluentd::identifies(version),
            Self::Logstash => return adapters::logstash::identifies(version),
            Self::SplunkHf => return adapters::splunk::identifies(version, false),
            Self::SplunkUf => return adapters::splunk::identifies(version, true),
            _ => {}
        }
        let prefix = match self {
            Self::Vector => "vector ",
            Self::FluentBit => "Fluent Bit v",
            Self::Telegraf => "Telegraf ",
            Self::Filebeat => "filebeat version ",
            Self::Fluentd => "fluentd ",
            Self::Logstash => "logstash ",
            Self::SplunkHf | Self::SplunkUf => "Splunk ",
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
        let mut invocation = match self {
            Self::Telegraf => {
                return adapters::telegraf::configure(launch, output, config, timeout_secs)
            }
            Self::Filebeat => {
                return adapters::filebeat::configure(launch, output, config, timeout_secs)
            }
            Self::Fluentd => {
                return adapters::fluentd::configure(launch, output, config, timeout_secs)
            }
            Self::Logstash => {
                return adapters::logstash::configure(launch, output, config, timeout_secs)
            }
            Self::SplunkHf | Self::SplunkUf => {
                return adapters::splunk::configure(launch, output, config, timeout_secs)
            }
            _ => launch.args(["--config".to_owned(), config.to_string_lossy().into_owned()]),
        };
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
            _ => {}
        }
        invocation
    }

    pub(super) fn text_error(self, line: &[u8]) -> Option<String> {
        match self {
            Self::Telegraf => return adapters::telegraf::error(line),
            Self::Filebeat => return adapters::filebeat::error(line),
            Self::Fluentd => return adapters::fluentd::error(line),
            Self::Logstash => return adapters::logstash::error(line),
            Self::SplunkHf | Self::SplunkUf => return adapters::splunk::error(line),
            _ => {}
        }
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

    pub(super) fn version_args(self) -> &'static [&'static str] {
        match self {
            Self::Telegraf => adapters::telegraf::version_args(),
            Self::Filebeat => adapters::filebeat::version_args(),
            Self::Fluentd => adapters::fluentd::version_args(),
            Self::Logstash => adapters::logstash::version_args(),
            Self::SplunkHf | Self::SplunkUf => adapters::splunk::version_args(),
            _ => &["--version"],
        }
    }

    pub(super) fn transport(self, case: CheckCase) -> Transport {
        match (self, case) {
            (Self::Filebeat, CheckCase::TcpToHttp) => Transport::ElasticBulk,
            (Self::Filebeat, CheckCase::RegexParsing) => Transport::BeatsParsed,
            (Self::Filebeat, _) => Transport::BeatsMessage,
            (Self::Fluentd, CheckCase::TcpToHttp) => Transport::HttpJson,
            (Self::Fluentd, CheckCase::RegexParsing) => Transport::ForwardParsed,
            (Self::Fluentd, _) => Transport::ForwardMessage,
            (Self::FluentBit, CheckCase::TcpToHttp) => Transport::HttpLogJson,
            (_, CheckCase::TcpToHttp) => Transport::HttpJson,
            (_, CheckCase::RegexParsing) => Transport::ParsedJson,
            _ => Transport::Text,
        }
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

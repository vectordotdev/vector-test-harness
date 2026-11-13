use std::{fs, net::TcpListener, path::Path};

use anyhow::Result;
use serde_json::json;

use crate::runtime::{sha256, Invocation};

pub(in crate::delivery) fn prepare(output: &Path) -> Result<()> {
    let settings = output.join("settings");
    fs::create_dir_all(&settings)?;
    let reservation = TcpListener::bind(("127.0.0.1", 0))?;
    let port = reservation.local_addr()?.port();
    fs::write(
        settings.join("logstash.yml"),
        format!(
            "allow_superuser: true\npipeline.workers: 2\npipeline.batch.size: 256\npipeline.batch.delay: 50\n\
             pipeline.ecs_compatibility: disabled\nqueue.type: memory\n\
             api.enabled: true\napi.http.host: 127.0.0.1\napi.http.port: {port}\n\
             xpack.monitoring.enabled: false\n"
        ),
    )?;
    fs::write(
        settings.join("jvm.options"),
        "-Xms256m\n-Xmx256m\n-Djava.awt.headless=true\n-Dfile.encoding=UTF-8\n\
         -Djava.security.egd=file:/dev/urandom\n",
    )?;
    fs::write(
        settings.join("log4j2.properties"),
        "status = error\nname = Harness\nappender.console.type = Console\n\
         appender.console.name = json_console\nappender.console.layout.type = JSONLayout\n\
         appender.console.layout.compact = true\nappender.console.layout.eventEol = true\n\
         rootLogger.level = info\nrootLogger.appenderRef.console.ref = json_console\n",
    )?;
    fs::write(
        output.join("metrics-endpoint.json"),
        serde_json::to_vec_pretty(&json!({
            "url":format!("http://127.0.0.1:{port}/_node/stats/pipelines/main"),
            "basis":"native output-plugin counter; query before process shutdown",
        }))?,
    )?;
    fs::write(
        settings.join("sha256.json"),
        serde_json::to_vec_pretty(&json!({
            "logstash.yml":sha256(&settings.join("logstash.yml"))?,
            "jvm.options":sha256(&settings.join("jvm.options"))?,
            "log4j2.properties":sha256(&settings.join("log4j2.properties"))?,
        }))?,
    )?;
    Ok(())
}

pub(in crate::delivery) fn configure(
    launch: Invocation,
    output: &Path,
    config: &Path,
    _timeout_secs: u64,
) -> Invocation {
    let mut invocation = launch.args([
        "--path.settings".to_owned(),
        output.join("settings").to_string_lossy().into_owned(),
        "--path.data".into(),
        output.join("data").to_string_lossy().into_owned(),
        "--path.logs".into(),
        output.to_string_lossy().into_owned(),
        "--log.format".into(),
        "json".into(),
        "--log.level".into(),
        "info".into(),
        "--path.config".into(),
        config.to_string_lossy().into_owned(),
    ]);
    invocation.unset_env = [
        "LS_JAVA_HOME",
        "LS_JVM_OPTS",
        "LS_JAVA_OPTS",
        "LS_SETTINGS_DIR",
        "LS_GEM_HOME",
        "LS_GEM_PATH",
        "JAVA_HOME",
        "JAVA_OPTS",
        "JAVA_TOOL_OPTIONS",
        "JDK_JAVA_OPTIONS",
        "JAVACMD",
        "RUBYOPT",
        "RUBYLIB",
    ]
    .map(String::from)
    .into();
    invocation
}

pub(in crate::delivery) fn version_args() -> &'static [&'static str] {
    &["--version"]
}

pub(in crate::delivery) fn identifies(version: &str) -> bool {
    // Its launcher prints the bundled Java location before the version line.
    version.lines().any(|line| {
        line.strip_prefix("logstash ")
            .and_then(|value| value.chars().next())
            .is_some_and(|character| character.is_ascii_digit())
    })
}

pub(in crate::delivery) fn error(line: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(line).ok()?;
    let level = value.get("level")?.as_str()?;
    (level.eq_ignore_ascii_case("error") || level.eq_ignore_ascii_case("fatal")).then(|| {
        value
            .get("message")
            .or_else(|| value.pointer("/logEvent/message"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Logstash logged an error")
            .chars()
            .take(1024)
            .collect()
    })
}

//! Splunk runs with private configuration and state, while sharing its installed engine.

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::runtime::Invocation;

pub(in crate::delivery) fn configure(
    mut launch: Invocation,
    output: &Path,
    _config: &Path,
    _timeout: u64,
) -> Invocation {
    let home = output.join("splunk-home");
    let home = home.canonicalize().unwrap_or(home);
    let engine = home.join("bin/splunkd").to_string_lossy().into_owned();
    if launch.program == "taskset" {
        // The common launch prefix is taskset --cpu-list <cpus> <CLI>.
        launch.args[2] = engine;
    } else {
        launch.program = engine;
    }
    #[cfg(target_os = "linux")]
    {
        // Match the vendor CLI's loader paths when launching splunkd directly.
        let mut libraries = vec![home.join("opt/jemalloc-4k-nostats/lib"), home.join("lib")];
        if let Some(path) = env::var_os("LD_LIBRARY_PATH") {
            libraries.extend(env::split_paths(&path));
        }
        launch = launch.env(
            "LD_LIBRARY_PATH",
            env::join_paths(libraries)
                .expect("generated Splunk library paths are valid")
                .to_string_lossy(),
        );
    }
    let mut paths = vec![home.join("bin")];
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    let native_path = env::join_paths(paths).unwrap_or_default();
    launch
        .args(["--nodaemon"])
        .env("SPLUNK_HOME", home.to_string_lossy())
        .env("SPLUNK_DB", home.join("var/lib/splunk").to_string_lossy())
        .env("SPLUNK_ETC", home.join("etc").to_string_lossy())
        .env("SPLUNK_BINDIP", "127.0.0.1")
        .env("HOSTNAME", "vth")
        .env("PATH", native_path.to_string_lossy())
}

pub(in crate::delivery) fn version_args() -> &'static [&'static str] {
    // A new installation otherwise asks for license acceptance on a closed stdin.
    &["version", "--no-prompt"]
}

pub(in crate::delivery) fn identifies(version: &str, universal: bool) -> bool {
    let prefix = if universal {
        "Splunk Universal Forwarder "
    } else {
        "Splunk "
    };
    version.lines().any(|line| {
        line.strip_prefix(prefix).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        })
    })
}

pub(in crate::delivery) fn error(line: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(line);
    let text = text.trim();
    // Before log4cplus opens splunkd.log, failures are written without a timestamp.
    if ["ERROR -", "FATAL -", "log4cplus:ERROR", "log4cplus:FATAL"]
        .iter()
        .any(|prefix| text.starts_with(prefix))
    {
        return Some(text.chars().take(1024).collect());
    }
    let mut fields = text.split_whitespace();
    let date = fields.next()?;
    let time = fields.next()?;
    let zone = fields.next()?;
    let level = fields.next()?;
    if date.len() != 10 || !time.contains(':') || zone.len() != 5 {
        return None;
    }
    matches!(level, "ERROR" | "FATAL").then(|| text.chars().take(1024).collect())
}

pub(in crate::delivery) fn prepare(
    binary: &Path,
    output: &Path,
    rendered_config: &str,
) -> Result<()> {
    let files: Value =
        serde_json::from_str(rendered_config).context("parse Splunk config bundle")?;
    let files = files
        .as_object()
        .context("Splunk config bundle must be an object")?;
    for (name, content) in files {
        ensure!(
            matches!(
                name.as_str(),
                "inputs.conf"
                    | "outputs.conf"
                    | "props.conf"
                    | "transforms.conf"
                    | "server.conf"
                    | "web.conf"
                    | "limits.conf"
                    | "telemetry.conf"
            ),
            "unsupported Splunk config file {name}"
        );
        ensure!(
            content.is_string(),
            "Splunk config file {name} must contain text"
        );
    }
    let cli = binary.canonicalize().context("resolve Splunk CLI")?;
    ensure!(
        cli.file_name().is_some_and(|name| name == "splunk"),
        "supply bin/splunk from an initialized isolated Splunk distribution"
    );
    let source = cli
        .parent()
        .and_then(Path::parent)
        .context("Splunk CLI must be inside its distribution's bin directory")?;
    ensure!(
        !source.join("ftr").exists(),
        "initialize this isolated Splunk distribution and accept its required terms explicitly before running it"
    );
    ensure!(
        source.join("etc/auth/server.pem").is_file(),
        "complete the isolated distribution's vendor startup prerequisites first (missing server.pem)"
    );
    ensure!(engine_binary(binary)?.is_file(), "Splunk engine is missing");
    #[cfg(target_os = "macos")]
    let temporary = Path::new("/private/tmp");
    #[cfg(not(target_os = "macos"))]
    let temporary = Path::new("/tmp");
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let home = temporary.join(format!("vth-splunk-{}-{nonce}", std::process::id()));
    fs::create_dir(&home).context("create private Splunk home")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&home, fs::Permissions::from_mode(0o700))?;
    }
    // Retain the home even after a failure so its native logs and config remain reviewable.
    symlink(&home, &output.join("splunk-home"))?;
    fs::write(
        output.join("splunk-runtime.json"),
        serde_json::to_vec_pretty(&json!({
            "source_home":source, "isolated_home":home,
            "engine":engine_binary(binary)?,
            "shared":"vendor distribution except etc and var",
            "configuration":"private etc copy", "state":"fresh private var",
            "native_log":home.join("var/log/splunk/splunkd.log"),
            "terms_acceptance":"requires preinitialized source; harness never accepts terms"
        }))?,
    )?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "etc" {
            copy_tree(&entry.path(), &home.join("etc"))?;
        } else if name != "var" {
            symlink(&entry.path(), &home.join(name))?;
        }
    }
    // The CLI and native subprocesses prefer these persisted paths over their environment.
    let launch_config = home.join("etc/splunk-launch.conf");
    let original = match fs::read_to_string(&launch_config) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).context("read private Splunk launch configuration"),
    };
    let paths = [
        ("SPLUNK_HOME", home.clone()),
        ("SPLUNK_DB", home.join("var/lib/splunk")),
    ];
    let mut found = [false; 2];
    let mut isolated = String::with_capacity(original.len());
    for line in original.split_inclusive('\n') {
        let setting = line
            .split_once('=')
            .and_then(|(key, _)| paths.iter().position(|(name, _)| key.trim() == *name));
        if let Some(index) = setting {
            let ending = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            isolated.push_str(&format!(
                "{}={}{}",
                paths[index].0,
                paths[index].1.display(),
                ending
            ));
            found[index] = true;
        } else {
            isolated.push_str(line);
        }
    }
    for (index, (name, path)) in paths.iter().enumerate() {
        if !found[index] {
            if !isolated.is_empty() && !isolated.ends_with('\n') {
                isolated.push('\n');
            }
            isolated.push_str(&format!("{name}={}\n", path.display()));
        }
    }
    fs::write(launch_config, isolated).context("isolate Splunk launch paths")?;
    // Direct splunkd needs the empty runtime parents normally made by CLI bootstrap.
    // Never copy the initialized source's queues, checkpoints, logs or PID files.
    for directory in [
        "var/lib/splunk",
        "var/log/splunk",
        "var/log/introspection",
        "var/log/watchdog",
        "var/log/client_events",
        "var/run/splunk",
        "var/spool/splunk",
        "var/spool/dirmoncache",
    ] {
        fs::create_dir_all(home.join(directory))?;
    }
    symlink(
        &home.join("var/log/splunk/splunkd.log"),
        &output.join("splunkd.log"),
    )?;
    // Every isolated runtime receives a fresh instance identity.
    let instance = home.join("etc/instance.cfg");
    if instance.exists() {
        fs::remove_file(instance)?;
    }
    let local = home.join("etc/system/local");
    fs::create_dir_all(&local)?;
    for (name, content) in files {
        fs::write(local.join(name), content.as_str().unwrap())?;
    }
    if files
        .get("transforms.conf")
        .and_then(Value::as_str)
        .is_some_and(|text| text.lines().any(|line| line.trim() == "[vth_drop]"))
    {
        fs::write(
            output.join("splunk-counter.json"),
            serde_json::to_vec_pretty(&json!({
                "rule":"vth_drop", "source_type":"vth_raw", "unit":"records",
                "native_log":home.join("var/log/splunk/metrics.log"),
                "basis":"per-rule native nullQueue metrics; zero-byte control events skip the counted rule"
            }))?,
        )?;
    }
    // This shipped app disables search/indexer-specific pipelines in a heavy forwarder.
    let forwarder_app = home.join("etc/apps/SplunkForwarder");
    if forwarder_app.is_dir() {
        fs::create_dir_all(forwarder_app.join("local"))?;
        fs::write(
            forwarder_app.join("local/app.conf"),
            "[install]\nstate = enabled\n",
        )?;
        // These shipped UI/monitoring apps execute management API scripts at startup.
        // They have no role in this forwarding-only instance with Web and KV Store off.
        for name in [
            "splunk-dashboard-studio",
            "splunk_monitoring_console",
            "splunk_instrumentation",
            "splunk_secure_gateway",
        ] {
            let app = home.join("etc/apps").join(name);
            if app.is_dir() {
                fs::create_dir_all(app.join("local"))?;
                fs::write(app.join("local/app.conf"), "[install]\nstate = disabled\n")?;
            }
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_symlink() {
            symlink(&fs::read_link(entry.path())?, &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn symlink(source: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, destination).context("link isolated Splunk distribution")
}

#[cfg(not(unix))]
fn symlink(_source: &Path, _destination: &Path) -> Result<()> {
    anyhow::bail!("isolated Splunk execution requires Unix")
}

pub(in crate::delivery) fn extra_logs(output: &Path) -> Vec<PathBuf> {
    let mut paths = vec![output.join("splunk-home/var/log/splunk/splunkd.log")];
    if output.join("splunk-counter.json").is_file() {
        paths.push(output.join("splunk-home/var/log/splunk/metrics.log"));
    }
    paths
}

pub(in crate::delivery) fn capture_logs(output: &Path) -> Result<()> {
    for source in extra_logs(output) {
        ensure!(
            fs::metadata(&source)?.len() <= 1024 * 1024,
            "native log exceeds 1 MiB"
        );
        let name = if source.file_name().is_some_and(|name| name == "metrics.log") {
            "splunk-metrics.log"
        } else {
            "splunkd.log"
        };
        let destination = output.join(name);
        if destination.is_symlink() {
            fs::remove_file(&destination)?;
        }
        fs::copy(&source, destination)
            .context("preserve native log outside temporary Splunk home")?;
    }
    Ok(())
}

pub(in crate::delivery) fn count(output: &Path) -> Result<Option<u64>> {
    if !output.join("splunk-counter.json").is_file() {
        return Ok(None);
    }
    let path = output.join("splunk-home/var/log/splunk/metrics.log");
    if !path.exists() {
        return Ok(None);
    }
    ensure!(
        fs::metadata(&path)?.len() <= 1024 * 1024,
        "native metrics log exceeds 1 MiB"
    );
    let bytes = fs::read(&path)?;
    let mut reports = Vec::new();
    let mut count = 0_u64;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if !line.ends_with(b"\n") {
            continue;
        }
        let text = std::str::from_utf8(line).context("native metrics is not UTF-8")?;
        let Some((_, data)) = text.split_once("group=transforms, ") else {
            continue;
        };
        let fields = data
            .trim()
            .split(", ")
            .filter_map(|field| field.split_once('='))
            .collect::<std::collections::BTreeMap<_, _>>();
        if fields.get("rule") != Some(&"vth_drop") || fields.get("source_type") != Some(&"vth_raw")
        {
            continue;
        }
        ensure!(
            matches!(fields.get("name"), Some(&"typing" | &"structuredparsing")),
            "unexpected native discard pipeline"
        );
        let number = |name| -> Result<u64> {
            fields
                .get(name)
                .with_context(|| format!("native discard metric {name} missing"))?
                .parse()
                .with_context(|| format!("invalid native discard metric {name}"))
        };
        let input = number("in")?;
        let dropped = number("out.drop")?;
        let hits = number("hit")?;
        ensure!(
            input == dropped,
            "counted native rule did not discard every input byte"
        );
        ensure!(
            hits == 0 || input > 0,
            "zero-byte control event reached native application counter"
        );
        for name in fields
            .keys()
            .filter(|name| name.starts_with("out.") && **name != "out.drop")
        {
            ensure!(
                number(name)? == 0,
                "counted native rule routed data outside nullQueue"
            );
        }
        count = count
            .checked_add(hits)
            .context("native discard counter overflow")?;
        reports.push(text.trim().to_owned());
    }
    if reports.is_empty() {
        return Ok(None);
    }
    let receipt = json!({
        "count":count, "unit":"records", "rule":"vth_drop", "source_type":"vth_raw",
        "native_log":path, "native_log_sha256":format!("{:x}",Sha256::digest(&bytes)), "reports":reports,
        "basis":"sum of complete per-rule native nullQueue intervals; excludes zero-byte control events before this rule",
        "identities_verified":false, "payloads_verified":false
    });
    let destination = output.join("native-metrics.json");
    let serialized = serde_json::to_vec_pretty(&receipt)?;
    if fs::read(&destination).ok().as_deref() != Some(serialized.as_slice()) {
        fs::write(output.join("native-metrics.json.tmp"), serialized)?;
        fs::rename(output.join("native-metrics.json.tmp"), destination)?;
    }
    Ok(Some(count))
}

pub(in crate::delivery) fn engine_binary(binary: &Path) -> Result<PathBuf> {
    Ok(binary
        .canonicalize()
        .context("resolve Splunk CLI")?
        .parent()
        .context("Splunk CLI has no parent")?
        .join("splunkd"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn detects_failure_before_native_log_opens() {
        let line = b"ERROR - Failed opening \"/tmp/vth/var/log/splunk/splunkd.log\": No such file or directory\n";
        assert!(super::error(line).is_some());
    }

    #[test]
    fn counts_native_discard_intervals_and_rejects_control_events() {
        let output = tempfile::tempdir().unwrap();
        assert_eq!(super::count(output.path()).unwrap(), None);
        let native = output.path().join("splunk-home/var/log/splunk");
        std::fs::create_dir_all(&native).unwrap();
        std::fs::write(output.path().join("splunk-counter.json"), "{}").unwrap();
        let mut metrics = String::new();
        // Actual native canary, warmup and measured interval shapes.
        for (bytes, hits) in [(186, 1), (186000, 1000), (1860000, 10000)] {
            metrics += &format!("11-13-2026 15:13:14.316 -0500 INFO  Metrics - group=transforms, name=structuredparsing, rule=vth_drop, source_type=vth_raw, in={bytes}, hit={hits}, timespent=0.000, out.drop={bytes}\n");
        }
        std::fs::write(native.join("metrics.log"), &metrics).unwrap();
        assert_eq!(super::count(output.path()).unwrap(), Some(11001));
        metrics += "11-13-2026 15:13:54.316 -0500 INFO  Metrics - group=transforms, name=structuredparsing, rule=vth_drop, source_type=vth_raw, in=0, hit=1, timespent=0.000, out.drop=0\n";
        std::fs::write(native.join("metrics.log"), metrics).unwrap();
        assert!(super::count(output.path()).is_err());
    }
}

//! Reproducible setup around the existing native-product matrix.

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{ensure, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    cli::{CampaignArgs, CheckCase},
    delivery::subject::Subject,
    runtime::{sha256, Runtime, INTERRUPTED},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Lock {
    schema_version: u32,
    platform: Platform,
    toolchain: String,
    subjects: BTreeMap<String, Distribution>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Platform {
    os: String,
    arch: String,
    distribution: String,
    version: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Distribution {
    version: String,
    url: String,
    sha256: String,
    binary: PathBuf,
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn relative(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "expected a repository-relative path without parent components: {}",
        path.display()
    );
    Ok(())
}

impl Lock {
    fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "unsupported runtime lock schema");
        ensure!(
            self.platform.os == "linux" && self.platform.arch == "x86_64",
            "runtime locks currently support Linux x86_64"
        );
        ensure!(
            self.platform.distribution == "ubuntu" && self.platform.version == "22.04",
            "runtime locks currently support Ubuntu 22.04"
        );
        let version = self.toolchain.split('.').collect::<Vec<_>>();
        ensure!(
            version.len() == 3
                && version
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())),
            "pin a complete Rust toolchain version"
        );
        ensure!(!self.subjects.is_empty(), "runtime lock has no subjects");
        for (name, distribution) in &self.subjects {
            let subject = Subject::from_str(name, false).map_err(anyhow::Error::msg)?;
            ensure!(
                name == subject.name(),
                "use canonical product names in lock"
            );
            ensure!(!distribution.version.is_empty(), "missing {name} version");
            let url = distribution
                .url
                .strip_prefix("https://")
                .context("runtime downloads require HTTPS")?;
            let authority = url.split('/').next().unwrap_or_default();
            ensure!(
                !authority.is_empty()
                    && !authority.contains('@')
                    && !url.contains(['?', '#'])
                    && distribution.url.ends_with(".deb"),
                "pin a public Debian package URL for {name}"
            );
            ensure!(
                hex(&distribution.sha256, 64),
                "pin a lowercase SHA-256 for {name}"
            );
            relative(&distribution.binary)?;
        }
        Ok(())
    }
}

/// No inherited product options, .env, curlrc, Git configuration, or Cargo flags.
/// Build commands additionally use the existing Rustup installation explicitly.
fn command(program: &str, cwd: &Path, home: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .current_dir(cwd)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home)
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("TZ", "UTC")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .stdin(Stdio::null());
    command
}

fn checked(command: &mut Command) -> Result<Output> {
    ensure!(!INTERRUPTED.load(Ordering::SeqCst), "campaign interrupted");
    let output = command.output().context("execute campaign setup command")?;
    ensure!(
        output.status.success(),
        "campaign command {:?} failed with {}: {}",
        command.get_program(),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output)
}

fn capture(command: &mut Command) -> Result<String> {
    String::from_utf8(checked(command)?.stdout).context("non-UTF-8 campaign command output")
}

fn save(output: &Path, receipt: &Value) -> Result<()> {
    let temporary = output.join("campaign.json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(receipt)?)?;
    fs::rename(temporary, output.join("campaign.json"))?;
    Ok(())
}

fn os_field(text: &str, field: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.strip_prefix(&format!("{field}="))
            .map(|value| value.trim_matches('"').to_owned())
    })
}

fn verify_platform(lock: &Lock) -> Result<String> {
    ensure!(
        std::env::consts::OS == lock.platform.os && std::env::consts::ARCH == lock.platform.arch,
        "campaign requires {} {}; use a matching Linux host",
        lock.platform.os,
        lock.platform.arch
    );
    let release = fs::read_to_string("/etc/os-release")?;
    ensure!(
        os_field(&release, "ID").as_deref() == Some(&lock.platform.distribution)
            && os_field(&release, "VERSION_ID").as_deref() == Some(&lock.platform.version),
        "campaign requires {} {}",
        lock.platform.distribution,
        lock.platform.version
    );
    Ok(release)
}

fn install(
    output: &Path,
    home: &Path,
    name: &str,
    distribution: &Distribution,
) -> Result<(PathBuf, Value)> {
    let archive = output.join("downloads").join(format!("{name}.deb"));
    let partial = archive.with_extension("deb.partial");
    checked(command("curl", output, home).args([
        "--disable",
        "--fail",
        "--location",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--retry",
        "2",
        "--connect-timeout",
        "30",
        "--max-time",
        "600",
        "--output",
        &partial.to_string_lossy(),
        &distribution.url,
    ]))?;
    let metadata = verify_distribution(output, home, &partial, name, distribution)?;
    fs::rename(&partial, &archive)?;
    let install = output.join("runtimes").join(name);
    fs::create_dir(&install)?;
    // Only complete, checksum-verified vendor packages reach the native extractor.
    // Extraction does not run Debian maintainer scripts or install system services.
    checked(command("dpkg-deb", output, home).args([
        "--extract",
        &archive.to_string_lossy(),
        &install.to_string_lossy(),
    ]))?;
    let binary = install
        .join(&distribution.binary)
        .canonicalize()
        .with_context(|| format!("{name} package has no configured executable"))?;
    ensure!(
        binary.starts_with(&install) && binary.is_file(),
        "{name} executable escapes its private distribution"
    );
    let libraries = capture(command("ldd", output, home).arg(&binary))?;
    ensure!(
        !libraries.contains("not found"),
        "{name} runtime libraries are missing; install the lock's host prerequisites: {libraries}"
    );
    let receipt = json!({
        "distribution":distribution,"archive":archive,"archive_sha256":sha256(&archive)?,
        "archive_bytes":fs::metadata(&archive)?.len(),"package_metadata":metadata,
        "binary":binary,"binary_sha256":sha256(&binary)?,"dynamic_libraries":libraries,
    });
    Ok((binary, receipt))
}

fn verify_distribution(
    output: &Path,
    home: &Path,
    archive: &Path,
    name: &str,
    distribution: &Distribution,
) -> Result<String> {
    ensure!(
        sha256(archive)? == distribution.sha256,
        "{name} distribution checksum mismatch; refusing to extract or execute"
    );
    let metadata = capture(command("dpkg-deb", output, home).args([
        "--field",
        &archive.to_string_lossy(),
        "Package",
        "Version",
        "Architecture",
        "Depends",
    ]))?;
    for (field, expected) in [
        ("Package", name),
        ("Version", distribution.version.as_str()),
        ("Architecture", "amd64"),
    ] {
        let actual = metadata
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{field}: ")));
        ensure!(
            actual == Some(expected),
            "{name} package {field} mismatch: expected {expected}, got {actual:?}"
        );
    }
    Ok(metadata)
}

fn clean_source(source: &Path, home: &Path) -> Result<()> {
    ensure!(
        capture(command("git", source, home).args([
            "status",
            "--porcelain",
            "--untracked-files=all"
        ]))?
        .trim()
        .is_empty(),
        "campaign source changed after checkout"
    );
    Ok(())
}

fn isolated_cargo(source: &Path) -> Result<()> {
    // Cargo searches parent directories even with a private CARGO_HOME.
    for ancestor in source.parent().into_iter().flat_map(Path::ancestors) {
        for name in ["config", "config.toml"] {
            let config = ancestor.join(".cargo").join(name);
            ensure!(
                !config.exists(),
                "unpinned ancestor Cargo configuration at {}; choose an output directory outside that tree",
                config.display()
            );
        }
    }
    Ok(())
}

fn execute(
    runtime: &Runtime,
    args: &CampaignArgs,
    output: &Path,
    receipt: &mut Value,
) -> Result<i32> {
    let setup_runner = std::env::current_exe()?.canonicalize()?;
    receipt["setup_runner"] = json!({"binary":setup_runner,"sha256":sha256(&setup_runner)?});
    let source = output.join("source");
    let home = output.join("home");
    fs::create_dir(&home)?;
    checked(command("git", output, &home).args([
        "clone",
        "--quiet",
        "--no-local",
        "--no-hardlinks",
        "--no-checkout",
        "--",
        &runtime.root.to_string_lossy(),
        &source.to_string_lossy(),
    ]))?;
    checked(command("git", &source, &home).args([
        "checkout",
        "--quiet",
        "--detach",
        &args.revision,
        "--",
    ]))?;
    let revision = capture(command("git", &source, &home).args(["rev-parse", "HEAD"]))?;
    ensure!(
        revision.trim() == args.revision,
        "harness revision mismatch"
    );
    clean_source(&source, &home)?;
    isolated_cargo(&source)?;
    let lock_path = source.join(&args.lock).canonicalize().with_context(|| {
        format!(
            "resolve runtime lock {} in harness commit {}",
            args.lock.display(),
            args.revision
        )
    })?;
    ensure!(
        lock_path.starts_with(&source),
        "runtime lock escapes source"
    );
    let lock: Lock = serde_json::from_slice(&fs::read(&lock_path)?)
        .context("parse runtime lock from selected commit")?;
    lock.validate()?;
    let subjects = if args.subjects.is_empty() {
        lock.subjects.keys().cloned().collect::<Vec<_>>()
    } else {
        args.subjects
            .iter()
            .map(|subject| subject.name().to_owned())
            .collect::<Vec<_>>()
    };
    for subject in &subjects {
        ensure!(
            lock.subjects.contains_key(subject),
            "{subject} has no pinned distribution in this runtime lock"
        );
    }
    receipt["harness"] = json!({
        "revision":revision.trim(),
        "tree":capture(command("git", &source, &home).args(["rev-parse", "HEAD^{tree}"]))?.trim(),
        "source":source,"cargo_lock_sha256":sha256(&source.join("Cargo.lock"))?,
    });
    receipt["runtime_lock"] =
        json!({"path":args.lock,"sha256":sha256(&lock_path)?,"contents":lock});
    receipt["subjects"] = json!(subjects);
    save(output, receipt)?;
    let os_release = verify_platform(&lock)?;
    receipt["host"] = json!({
        "os_release":os_release,
        "kernel":capture(command("uname", output, &home).args(["-srvmo"]))?,
        "packages":capture(command("dpkg-query", output, &home).args(["-W", "-f=${binary:Package}\t${Version}\t${Architecture}\n"]))?,
    });
    fs::create_dir(output.join("downloads"))?;
    fs::create_dir(output.join("runtimes"))?;
    let mut runtimes = BTreeMap::new();
    receipt["distributions"] = json!({});
    for subject in &subjects {
        eprintln!("Preparing {subject} {}", lock.subjects[subject].version);
        let (binary, distribution) = install(output, &home, subject, &lock.subjects[subject])?;
        runtimes.insert(subject, json!({"binary":binary}));
        receipt["distributions"][subject] = distribution;
        save(output, receipt)?;
    }
    let runtimes_path = output.join("runtimes.json");
    fs::write(
        &runtimes_path,
        serde_json::to_vec_pretty(&json!({"subjects":runtimes}))?,
    )?;
    receipt["runtime_manifest_sha256"] = json!(sha256(&runtimes_path)?);

    let cargo_home = output.join("cargo-home");
    let target = output.join("build");
    let rustup_home = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")))
        .context("set HOME or RUSTUP_HOME for the pinned toolchain")?;
    let path = std::env::var_os("PATH").context("PATH is not set")?;
    let build_command = |program: &str| {
        let mut command = command(program, &source, &home);
        command
            .env("PATH", &path)
            .env("RUSTUP_HOME", &rustup_home)
            .env("CARGO_HOME", &cargo_home);
        command
    };
    let toolchain = format!("+{}", lock.toolchain);
    let rustc = capture(build_command("rustc").args([&toolchain, "-vV"]))?;
    let cargo = capture(build_command("cargo").args([&toolchain, "--version"]))?;
    receipt["build"] = json!({"toolchain":lock.toolchain,"rustc":rustc,"cargo":cargo,
        "profile":"release","locked":true,"cargo_home":cargo_home,"target":target,
        "environment":"empty environment plus PATH, HOME, RUSTUP_HOME, private CARGO_HOME, C locale and UTC"});
    save(output, receipt)?;
    eprintln!(
        "Building harness {} with Rust {}",
        args.revision, lock.toolchain
    );
    let built = build_command("cargo")
        .args([
            &toolchain,
            "build",
            "--locked",
            "--release",
            "--bin",
            "vth",
            "--target-dir",
        ])
        .arg(&target)
        .stdout(fs::File::create(output.join("build.stdout.log"))?)
        .stderr(fs::File::create(output.join("build.stderr.log"))?)
        .status()?;
    ensure!(
        built.success(),
        "campaign build failed; see build.stderr.log"
    );
    clean_source(&source, &home)?;
    let executable = target.join("release/vth").canonicalize()?;
    receipt["harness"]["binary"] = json!(executable);
    receipt["harness"]["binary_sha256"] = json!(sha256(&executable)?);
    let results = output.join("results");
    let mut matrix = command(
        executable.to_str().context("non-UTF-8 executable path")?,
        &source,
        &home,
    );
    matrix
        .args(["--no-dotenv", "--root"])
        .arg(&source)
        .args(["matrix", "--runtimes"])
        .arg(&runtimes_path)
        .arg("--output")
        .arg(&results)
        .args([
            "--subjects",
            &subjects.join(","),
            "--cases",
            &args
                .cases
                .iter()
                .map(|case| case.name())
                .collect::<Vec<_>>()
                .join(","),
            "--events",
            &args.events.to_string(),
            "--seed",
            &args.seed.to_string(),
            "--runs",
            &args.runs.to_string(),
        ]);
    if let Some(timeout) = args.timeout_secs {
        matrix.args(["--timeout-secs", &timeout.to_string()]);
    }
    receipt["state"] = json!("running");
    receipt["matrix"] =
        json!({"path":results.join("matrix.json"),"args":matrix.get_args().collect::<Vec<_>>()});
    save(output, receipt)?;
    ensure!(!INTERRUPTED.load(Ordering::SeqCst), "campaign interrupted");
    let status = matrix.status()?;
    receipt["matrix"]["exit_code"] = json!(status.code());
    clean_source(&source, &home)?;
    let report: Value = serde_json::from_slice(&fs::read(results.join("matrix.json"))?)?;
    ensure!(
        report["runner"]["sha256"] == receipt["harness"]["binary_sha256"],
        "matrix runner does not match the pinned build"
    );
    receipt["matrix"]["sha256"] = json!(sha256(&results.join("matrix.json"))?);
    receipt["matrix"]["summary"] = report["summary"].clone();
    receipt["state"] = json!(if status.success() {
        "finished"
    } else {
        "matrix_failed"
    });
    Ok(status.code().unwrap_or(1))
}

pub fn run(runtime: &Runtime, args: CampaignArgs) -> Result<i32> {
    ensure!(
        hex(&args.revision, 40),
        "--revision must be a full lowercase 40-character commit SHA"
    );
    relative(&args.lock)?;
    ensure!(
        !args
            .cases
            .iter()
            .any(|case| matches!(case, CheckCase::DockerPartial | CheckCase::FileToBlackhole)),
        "campaign Docker/perf prerequisites are not yet pinned; use matrix for these cases"
    );
    let output = runtime.path(&args.output)?;
    let mut receipt = json!({"schema_version":1,"state":"preparing","revision":args.revision,
        "runtime_lock_path":args.lock,"output":output,
        "selection":{"subjects":args.subjects.iter().map(|s|s.name()).collect::<Vec<_>>(),
            "cases":args.cases.iter().map(|c|c.name()).collect::<Vec<_>>(),
            "events":args.events,"seed":args.seed,"runs":args.runs,"timeout_secs":args.timeout_secs},
        "performance_comparison":false});
    if runtime.dry_run {
        println!("{}", serde_json::to_string(&receipt)?);
        return Ok(0);
    }
    fs::create_dir_all(output.parent().context("campaign output has no parent")?)?;
    fs::create_dir(&output).context("campaign output must be a new directory")?;
    let output = output.canonicalize()?;
    receipt["created_unix_seconds"] =
        json!(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs());
    save(&output, &receipt)?;
    println!("{}", output.join("campaign.json").display());
    let result = execute(runtime, &args, &output, &mut receipt);
    if let Err(error) = &result {
        receipt["state"] = json!("failed");
        receipt["error"] = json!(format!("{error:#}"));
    }
    receipt["completed_unix_seconds"] =
        json!(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs());
    save(&output, &receipt)?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_cargo_configuration_outside_pinned_source() {
        let directory = tempfile::tempdir().unwrap();
        let ancestor = directory.path().join(".cargo");
        fs::create_dir(&ancestor).unwrap();
        let config = ancestor.join("config.toml");
        fs::write(&config, "[build]\nrustflags = ['--cfg', 'unexpected']\n").unwrap();
        let error = isolated_cargo(&directory.path().join("campaign/source")).unwrap_err();
        assert!(error.to_string().contains(config.to_str().unwrap()));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn verifies_complete_package_before_extraction() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let control = root.join("package/DEBIAN");
        fs::create_dir_all(&control).unwrap();
        fs::write(control.join("control"), "Package: vector\nVersion: 1.2.3-1\nArchitecture: amd64\nMaintainer: VTH <nobody@example.invalid>\nDescription: integrity fixture\n").unwrap();
        let archive = root.join("fixture.deb");
        checked(
            command("dpkg-deb", root, root)
                .arg("--build")
                .arg(root.join("package"))
                .arg(&archive),
        )
        .unwrap();
        let mut distribution = Distribution {
            version: "1.2.3-1".into(),
            url: "https://example.invalid/fixture.deb".into(),
            sha256: sha256(&archive).unwrap(),
            binary: "usr/bin/vector".into(),
        };
        verify_distribution(root, root, &archive, "vector", &distribution).unwrap();
        distribution.version = "9.9.9".into();
        assert!(
            verify_distribution(root, root, &archive, "vector", &distribution)
                .unwrap_err()
                .to_string()
                .contains("Version mismatch")
        );
        fs::write(&archive, "tampered package").unwrap();
        assert!(
            verify_distribution(root, root, &archive, "vector", &distribution)
                .unwrap_err()
                .to_string()
                .contains("checksum mismatch")
        );
        assert!(!root.join("runtimes").exists());
    }
}

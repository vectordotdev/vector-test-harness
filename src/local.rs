use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, ensure, Context, Result};
use flate2::read::MultiGzDecoder;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    cli::{
        BenchArgs, BuildArgs, FlamegraphArgs, GenerateArgs, ProfileArgs, ProfileTool, WorkloadArgs,
    },
    runtime::{check_file, identifier, Invocation, Runtime, INTERRUPTED},
};

struct BuildLock(PathBuf);
impl Drop for BuildLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn build(runtime: &Runtime, args: BuildArgs) -> Result<()> {
    identifier(&args.binary.replace('-', "_"), "binary name")?;
    ensure!(
        !args.revision.is_empty() && !args.revision.starts_with('-'),
        "invalid Git revision"
    );
    let key = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}\n{}\n{}\n{}\n{:?}\n{}",
                args.repository,
                args.revision,
                args.binary,
                args.no_default_features,
                args.features,
                args.rustflags
            )
            .as_bytes()
        )
    );
    let parent = runtime.root.join(".cache/builds");
    let dir = parent.join(&key);
    runtime.mkdir(&parent)?;
    let _lock = if runtime.dry_run {
        None
    } else {
        let path = parent.join(format!("{key}.lock"));
        fs::OpenOptions::new().write(true).create_new(true).open(&path).with_context(|| format!("build lock exists at {}; check for a running build before removing a stale lock", path.display()))?;
        Some(BuildLock(path))
    };
    if !dir.join(".git").exists() {
        runtime.run(Invocation::new("git", &runtime.root).args([
            "clone",
            "--no-checkout",
            "--",
            &args.repository,
            &dir.to_string_lossy(),
        ]))?;
    }
    runtime.run(Invocation::new("git", &dir).args([
        "fetch",
        "--no-tags",
        "--",
        "origin",
        &args.revision,
    ]))?;
    let revision = runtime.capture(Invocation::new("git", &dir).args([
        "rev-parse",
        "--verify",
        "FETCH_HEAD^{commit}",
    ]))?;
    let revision = if runtime.dry_run {
        "<resolved-commit>"
    } else {
        revision.trim()
    };
    runtime.run(Invocation::new("git", &dir).args(["checkout", "--detach", revision, "--"]))?;
    let cargo = Invocation::new("cargo", &dir).env("RUSTFLAGS", &args.rustflags);
    let metadata =
        runtime.capture(
            cargo
                .clone()
                .args(["metadata", "--no-deps", "--format-version", "1"]),
        )?;
    if !runtime.dry_run {
        let metadata: Value = serde_json::from_str(&metadata)?;
        let target = PathBuf::from(
            metadata["target_directory"]
                .as_str()
                .context("Cargo metadata has no target_directory")?,
        );
        ensure!(
            target == dir.join("target"),
            "isolated build must use its own target directory; Cargo reports {}",
            target.display()
        );
        ensure!(
            !fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()),
            "target is a symlink; refusing to change or follow it"
        );
    }
    let mut command = cargo.args([
        "build",
        "--locked",
        "--release",
        "--bin",
        &args.binary,
        "--message-format=json-render-diagnostics",
    ]);
    if args.no_default_features {
        command.args.push("--no-default-features".into());
    }
    if let Some(features) = args.features {
        command = command.args(["--features", &features]);
    }
    if runtime.dry_run {
        return runtime.plan(&command);
    }
    let output = runtime.output(&command)?;
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    let mut executable = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Ok(message) = serde_json::from_str::<Value>(line) {
            if message["reason"] == "compiler-message" {
                if let Some(rendered) = message["message"]["rendered"].as_str() {
                    eprint!("{rendered}");
                }
            }
            if message["reason"] == "compiler-artifact" && message["target"]["name"] == args.binary
            {
                executable = message["executable"]
                    .as_str()
                    .map(str::to_owned)
                    .or(executable);
            }
        }
    }
    ensure!(
        output.status.success(),
        "Cargo build failed with {}",
        output.status
    );
    let executable = executable.context("Cargo did not report the requested executable")?;
    let rustc = runtime.capture(Invocation::new("rustc", &dir).args(["-vV"]))?;
    runtime.write(&dir.join("vth-build.json"), serde_json::to_string_pretty(&json!({"schema_version":1,"repository":args.repository,"revision":revision,"invocation":command,"rustc":rustc,"executable":executable}))?.as_bytes())?;
    println!("{executable}");
    Ok(())
}

struct Workload {
    dir: PathBuf,
    configs: Vec<PathBuf>,
    input: Option<PathBuf>,
    binary: PathBuf,
    output: PathBuf,
}

fn workload(runtime: &Runtime, args: &WorkloadArgs) -> Result<Workload> {
    identifier(&args.case, "standalone case")?;
    let dir = runtime.root.join("cases/performance").join(&args.case);
    ensure!(
        dir.is_dir(),
        "standalone case does not exist: {}",
        args.case
    );
    let binary = runtime.path(&args.vector_bin)?;
    check_file(&binary)?;
    let configs = if let Some(config) = &args.config {
        vec![runtime.path(config)?]
    } else if dir.join("configs").is_dir() {
        let mut configs = fs::read_dir(dir.join("configs"))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<io::Result<Vec<_>>>()?;
        configs
            .retain(|p| p.is_file() && p.extension().is_some_and(|e| e == "toml" || e == "yaml"));
        configs.sort();
        configs
    } else {
        vec![dir.join("vector.yaml")]
    };
    ensure!(!configs.is_empty(), "case has no Vector configs");
    for config in &configs {
        check_file(config)?;
    }
    let input = if let Some(input) = &args.input {
        Some(runtime.path(input)?)
    } else {
        match args.case.as_str() {
            "stdin_to_blackhole" => Some(
                runtime
                    .root
                    .join("cases/performance/file_to_blackhole/baked.log.gz"),
            ),
            "klog_remap_to_blackhole" => Some(dir.join("baked.log.gz")),
            "file_to_blackhole" => None,
            _ => bail!("supply --input for this standalone case"),
        }
    };
    if let Some(input) = &input {
        check_file(input)?;
    }
    let output = args
        .output
        .as_ref()
        .map(|p| runtime.path(p))
        .transpose()?
        .unwrap_or_else(|| {
            runtime.root.join(".cache/runs").join(format!(
                "{}-{}",
                args.case,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ))
        });
    Ok(Workload {
        dir,
        configs,
        input,
        binary,
        output,
    })
}

struct RunningChild(Child);
impl Drop for RunningChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn terminate(child: &mut Child) -> Result<()> {
    #[cfg(unix)]
    {
        let group = format!("-{}", child.id());
        let _ = Command::new("kill")
            .args(["-TERM", "--", &group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = Command::new("kill")
            .args(["-KILL", "--", &group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

fn execute_workload(runtime: &Runtime, spec: Invocation, input: Option<&Path>) -> Result<f64> {
    if runtime.dry_run {
        if let Some(input) = input {
            println!("{}", json!({"action":"feed-stdin","path":input}));
        }
        runtime.plan(&spec)?;
        return Ok(0.0);
    }
    let mut command = runtime.command(&spec);
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let started = Instant::now();
    let mut child = RunningChild(
        command
            .spawn()
            .with_context(|| format!("starting {}", spec.program))?,
    );
    let feeder = if let Some(input) = input {
        let file = fs::File::open(input)?;
        let gzip = input.extension().is_some_and(|e| e == "gz");
        let mut stdin = child.0.stdin.take().context("no child stdin")?;
        Some(thread::spawn(move || -> io::Result<u64> {
            if gzip {
                io::copy(&mut MultiGzDecoder::new(file), &mut stdin)
            } else {
                io::copy(&mut io::BufReader::new(file), &mut stdin)
            }
        }))
    } else {
        None
    };
    let status = loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            terminate(&mut child.0)?;
            bail!("workload interrupted");
        }
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(5));
    };
    let seconds = started.elapsed().as_secs_f64();
    if let Some(feeder) = feeder {
        let fed = feeder
            .join()
            .map_err(|_| anyhow::anyhow!("input feeder panicked"))?;
        ensure!(status.success(), "{} failed with {status}", spec.program);
        fed.context("feeding workload input")?;
    }
    ensure!(status.success(), "{} failed with {status}", spec.program);
    Ok(seconds)
}

fn vector(workload: &Workload, config: &Path, binary: &Path) -> Invocation {
    Invocation::new(&binary.to_string_lossy(), &workload.dir).args([
        "-qq",
        "--config",
        &config.to_string_lossy(),
    ])
}

fn sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65_536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn bench(runtime: &Runtime, args: BenchArgs) -> Result<()> {
    let workload = workload(runtime, &args.workload)?;
    ensure!(
        workload.input.is_some(),
        "timing requires a finite stdin workload; use profile and generate for file_to_blackhole"
    );
    let baseline = args.baseline_bin.map(|p| runtime.path(p)).transpose()?;
    if let Some(path) = &baseline {
        check_file(path)?;
    }
    runtime.mkdir(&workload.output)?;
    let mut records = vec![];
    for config in &workload.configs {
        let binaries: Vec<(&str, &Path)> = baseline
            .as_deref()
            .map(|b| vec![("baseline", b), ("candidate", workload.binary.as_path())])
            .unwrap_or_else(|| vec![("candidate", workload.binary.as_path())]);
        for (_, binary) in &binaries {
            for _ in 0..args.warmup {
                execute_workload(
                    runtime,
                    vector(&workload, config, binary),
                    workload.input.as_deref(),
                )?;
            }
        }
        let mut samples = BTreeSamples::default();
        for repetition in 0..args.runs {
            let mut order = binaries.clone();
            if repetition % 2 == 1 {
                order.reverse();
            }
            for (label, binary) in order {
                samples.0.entry(label).or_default().push(execute_workload(
                    runtime,
                    vector(&workload, config, binary),
                    workload.input.as_deref(),
                )?);
            }
        }
        if runtime.dry_run {
            continue;
        }
        for (label, binary) in &binaries {
            let values = &samples.0[label];
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let stddev = if values.len() > 1 {
                (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64)
                    .sqrt()
            } else {
                0.0
            };
            records.push(json!({"label":label,"binary":binary,"binary_sha256":sha256(binary)?,"config":config,"config_sha256":sha256(config)?,"seconds":values,"mean_seconds":mean,"stddev_seconds":stddev}));
        }
    }
    if runtime.dry_run {
        return runtime.write(&workload.output.join("benchmark.json"), &[]);
    }
    // Packaged/container copies need not contain .git. Identify the executable too,
    // so uncommitted harness changes do not masquerade as the recorded revision.
    let revision = runtime
        .output(&Invocation::new("git", &runtime.root).args(["rev-parse", "HEAD"]))
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_owned());
    let dirty = revision.as_ref().and_then(|_| {
        runtime
            .output(&Invocation::new("git", &runtime.root).args(["status", "--porcelain"]))
            .ok()
            .filter(|output| output.status.success())
            .map(|output| !output.stdout.is_empty())
    });
    let document = json!({"schema_version":1,"method":"wall-clock-with-streamed-input-v1","case":args.workload.case,"harness_revision":revision,"harness_dirty":dirty,"harness_binary_sha256":sha256(&std::env::current_exe()?)?,"harness_version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"warmup":args.warmup,"input":workload.input,"input_sha256":workload.input.as_deref().map(sha256).transpose()?,"results":records});
    runtime.write(
        &workload.output.join("benchmark.json"),
        serde_json::to_string_pretty(&document)?.as_bytes(),
    )?;
    println!("{}", workload.output.join("benchmark.json").display());
    Ok(())
}

#[derive(Default)]
struct BTreeSamples<'a>(std::collections::BTreeMap<&'a str, Vec<f64>>);

pub fn profile(runtime: &Runtime, args: ProfileArgs) -> Result<()> {
    let workload = workload(runtime, &args.workload)?;
    runtime.mkdir(&workload.output)?;
    for config in &workload.configs {
        let name = config
            .file_stem()
            .context("config has no filename")?
            .to_string_lossy();
        let base = vector(&workload, config, &workload.binary);
        let command = match args.tool {
            ProfileTool::Perf => Invocation::new("perf", &workload.dir).args([
                "record",
                "--call-graph",
                "dwarf",
                "--output",
                &workload
                    .output
                    .join(format!("{name}.perf.data"))
                    .to_string_lossy(),
                "--",
            ]),
            ProfileTool::Stat => Invocation::new("perf", &workload.dir).args([
                "stat",
                "--output",
                &workload
                    .output
                    .join(format!("{name}.perf-stat.txt"))
                    .to_string_lossy(),
                "--",
            ]),
            ProfileTool::Massif => Invocation::new("valgrind", &workload.dir).args([
                "--tool=massif".into(),
                format!(
                    "--massif-out-file={}",
                    workload.output.join(format!("{name}.massif.out")).display()
                ),
            ]),
            ProfileTool::Dhat => Invocation::new("valgrind", &workload.dir).args([
                "--tool=dhat".into(),
                format!(
                    "--dhat-out-file={}",
                    workload.output.join(format!("{name}.dhat.out")).display()
                ),
            ]),
        }
        .args(std::iter::once(base.program).chain(base.args));
        execute_workload(runtime, command, workload.input.as_deref())?;
    }
    Ok(())
}

fn redirected(
    runtime: &Runtime,
    spec: Invocation,
    input: Option<&Path>,
    output: &Path,
) -> Result<()> {
    if runtime.dry_run {
        runtime.plan(&spec)?;
        println!(
            "{}",
            json!({"action":"redirect","stdin":input,"stdout":output})
        );
        return Ok(());
    }
    let mut command = runtime.command(&spec);
    if let Some(input) = input {
        command.stdin(fs::File::open(input)?);
    } else {
        command.stdin(Stdio::null());
    }
    command.stdout(fs::File::create(output)?);
    let status = command.status()?;
    ensure!(status.success(), "{} failed with {status}", spec.program);
    Ok(())
}

fn collapse(runtime: &Runtime, input: &Path, output: &Path, label: &str) -> Result<PathBuf> {
    let script = output.join(format!("{label}.perf-script"));
    let folded = output.join(format!("{label}.folded"));
    redirected(
        runtime,
        Invocation::new("perf", &runtime.root).args([
            "script",
            "--input",
            &input.to_string_lossy(),
        ]),
        None,
        &script,
    )?;
    redirected(
        runtime,
        Invocation::new("inferno-collapse-perf", &runtime.root),
        Some(&script),
        &folded,
    )?;
    redirected(
        runtime,
        Invocation::new("inferno-flamegraph", &runtime.root),
        Some(&folded),
        &output.join(format!("{label}.svg")),
    )?;
    Ok(folded)
}

pub fn flamegraph(runtime: &Runtime, args: FlamegraphArgs) -> Result<()> {
    let input = runtime.path(args.input)?;
    check_file(&input)?;
    let baseline = args.baseline.map(|p| runtime.path(p)).transpose()?;
    if let Some(path) = &baseline {
        check_file(path)?;
    }
    let output = runtime.path(args.output)?;
    runtime.mkdir(&output)?;
    let folded = collapse(runtime, &input, &output, "candidate")?;
    if let Some(baseline) = baseline {
        let base = collapse(runtime, &baseline, &output, "baseline")?;
        for (name, first, second, negate) in [
            ("diff", &base, &folded, false),
            ("diff-reverse", &folded, &base, true),
        ] {
            let diff = output.join(format!("{name}.folded"));
            redirected(
                runtime,
                Invocation::new("inferno-diff-folded", &runtime.root).args([
                    first.to_string_lossy().into_owned(),
                    second.to_string_lossy().into_owned(),
                ]),
                None,
                &diff,
            )?;
            let mut command = Invocation::new("inferno-flamegraph", &runtime.root);
            if negate {
                command.args.push("--negate".into());
            }
            redirected(
                runtime,
                command,
                Some(&diff),
                &output.join(format!("{name}.svg")),
            )?;
        }
    }
    Ok(())
}

pub fn generate(runtime: &Runtime, args: GenerateArgs) -> Result<()> {
    let binary = runtime.path(args.filegen_bin)?;
    check_file(&binary)?;
    let dir = runtime.root.join("cases/performance/file_to_blackhole");
    let logs = dir.join("logs");
    ensure!(
        !fs::symlink_metadata(&logs).is_ok_and(|m| m.file_type().is_symlink()),
        "logs directory is a symlink"
    );
    runtime.mkdir(&logs)?;
    if args.reset_logs && logs.exists() {
        for entry in fs::read_dir(&logs)? {
            let entry = entry?;
            if entry.file_type()?.is_file() && entry.path().extension().is_some_and(|e| e == "log")
            {
                if runtime.dry_run {
                    println!(
                        "{}",
                        json!({"action":"remove-generated-log","path":entry.path()})
                    );
                } else {
                    fs::remove_file(entry.path())?;
                }
            }
        }
    }
    runtime.run(
        Invocation::new(&binary.to_string_lossy(), &dir).args(["--config-path", "filegen.toml"]),
    )
}

//! Sequential execution of the current native-product case catalog.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{ensure, Context, Result};
use clap::ValueEnum;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    cli::{CheckCase, MatrixArgs},
    delivery::subject::Subject,
    runtime::{sha256, Runtime, INTERRUPTED},
};

const CLEAN_ENV: &[&str] = &[
    "GEM_HOME",
    "GEM_PATH",
    "RUBYOPT",
    "JRUBY_OPTS",
    "JAVA_HOME",
    "LS_JAVA_HOME",
    "LS_JAVA_OPTS",
    "JAVA_TOOL_OPTIONS",
    "JDK_JAVA_OPTIONS",
    "LOGSTASH_HOME",
];
const POLL: Duration = Duration::from_millis(50);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Runtimes {
    subjects: BTreeMap<String, Product>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Product {
    binary: PathBuf,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

struct Batch<'a> {
    root: &'a Path,
    executable: &'a Path,
    args: &'a MatrixArgs,
}

fn pending(case: CheckCase, subject: Subject) -> Option<(&'static str, &'static str)> {
    match (case, subject) {
        (CheckCase::Sighup, Subject::Filebeat) => Some(("unsupported", "Filebeat reloads external input/module configuration, not its output; this case requires a same-process output switch")),
        (CheckCase::Sighup, Subject::SplunkHf | Subject::SplunkUf) => Some(("unsupported", "Splunk requires an instance restart after outputs.conf changes; that does not satisfy this SIGHUP contract")),
        (CheckCase::TcpToHttp, Subject::SplunkHf | Subject::SplunkUf) => Some(("pending_protocol", "A native Splunk HTTP output and matching verifier protocol have not been established")),
        (CheckCase::DockerPartial, _) if !cfg!(target_os = "linux") => Some(("unavailable_environment", "Docker fragment checks require a local Linux Docker daemon and host log-file access")),
        (case, subject) if case.is_standalone() => crate::delivery::standalone::prerequisite(case, subject),
        _ => None,
    }
}

fn save(output: &Path, report: &mut Value) -> Result<()> {
    let mut counts = BTreeMap::<String, usize>::new();
    for row in report["rows"].as_array().context("matrix rows missing")? {
        *counts
            .entry(row["outcome"].as_str().unwrap_or("errored").to_owned())
            .or_default() += 1;
    }
    let pending = counts
        .iter()
        .filter(|(name, _)| name.starts_with("pending_"))
        .map(|(_, count)| count)
        .sum::<usize>();
    report["summary"] = json!({"outcomes":counts,"pending_cases":pending,"ranking":false});
    let temporary = output.join("matrix.json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(report)?)?;
    fs::rename(temporary, output.join("matrix.json"))?;
    Ok(())
}

fn read_report(path: &Path) -> Result<Value> {
    ensure!(
        fs::metadata(path)?.len() <= 8 * 1024 * 1024,
        "case report exceeds 8 MiB"
    );
    serde_json::from_slice(&fs::read(path)?).context("read child case report")
}

fn signal(pid: u32, name: &str, group: bool) {
    if pid <= 1 {
        return;
    }
    let target = if group {
        format!("-{pid}")
    } else {
        pid.to_string()
    };
    let _ = Command::new("/bin/kill")
        .args([&format!("-{name}"), "--", &target])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Only groups owned by this live wrapper's descendants are eligible for
/// emergency cleanup. Never infer ownership from a product name or global PID.
fn owned_groups(wrapper: u32, case_output: &Path) -> BTreeSet<u32> {
    let Ok(output) = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,pgid="])
        .output()
    else {
        return BTreeSet::new();
    };
    if !output.status.success() {
        return BTreeSet::new();
    }
    let processes = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let values = line
                .split_whitespace()
                .map(str::parse::<u32>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .ok()?;
            (values.len() == 3).then(|| (values[0], values[1], values[2]))
        })
        .collect::<Vec<_>>();
    let mut children = BTreeSet::from([wrapper]);
    loop {
        let old = children.len();
        for (pid, parent, _) in &processes {
            if children.contains(parent) {
                children.insert(*pid);
            }
        }
        if old == children.len() {
            break;
        }
    }
    // Read this unique case's declared subject PID before emergency cleanup.
    // Its process group must also be present in the live owned descendant tree.
    let subject = read_report(&case_output.join("result.json"))
        .ok()
        .and_then(|report| report["process"]["pid"].as_u64())
        .and_then(|pid| u32::try_from(pid).ok());
    let mut groups = BTreeSet::new();
    for (pid, _, group) in processes {
        if children.contains(&pid) && group > 1 && children.contains(&group) {
            groups.insert(group);
        }
        if subject == Some(pid) && children.contains(&pid) && group == pid {
            groups.insert(group);
        }
    }
    groups
}

struct Wrapper {
    child: Child,
    output: PathBuf,
    reaped: bool,
}

impl Wrapper {
    fn interrupt(&mut self) -> Result<(ExitStatus, Value)> {
        let mut groups = owned_groups(self.child.id(), &self.output);
        signal(self.child.id(), "INT", false);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait()? {
                self.reaped = true;
                return Ok((status, json!({"interrupt_sent":true,"forced":false})));
            }
            thread::sleep(POLL);
        }
        groups.extend(owned_groups(self.child.id(), &self.output));
        for group in &groups {
            signal(*group, "TERM", true);
        }
        thread::sleep(Duration::from_millis(100));
        for group in &groups {
            signal(*group, "KILL", true);
        }
        let _ = self.child.kill();
        let status = self.child.wait()?;
        self.reaped = true;
        Ok((
            status,
            json!({"interrupt_sent":true,"forced":true,"owned_process_groups":groups}),
        ))
    }
}

impl Drop for Wrapper {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.interrupt();
        }
    }
}

fn run_case(
    batch: &Batch<'_>,
    case: CheckCase,
    subject: Subject,
    product: &Product,
    output: &Path,
    row: &mut Value,
) -> Result<()> {
    let (root, executable, args) = (batch.root, batch.executable, batch.args);
    let timeout_secs = args.timeout_for(case);
    let parent = output.parent().context("case output has no parent")?;
    fs::create_dir_all(parent)?;
    let prefix = output
        .file_name()
        .context("case output has no name")?
        .to_string_lossy();
    let stdout = parent.join(format!("{prefix}.stdout.log"));
    let stderr = parent.join(format!("{prefix}.stderr.log"));
    let mut child_args = vec![
        "--no-dotenv".to_owned(),
        "--root".into(),
        root.to_string_lossy().into_owned(),
        "check".into(),
        case.name().into(),
        "--subject".into(),
        subject.name().into(),
        "--subject-bin".into(),
        product.binary.to_string_lossy().into_owned(),
        "--timeout-secs".into(),
        timeout_secs.to_string(),
        "--output".into(),
        output.to_string_lossy().into_owned(),
    ];
    if case.is_workload() {
        child_args.extend([
            "--events".into(),
            args.events.to_string(),
            "--seed".into(),
            args.seed.to_string(),
        ]);
    }
    row["command"] = json!({"program":executable,"args":child_args,"cwd":root,"env_keys":product.env.keys().collect::<Vec<_>>(),"unset_env":CLEAN_ENV});
    row["raw_report_path"] = json!(output.join("result.json"));
    row["stdout_path"] = json!(stdout);
    row["stderr_path"] = json!(stderr);
    let mut command = Command::new(executable);
    command
        .current_dir(root)
        .args(&child_args)
        .stdin(Stdio::null())
        .stdout(fs::File::create(&stdout)?)
        .stderr(fs::File::create(&stderr)?);
    for key in CLEAN_ENV {
        command.env_remove(key);
    }
    command.envs(&product.env);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut wrapper = Wrapper {
        child: command.spawn().context("start isolated case wrapper")?,
        output: output.to_owned(),
        reaped: false,
    };
    row["wrapper_pid"] = json!(wrapper.child.id());
    let started = Instant::now();
    let deadline = started + Duration::from_secs(timeout_secs + 10);
    let mut watchdog = None;
    let status = loop {
        if let Some(status) = wrapper.child.try_wait()? {
            wrapper.reaped = true;
            break status;
        }
        let interrupted = INTERRUPTED.load(Ordering::SeqCst);
        if interrupted || Instant::now() >= deadline {
            let (status, cleanup) = wrapper.interrupt()?;
            row["wrapper_cleanup"] = cleanup;
            watchdog = Some(if interrupted {
                "interrupted"
            } else {
                "timed_out"
            });
            break status;
        }
        thread::sleep(POLL);
    };
    row["wrapper_exit_code"] = json!(status.code());
    row["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    match read_report(&output.join("result.json")) {
        Ok(report) => {
            row["raw_report_exists"] = json!(true);
            row["subject_version"] = report["subject"]["version"].clone();
            row["reason"] = report["reason"].clone();
            let passed = status.success()
                && report["passed"] == true
                && report["outcome"] == "passed"
                && watchdog.is_none();
            row["outcome"] = json!(watchdog.unwrap_or(if passed {
                "passed"
            } else {
                match report["outcome"].as_str() {
                    Some("failed") => "failed",
                    Some("timed_out") => "timed_out",
                    Some("interrupted") => "interrupted",
                    Some("unsupported") => "unsupported",
                    Some("unavailable_environment") => "unavailable_environment",
                    Some("pending_runner") => "pending_runner",
                    Some("pending_protocol") => "pending_protocol",
                    Some("pending_counter") => "pending_counter",
                    _ => "errored",
                }
            }));
            let verification = &report["verification"];
            let count_only = verification["kind"] == "terminal_count_only";
            let observed = verification["unique"]
                .as_u64()
                .or_else(|| verification["processed"].as_u64())
                .or_else(|| {
                    verification["receivers"]
                        .as_object()?
                        .values()
                        .try_fold(0_u64, |sum, value| {
                            sum.checked_add(value["unique"].as_u64()?)
                        })
                })
                .or_else(|| {
                    let variants = report["variants"].as_array()?;
                    let count =
                        variants.first()?["full_fixture"]["verification"]["received"].as_u64()?;
                    variants
                        .iter()
                        .all(|variant| {
                            variant["full_fixture"]["verification"]["received"].as_u64()
                                == Some(count)
                        })
                        .then_some(count)
                });
            row["observed_records"] = json!(observed);
            row["verified_records"] = json!(if passed { observed } else { None });
            row["verification_basis"] = json!(verification["kind"].as_str().or_else(|| {
                (verification["unique"].is_u64() || verification["receivers"].is_object())
                    .then_some("application_records")
            }));
            row["payloads_verified"] = json!(
                passed
                    && verification["payloads_verified"].as_bool().unwrap_or(
                        !count_only
                            && (verification["unique"].is_u64()
                                || verification["receivers"].is_object())
                    )
            );
            if let Some(variants) = report["variants"].as_array() {
                row["variants"] = json!(variants.iter().map(|variant| json!({"name":variant["name"],"full_fixture_records":variant["full_fixture"]["verification"]["received"],"full_fixture_passed":variant["full_fixture"]["passed"],"canary_records":variant["canary"]["verification"]["received"],"canary_passed":variant["canary"]["passed"]})).collect::<Vec<_>>());
            }
            row["measured_records"] = report["fixture"]["measured"].clone();
            if passed
                && case.is_workload()
                && !matches!(case, CheckCase::TcpToBlackhole)
                && report["measurement"]["status"] == "verified"
            {
                row["finite_seconds"] = json!(report["measurement"]["seconds"]
                    .as_f64()
                    .filter(|value| value.is_finite() && *value > 0.0));
            }
        }
        Err(error) => {
            row["outcome"] = json!(watchdog.unwrap_or("errored"));
            row["reason"] = json!(format!("case wrapper exited without a readable final report: {error:#}; inspect saved stderr"));
        }
    }
    if let Some(outcome) = watchdog {
        row["reason"] = json!(if outcome == "interrupted" {
            "matrix interrupted; case wrapper cleanup requested"
        } else {
            "case wrapper exceeded its budget plus cleanup allowance"
        });
    }
    Ok(())
}

pub fn run(runtime: &Runtime, args: MatrixArgs) -> Result<i32> {
    ensure!(
        cfg!(unix),
        "matrix execution currently requires a Unix host"
    );
    let config = runtime
        .path(&args.runtimes)?
        .canonicalize()
        .context("resolve runtimes JSON")?;
    let mut runtimes: Runtimes =
        serde_json::from_slice(&fs::read(&config)?).context("parse runtimes JSON")?;
    let mut canonical = BTreeMap::new();
    for (name, mut product) in std::mem::take(&mut runtimes.subjects) {
        let subject = Subject::from_str(&name, false).map_err(anyhow::Error::msg)?;
        for (key, value) in &product.env {
            ensure!(
                !key.is_empty() && !key.contains(['=', '\0']) && !value.contains('\0'),
                "invalid runtime environment entry"
            );
        }
        if product.binary.is_relative() {
            product.binary = config.parent().unwrap().join(&product.binary);
        }
        if let Ok(path) = product.binary.canonicalize() {
            product.binary = path;
        }
        ensure!(
            canonical
                .insert(subject.name().to_owned(), product)
                .is_none(),
            "duplicate runtime subject alias"
        );
    }
    runtimes.subjects = canonical;
    let subjects = Subject::value_variants()
        .iter()
        .copied()
        .filter(|subject| {
            args.subjects.is_empty()
                || args
                    .subjects
                    .iter()
                    .any(|wanted| wanted.name() == subject.name())
        })
        .collect::<Vec<_>>();
    let cases = CheckCase::value_variants()
        .iter()
        .copied()
        .filter(|case| {
            args.cases.is_empty() || args.cases.iter().any(|wanted| wanted.name() == case.name())
        })
        .collect::<Vec<_>>();
    let output = runtime.path(&args.output)?;
    ensure!(
        !output.exists(),
        "output directory already exists: {}",
        output.display()
    );
    let executable = std::env::current_exe()?.canonicalize()?;
    let mut provenance = serde_json::Map::new();
    for (subject, product) in &runtimes.subjects {
        let digest = if product.binary.is_file() {
            sha256(&product.binary).ok()
        } else {
            None
        };
        provenance.insert(subject.clone(), json!({"binary":product.binary,"binary_sha256":digest,"env_keys":product.env.keys().collect::<Vec<_>>()}));
    }
    let mut rows = Vec::new();
    for case in &cases {
        for subject in &subjects {
            for run in 1..=args.runs {
                let (outcome, reason) = if let Some((outcome, reason)) = pending(*case, *subject) {
                    (outcome, Some(reason.to_owned()))
                } else if !runtimes
                    .subjects
                    .get(subject.name())
                    .is_some_and(|product| product.binary.is_file())
                {
                    (
                        "missing_binary",
                        Some("No existing binary is configured for this product".to_owned()),
                    )
                } else {
                    ("queued", None)
                };
                rows.push(json!({"case":case.name(),"subject":subject.name(),"run":run,"outcome":outcome,"reason":reason,
                    "verified_records":null,"observed_records":null,"verification_basis":null,"payloads_verified":false,"measured_records":null,
                    "finite_seconds":null,"subject_version":null,"raw_report_path":null,"raw_report_exists":false,"timeout_secs":args.timeout_for(*case)}));
            }
        }
    }
    let mut report = json!({"schema_version":1,"state":"planned","execution_complete":false,"all_requested_cases_verified":false,
        "created_unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),"performance_comparison":false,
        "runner":{"binary":executable,"sha256":sha256(&executable)?,"version":env!("CARGO_PKG_VERSION")},
        "runtimes":{"source":config,"sha256":sha256(&config)?,"subjects":provenance},
        "catalog":{"cases":CheckCase::value_variants().iter().map(|case| case.name()).collect::<Vec<_>>(),"subjects":Subject::value_variants().iter().map(|subject|subject.name()).collect::<Vec<_>>()},
        "selection":{"cases":cases.iter().map(|case|case.name()).collect::<Vec<_>>(),"subjects":subjects.iter().map(|subject|subject.name()).collect::<Vec<_>>(),"runs":args.runs},
        "finite_workload":{"events":args.events,"seed":args.seed,"timeout_secs":args.timeout_for(CheckCase::TcpToTcp)},
        "timeout_policy":{"override_secs":args.timeout_secs,"default_secs":90,"klog_default_secs":300},
        "scope":"sequential same-host verification; native unsupported and pending backends remain visible; no ranking or capacity claim",
        "execution":{"concurrent_cases":1,"exclusive_host":false,"headroom_calibrated":false},
        "os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"rows":rows});
    if runtime.dry_run {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(0);
    }
    fs::create_dir_all(output.parent().context("matrix output has no parent")?)?;
    fs::create_dir(&output)?;
    println!("{}", output.join("matrix.json").display());
    report["state"] = json!("running");
    save(&output, &mut report)?;
    let rows = report["rows"].as_array().unwrap().len();
    let batch = Batch {
        root: &runtime.root,
        executable: &executable,
        args: &args,
    };
    for index in 0..rows {
        if report["rows"][index]["outcome"] != "queued" {
            continue;
        }
        if INTERRUPTED.load(Ordering::SeqCst) {
            report["rows"][index]["outcome"] = json!("not_run");
            report["rows"][index]["reason"] = json!("matrix interrupted before this row started");
            continue;
        }
        let mut row = report["rows"][index].clone();
        let case = CheckCase::from_str(row["case"].as_str().unwrap(), false)
            .map_err(anyhow::Error::msg)?;
        let subject = Subject::from_str(row["subject"].as_str().unwrap(), false)
            .map_err(anyhow::Error::msg)?;
        let case_output = output
            .join("cases")
            .join(case.name())
            .join(subject.name())
            .join(format!("run-{:03}", row["run"].as_u64().unwrap()));
        row["outcome"] = json!("running");
        row["case_output"] = json!(case_output);
        row["raw_report_path"] = json!(case_output.join("result.json"));
        report["rows"][index] = row.clone();
        save(&output, &mut report)?;
        if let Err(error) = run_case(
            &batch,
            case,
            subject,
            &runtimes.subjects[subject.name()],
            &case_output,
            &mut row,
        ) {
            row["outcome"] = json!("errored");
            row["reason"] = json!(format!("{error:#}"));
        }
        report["rows"][index] = row;
        save(&output, &mut report)?;
    }
    let rows = report["rows"].as_array().unwrap();
    let failed = rows.iter().any(|row| {
        matches!(
            row["outcome"].as_str(),
            Some("failed" | "errored" | "timed_out")
        )
    });
    let incomplete = rows
        .iter()
        .any(|row| !matches!(row["outcome"].as_str(), Some("passed" | "unsupported")));
    report["all_requested_cases_verified"] =
        json!(rows.iter().all(|row| row["outcome"] == "passed"));
    report["all_supported_cases_verified"] = json!(!incomplete);
    report["execution_complete"] = json!(true);
    report["state"] = json!(if INTERRUPTED.load(Ordering::SeqCst) {
        "interrupted"
    } else {
        "finished"
    });
    save(&output, &mut report)?;
    Ok(if INTERRUPTED.load(Ordering::SeqCst) {
        130
    } else if failed {
        1
    } else if incomplete {
        2
    } else {
        0
    })
}

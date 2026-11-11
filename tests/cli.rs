#![cfg(unix)]

use std::{
    fs,
    io::Write,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::OnceLock,
    time::{Duration, Instant},
};

use flate2::{write::GzEncoder, Compression};
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};

fn tools() -> &'static TempDir {
    static TOOLS: OnceLock<TempDir> = OnceLock::new();
    TOOLS.get_or_init(|| {
        let dir = tempdir().unwrap();
        let executable = dir.path().join("fixture");
        let status = Command::new("rustc")
            .args(["--edition=2021", "--crate-name=vth_fixture"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tool.rs"))
            .arg("-o")
            .arg(&executable)
            .status()
            .unwrap();
        assert!(status.success());
        for name in [
            "aws",
            "terraform",
            "ansible-playbook",
            "curl",
            "ssh",
            "packer",
            "vector",
            "socat",
            "perf",
            "inferno-collapse-perf",
            "inferno-flamegraph",
            "inferno-diff-folded",
        ] {
            symlink(&executable, dir.path().join(name)).unwrap();
        }
        dir
    })
}

struct Fixture {
    dir: TempDir,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("vth fixture ")
            .tempdir()
            .unwrap();
        let root = dir.path();
        for name in [
            "ansible",
            "config",
            "cases/example_performance/terraform",
            "cases/example_performance/ansible",
            "cases/example_performance/configurations/default",
            "cases/performance/stdin_to_blackhole",
            "cases/performance/file_to_blackhole",
            "cases/wrapped_json_correctness/ansible/config_files",
        ] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        fs::write(root.join("cases/example_performance/terraform/main.tf"), "").unwrap();
        fs::write(
            root.join("cases/example_performance/ansible/bootstrap.yml"),
            "- hosts: all\n  roles:\n    - role: vector\n    - role: fluentbit\n",
        )
        .unwrap();
        for name in [
            "cases/example_performance/ansible/run.yml",
            "cases/example_performance/configurations/default/ansible.yml",
            "cases/example_performance/configurations/default/terraform.tfvars",
            "cases/performance/stdin_to_blackhole/vector.yaml",
        ] {
            fs::write(root.join(name), "").unwrap();
        }
        for name in ["subjects.json", "metrics.json"] {
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("config")
                    .join(name),
                root.join("config").join(name),
            )
            .unwrap();
        }
        let config = "cases/wrapped_json_correctness/ansible/config_files/vector.yaml";
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(config),
            root.join(config),
        )
        .unwrap();
        Self { dir }
    }
    fn root(&self) -> &Path {
        self.dir.path()
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_vth"));
        command
            .current_dir(self.root())
            .env_clear()
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", tools().path().display()),
            )
            .env("HOME", self.root())
            .env("USER", "tester")
            .env("VECTOR_TEST_USER_ID", "tester")
            .env("VECTOR_TEST_SSH_PRIVATE_KEY", self.root().join("a key"))
            .env("VECTOR_TEST_SSH_PUBLIC_KEY", self.root().join("a key.pub"))
            .env("VECTOR_TEST_STATE_S3_BUCKET_NAME", "test-state")
            .env("VECTOR_TEST_RESULTS_S3_BUCKET_NAME", "test-results")
            .env("VTH_TEST_LOG", self.root().join("invocations.jsonl"));
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        let output = self.command().args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
    fn invocations(&self) -> Vec<Value> {
        fs::read_to_string(self.root().join("invocations.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn athena(&self) -> (PathBuf, PathBuf) {
        let columns = [
            ("name", "varchar"),
            ("configuration", "varchar"),
            ("subject", "varchar"),
            ("version", "varchar"),
            ("count", "bigint"),
            ("duration_avg", "double"),
            ("disk_read_avg", "double"),
            ("net_recv_avg", "double"),
            ("mem_used_max", "double"),
        ];
        let metadata = json!({"ColumnInfo":columns.iter().map(|(name,kind)|json!({"Name":name,"Type":kind})).collect::<Vec<_>>()});
        let header = json!({"Data":columns.iter().map(|(name,_)|json!({"VarCharValue":name})).collect::<Vec<_>>()});
        let row = |subject: &str, duration: &str| json!({"Data":(["example_performance","default",subject,"0.55.0","2",duration,"1048576","2097152","3145728"].iter().map(|v|json!({"VarCharValue":v})).collect::<Vec<_>>())});
        let first = self.root().join("first.json");
        let second = self.root().join("second.json");
        fs::write(&first,json!({"ResultSet":{"ResultSetMetadata":metadata,"Rows":[header,row("vector","1.0")]},"NextToken":"page-two"}).to_string()).unwrap();
        fs::write(
            &second,
            json!({"ResultSet":{"ResultSetMetadata":metadata,"Rows":[row("fluentbit","2.0")]}})
                .to_string(),
        )
        .unwrap();
        (first, second)
    }
}

fn args(record: &Value) -> Vec<&str> {
    record["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

#[test]
fn run_plans_and_executes_the_existing_remote_lifecycle() {
    let fixture = Fixture::new();
    fixture.run(&[
        "run",
        "-t",
        "example_performance",
        "-s",
        "vector",
        "--ssh-cidr",
        "203.0.113.2/32",
    ]);
    let records = fixture.invocations();
    assert_eq!(
        records
            .iter()
            .map(|r| r["program"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "terraform",
            "terraform",
            "ansible-playbook",
            "ansible-playbook"
        ]
    );
    let init = args(&records[0]);
    assert!(init.contains(
        &"-backend-config=key=vector-test-case/example_performance/default/tester.tfstate"
    ));
    assert!(!init.contains(&"-var"));
    let apply = args(&records[1]);
    assert!(apply.contains(&"ssh_cidr=203.0.113.2/32"));
    assert!(apply
        .iter()
        .any(|a| a.starts_with("pub_key=") && a.ends_with("a key.pub")));
    let bootstrap = args(&records[2]);
    let vars: Value =
        serde_json::from_str(bootstrap[bootstrap.iter().position(|a| a.starts_with('{')).unwrap()])
            .unwrap();
    assert_eq!(
        vars["test_namespace"],
        "tag_TestName_example_performance:&tag_TestConfiguration_default:&tag_TestUserID_tester"
    );
    assert_eq!(args(&records[3]).last(), Some(&"run.yml"));
}

#[test]
fn terraform_failure_stops_before_ansible() {
    let fixture = Fixture::new();
    let output = fixture
        .command()
        .env("VTH_TEST_MODE", "failed-terraform")
        .args([
            "run",
            "-t",
            "example_performance",
            "--ssh-cidr",
            "203.0.113.2/32",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(fixture
        .invocations()
        .iter()
        .all(|r| r["program"] == "terraform"));
}

#[test]
fn dry_run_has_no_external_or_filesystem_side_effects() {
    let fixture = Fixture::new();
    let output = fixture.run(&[
        "--dry-run",
        "run",
        "-t",
        "example_performance",
        "-s",
        "vector",
        "--ssh-cidr",
        "203.0.113.2/32",
    ]);
    let plans: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(plans.len(), 4);
    assert_eq!(plans[0]["env"]["TF_DATA_DIR"], ".terraform/default/tester");
    assert!(fixture.invocations().is_empty());
    assert!(!fixture
        .root()
        .join("cases/example_performance/terraform/.terraform")
        .exists());
    fixture.run(&[
        "container",
        "run",
        "-t",
        "example_performance",
        "--dry-run",
        "--ssh-cidr",
        "203.0.113.2/32",
    ]);
    fixture.run(&[
        "--dry-run",
        "check",
        "wrapped_json_correctness",
        "--vector-bin",
        tools().path().join("vector").to_str().unwrap(),
        "--output",
        "local-check",
    ]);
    assert!(fixture.invocations().is_empty());
    assert!(!fixture.root().join("local-check").exists());
}

#[test]
fn local_check_failures_retain_artifacts_and_reap_vector() {
    let fixture = Fixture::new();
    for (name, mode, delay, code, outcome, forced) in [
        ("exited", "failed-vector", "0", 1, "failed", false),
        ("stalled", "", "10000", 2, "timed_out", true),
    ] {
        let started = Instant::now();
        let output = fixture
            .command()
            .env("VTH_TEST_MODE", mode)
            .env("VTH_TEST_DELAY_MS", delay)
            .args(["check", "wrapped_json_correctness", "--vector-bin"])
            .arg(tools().path().join("vector"))
            .args(["--timeout-secs", "1", "--output", name])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code));
        assert!(started.elapsed() < Duration::from_secs(5));
        let artifacts = fixture.root().join(name);
        let report: Value =
            serde_json::from_slice(&fs::read(artifacts.join("result.json")).unwrap()).unwrap();
        assert_eq!(report["outcome"], outcome);
        assert_eq!(report["complete"], false);
        assert_eq!(report["cleanup"]["forced"], forced);
        assert_eq!(report["cleanup"]["vector_reaped"], true);
        for file in [
            "initial.yaml",
            "input.jsonl",
            "expected.json",
            "received.jsonl",
            "vector.stdout.log",
            "vector.stderr.log",
        ] {
            assert!(artifacts.join(file).is_file(), "missing {file}");
        }
        let probe = Command::new("/bin/kill")
            .args(["-0", "--", &report["vector_pid"].to_string()])
            .output()
            .unwrap();
        assert!(!probe.status.success(), "Vector process survived cleanup");
    }
    assert!(fixture
        .invocations()
        .iter()
        .all(|r| r["program"] == "vector"));
}

#[test]
fn bootstrap_modes_and_reused_instances() {
    let fixture = Fixture::new();
    fixture.run(&[
        "run",
        "-t",
        "example_performance",
        "-s",
        "vector",
        "--run-only",
    ]);
    assert_eq!(fixture.invocations().len(), 1);
    assert_eq!(args(&fixture.invocations()[0]).last(), Some(&"run.yml"));
    fs::remove_file(fixture.root().join("invocations.jsonl")).unwrap();
    fixture.run(&[
        "run",
        "-t",
        "example_performance",
        "--bootstrap-only",
        "--skip-terraform",
    ]);
    assert_eq!(fixture.invocations().len(), 1);
    assert_eq!(
        args(&fixture.invocations()[0]).last(),
        Some(&"bootstrap.yml")
    );
}

#[test]
fn invalid_cases_and_subjects_fail_before_commands() {
    let fixture = Fixture::new();
    for extra in [
        ["-t", "../example_performance"],
        ["-c", "missing"],
        ["-s", "filebeat"],
    ] {
        let output = fixture
            .command()
            .args(["run", "-t", "example_performance"])
            .args(extra)
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
    assert!(fixture.invocations().is_empty());
}

#[test]
fn teardown_initializes_the_same_scoped_backend() {
    let fixture = Fixture::new();
    let output = fixture.run(&[
        "teardown",
        "-t",
        "example_performance",
        "--yes",
        "--ssh-cidr",
        "203.0.113.2/32",
    ]);
    assert!(output.status.success());
    let invocations = fixture.invocations();
    assert_eq!(args(&invocations[0])[0], "init");
    assert_eq!(args(&invocations[1])[0], "destroy");
    assert!(args(&invocations[1]).contains(&"-auto-approve"));
}

#[test]
fn dotenv_is_data_and_never_executes_envrc_or_shell_substitution() {
    let fixture = Fixture::new();
    let marker = fixture.root().join("should-not-exist");
    fs::write(
        fixture.root().join(".env"),
        format!(
            "VECTOR_TEST_NAME=example_performance\nMARKER='$(touch {})'\n",
            marker.display()
        ),
    )
    .unwrap();
    fs::write(
        fixture.root().join(".envrc"),
        format!("touch '{}'\n", marker.display()),
    )
    .unwrap();
    let mut command = fixture.command();
    command.current_dir(fixture.root().join("cases/example_performance/ansible"));
    let output = command
        .args(["run", "--run-only", "-s", "vector"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists());
}

#[test]
fn athena_pages_are_combined_and_failed_queries_fail() {
    let fixture = Fixture::new();
    let (one, two) = fixture.athena();
    let output = fixture
        .command()
        .env("VTH_TEST_PAGE_ONE", &one)
        .env("VTH_TEST_PAGE_TWO", &two)
        .args(["compare", "-t", "example_performance", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(rows[0]["subject"], "vector");
    assert!(fixture
        .invocations()
        .iter()
        .any(|r| args(r).contains(&"page-two")));
    let output = fixture
        .command()
        .env("VTH_TEST_MODE", "failed-query")
        .args(["compare", "-t", "example_performance"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("synthetic failure"));
}

#[test]
fn website_export_preserves_the_existing_shape_and_throughput_metric() {
    let fixture = Fixture::new();
    let (one, _) = fixture.athena();
    fixture.run(&[
        "export",
        "--input",
        one.to_str().unwrap(),
        "--no-upload",
        "--output",
        "export.json",
    ]);
    let doc: Value =
        serde_json::from_slice(&fs::read(fixture.root().join("export.json")).unwrap()).unwrap();
    let performance = &doc["performance_tests"];
    for field in ["tests", "subjects", "measurements", "metrics"] {
        assert!(performance[field].is_array());
    }
    let throughput = performance["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["metric"] == "throughput_avg")
        .unwrap();
    assert_eq!(throughput["value"], 2_097_152.0);
    assert_eq!(performance["subjects"][0]["slug"], "vector");
    assert!(fixture.invocations().is_empty());
}

#[test]
fn s3_results_select_the_latest_timestamp_and_surface_aws_failures() {
    let fixture = Fixture::new();
    let output = fixture.run(&["results", "-t", "example_performance", "--json"]);
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows[0]["mib_per_second"], 2.0);
    let invocations = fixture.invocations();
    let copy = invocations
        .iter()
        .find(|r| args(r).contains(&"cp"))
        .unwrap();
    assert!(args(copy).iter().any(|a| a.contains("timestamp=10/")));
    let output = fixture
        .command()
        .env("VTH_TEST_MODE", "forbidden")
        .args(["bootstrap-aws", "--account-id", "123456789012"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!fixture
        .invocations()
        .iter()
        .any(|r| args(r).contains(&"create-bucket")));
    assert!(!fixture.root().join(".env").exists());
}

#[test]
fn standalone_benchmark_streams_gzip_and_records_raw_samples() {
    let fixture = Fixture::new();
    let input = fixture.root().join("input fixture.gz");
    let mut gzip = GzEncoder::new(fs::File::create(&input).unwrap(), Compression::default());
    gzip.write_all(b"one\ntwo\nthree\n").unwrap();
    gzip.finish().unwrap();
    let binary = tools().path().join("vector");
    let output = fixture
        .command()
        .env("VTH_TEST_BYTES", fixture.root().join("bytes"))
        .args([
            "bench",
            "stdin_to_blackhole",
            "--vector-bin",
            binary.to_str().unwrap(),
            "--baseline-bin",
            binary.to_str().unwrap(),
            "--input",
            input.to_str().unwrap(),
            "--runs",
            "2",
            "--warmup",
            "0",
            "--output",
            "out",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(fixture.root().join("bytes")).unwrap(),
        "14"
    );
    let results: Value =
        serde_json::from_slice(&fs::read(fixture.root().join("out/benchmark.json")).unwrap())
            .unwrap();
    assert_eq!(results["results"].as_array().unwrap().len(), 2);
    assert_eq!(
        results["results"][0]["seconds"].as_array().unwrap().len(),
        2
    );
    assert_eq!(results["input_sha256"].as_str().unwrap().len(), 64);
    assert!(results["harness_revision"].is_null());
    assert_eq!(results["harness_binary_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(
        fixture
            .invocations()
            .iter()
            .filter(|r| r["program"] == "vector")
            .count(),
        4
    );
}

#[test]
fn flamegraphs_use_checked_processes_and_keep_both_diff_directions() {
    let fixture = Fixture::new();
    fs::write(fixture.root().join("candidate.data"), "").unwrap();
    fs::write(fixture.root().join("baseline.data"), "").unwrap();
    fixture.run(&[
        "flamegraph",
        "--input",
        "candidate.data",
        "--baseline",
        "baseline.data",
        "--output",
        "graphs",
    ]);
    for name in [
        "candidate.svg",
        "baseline.svg",
        "diff.svg",
        "diff-reverse.svg",
    ] {
        assert!(fixture.root().join("graphs").join(name).is_file());
    }
}

#[test]
fn guest_timeout_and_long_message_preserve_the_existing_contract() {
    let fixture = Fixture::new();
    let output = Command::new(env!("CARGO_BIN_EXE_vth-guest"))
        .arg("longlog")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.len() > 16 * 1024);
    let message: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(message["message"], "Log text");
    assert_eq!(message["aaa"], "aaa_value");
    assert_eq!(message["bzz"], "bzz_value");
    fs::write(fixture.root().join("input"), "message\n").unwrap();
    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_vth-guest"))
        .env("PATH", tools().path())
        .env("VTH_TEST_DELAY_MS", "3000")
        .args([
            "socat-loop",
            "0.03",
            fixture.root().join("input").to_str().unwrap(),
            "-u",
            "-",
            "tcp:example:9000",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(124));
    assert!(start.elapsed() < Duration::from_secs(2));
}

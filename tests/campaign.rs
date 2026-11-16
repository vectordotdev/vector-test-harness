use std::{path::Path, process::Command};

use serde_json::{json, Value};
use tempfile::tempdir;

#[test]
fn campaign_plans_pinned_runs_and_rejects_unpinned_inputs_without_writes() {
    let temporary = tempdir().unwrap();
    let revision = "0123456789abcdef0123456789abcdef01234567";
    let invoke = |revision: &str, output: &Path, extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_vth"))
            .args([
                "--root",
                env!("CARGO_MANIFEST_DIR"),
                "--no-dotenv",
                "--dry-run",
                "campaign",
                "--revision",
                revision,
                "--output",
            ])
            .arg(output)
            .args(extra)
            .output()
            .unwrap()
    };

    let planned = temporary.path().join("planned");
    let output = invoke(
        revision,
        &planned,
        &[
            "--subjects",
            "vector,fluent-bit",
            "--cases",
            "tcp_to_tcp_performance,file_to_tcp_performance",
            "--events",
            "123",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(plan["revision"], revision);
    assert_eq!(
        plan["selection"]["subjects"],
        json!(["vector", "fluent-bit"])
    );
    assert_eq!(
        plan["selection"]["cases"],
        json!(["tcp_to_tcp_performance", "file_to_tcp_performance"])
    );
    assert_eq!(plan["selection"]["events"], 123);
    assert_eq!(plan["output"], planned.to_str().unwrap());
    assert!(!planned.exists());

    for (name, selected_revision, extra, reason) in [
        (
            "mutable-revision",
            "HEAD",
            vec![],
            "full lowercase 40-character",
        ),
        (
            "parent-lock",
            revision,
            vec!["--lock", "../escape.json"],
            "repository-relative",
        ),
        (
            "docker",
            revision,
            vec!["--cases", "docker_partial_events_merging_correctness"],
            "prerequisites are not yet pinned",
        ),
        (
            "profile",
            revision,
            vec!["--cases", "file_to_blackhole"],
            "prerequisites are not yet pinned",
        ),
    ] {
        let destination = temporary.path().join(name);
        let output = invoke(selected_revision, &destination, &extra);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{name} unexpectedly passed");
        assert!(stderr.contains(reason), "{name}: {stderr}");
        assert!(!destination.exists(), "{name} created output");
    }
}

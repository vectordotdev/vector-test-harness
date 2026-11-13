//! Small execution receipts; CPU affinity is placement, not exclusive reservation.

use std::{fs, process::Command};

use anyhow::{ensure, Result};
use serde_json::{json, Value};

use crate::cli::parse_cpu_list;

fn read(path: &str) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == key).then(|| value.trim().to_owned())
    })
}

pub(super) fn affinity(pid: u32) -> Option<String> {
    field(&read(&format!("/proc/{pid}/status"))?, "Cpus_allowed_list")
}

pub(super) fn capture() -> Value {
    let os_release = read("/etc/os-release").and_then(|text| {
        text.lines().find_map(|line| {
            line.strip_prefix("PRETTY_NAME=")
                .map(|value| value.trim_matches('"').to_owned())
        })
    });
    let kernel_release = read("/proc/sys/kernel/osrelease")
        .map(|value| value.trim().to_owned())
        .or_else(|| {
            let output = Command::new("uname").args(["-s", "-r"]).output().ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        });
    let cpu_model = read("/proc/cpuinfo")
        .and_then(|text| field(&text, "model name").or_else(|| field(&text, "Hardware")));
    let logical_cpus = read("/sys/devices/system/cpu/online")
        .and_then(|text| parse_cpu_list(text.trim()).ok())
        .map(|cpus| cpus.len());
    let memory_bytes = read("/proc/meminfo")
        .and_then(|text| field(&text, "MemTotal"))
        .and_then(|value| value.strip_suffix("kB").map(str::trim).map(str::to_owned))
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|kilobytes| kilobytes.checked_mul(1024));
    json!({
        "os_release":os_release,"kernel_release":kernel_release,
        "cpu_model":cpu_model,"logical_cpus":logical_cpus,"memory_bytes":memory_bytes,
        "harness_cpus":affinity(std::process::id()),
        "available_parallelism":std::thread::available_parallelism().ok().map(usize::from),
        "metadata_basis":"Linux procfs/sysfs; unavailable fields are null",
    })
}

pub(super) fn placement(requested: Option<&str>, pid: u32) -> Result<Value> {
    let vector_cpus = affinity(pid);
    let harness_cpus = affinity(std::process::id());
    if let Some(requested) = requested {
        let observed = vector_cpus
            .as_deref()
            .and_then(|value| parse_cpu_list(value).ok());
        ensure!(
            observed == parse_cpu_list(requested).ok(),
            "Vector did not apply requested CPU affinity {requested} (observed {vector_cpus:?})"
        );
    }
    let disjoint_cpu_sets = harness_cpus
        .as_deref()
        .and_then(|value| parse_cpu_list(value).ok())
        .zip(
            vector_cpus
                .as_deref()
                .and_then(|value| parse_cpu_list(value).ok()),
        )
        .map(|(harness, vector)| harness.is_disjoint(&vector));
    Ok(json!({
        "requested_vector_cpus":requested,"observed_vector_cpus":vector_cpus,
        "observed_harness_cpus":harness_cpus,"disjoint_cpu_sets":disjoint_cpu_sets,
        "exclusive_reservation":false,
        "basis":"affinity observed after warmup; CPU placement does not reserve CPUs exclusively",
    }))
}

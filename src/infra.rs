use std::{
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::json;

use crate::{
    cli::{CaseArgs, MetaMode, RunArgs, TeardownArgs},
    runtime::{check_file, identifier, status_code, Invocation, Runtime},
};

#[derive(Debug, Deserialize)]
pub struct Subject {
    pub slug: String,
    pub name: String,
    pub preinstalled_version: Option<String>,
}

pub fn subjects(runtime: &Runtime) -> Result<Vec<Subject>> {
    serde_json::from_slice(&fs::read(runtime.root.join("config/subjects.json"))?)
        .context("reading subject catalog")
}

struct Case {
    name: String,
    configuration: String,
    user: String,
    dir: PathBuf,
}

fn case(runtime: &Runtime, args: &CaseArgs) -> Result<Case> {
    let name = args
        .test
        .clone()
        .or_else(|| runtime.get("VECTOR_TEST_NAME"))
        .context("supply --test or VECTOR_TEST_NAME")?;
    let configuration = args
        .configuration
        .clone()
        .or_else(|| runtime.get("VECTOR_TEST_CONFIGURATION"))
        .unwrap_or_else(|| "default".into());
    let user = args
        .user_id
        .clone()
        .or_else(|| runtime.get("VECTOR_TEST_USER_ID"))
        .context("supply --user-id or VECTOR_TEST_USER_ID")?;
    identifier(&name, "test name")?;
    identifier(&configuration, "configuration")?;
    identifier(&user, "user ID")?;
    let dir = runtime.root.join("cases").join(&name);
    ensure!(
        dir.join("terraform/main.tf").is_file(),
        "unknown or retired remote case {name}; use vth list for supported cases"
    );
    for file in [
        "terraform/main.tf".into(),
        "ansible/bootstrap.yml".into(),
        "ansible/run.yml".into(),
        format!("configurations/{configuration}/terraform.tfvars"),
        format!("configurations/{configuration}/ansible.yml"),
    ] {
        check_file(&dir.join(file))?;
    }
    Ok(Case {
        name,
        configuration,
        user,
        dir,
    })
}

fn terraform(_runtime: &Runtime, case: &Case) -> Invocation {
    Invocation::new("terraform", &case.dir.join("terraform"))
        .env("TF_IN_AUTOMATION", "true")
        .env(
            "TF_DATA_DIR",
            format!(".terraform/{}/{}", case.configuration, case.user),
        )
}

fn interactive(runtime: &Runtime) -> &'static str {
    if runtime.get("TF_INTERACTIVE").as_deref() == Some("true") {
        "-input=true"
    } else {
        "-input=false"
    }
}

fn terraform_init(runtime: &Runtime, case: &Case) -> Result<()> {
    let region = runtime
        .get("AWS_DEFAULT_REGION")
        .or_else(|| runtime.get("AWS_REGION"))
        .unwrap_or_else(|| "us-east-1".into());
    let mut init = terraform(runtime, case).args([
        "init".into(),
        interactive(runtime).into(),
        format!(
            "-backend-config=bucket={}",
            runtime.required("VECTOR_TEST_STATE_S3_BUCKET_NAME")?
        ),
        format!("-backend-config=region={region}"),
        "-backend-config=encrypt=true".into(),
        "-backend-config=dynamodb_table=TerraformLocks".into(),
        format!(
            "-backend-config=key=vector-test-case/{}/{}/{}.tfstate",
            case.name, case.configuration, case.user
        ),
    ]);
    if let Some(dir) = runtime.get("TF_PLUGIN_DIR") {
        init = init.args([
            "-plugin-dir".into(),
            runtime.path(dir)?.to_string_lossy().into_owned(),
        ]);
    }
    runtime.run(init)
}

fn ssh_cidr(runtime: &Runtime, supplied: Option<&str>) -> Result<String> {
    if let Some(cidr) = supplied
        .map(str::to_owned)
        .or_else(|| runtime.get("VECTOR_TEST_SSH_CIDR"))
    {
        let (ip, prefix) = cidr
            .split_once('/')
            .context("SSH CIDR needs a prefix, e.g. 203.0.113.1/32")?;
        let ip: IpAddr = ip.parse().context("invalid SSH CIDR IP address")?;
        let prefix: u8 = prefix.parse().context("invalid SSH CIDR prefix")?;
        ensure!(
            prefix <= if ip.is_ipv4() { 32 } else { 128 },
            "invalid SSH CIDR prefix"
        );
        return Ok(cidr);
    }
    let command = Invocation::new("curl", &runtime.root).args([
        "--fail",
        "--silent",
        "--show-error",
        "--max-time",
        "10",
        "https://checkip.amazonaws.com",
    ]);
    let output = runtime.capture(command)?;
    if runtime.dry_run {
        return Ok("<egress-ip>/32".into());
    }
    let ip: IpAddr = output
        .trim()
        .parse()
        .context("cannot discover egress IP; supply --ssh-cidr")?;
    Ok(format!("{ip}/{}", if ip.is_ipv4() { 32 } else { 128 }))
}

fn variables(runtime: &Runtime, case: &Case, cidr: &str) -> Result<Vec<String>> {
    let mut result = vec![];
    for (key, value) in [
        (
            "pub_key",
            runtime
                .env_path("VECTOR_TEST_SSH_PUBLIC_KEY")?
                .to_string_lossy()
                .into_owned(),
        ),
        ("test_name", case.name.clone()),
        ("test_configuration", case.configuration.clone()),
        ("user_id", case.user.clone()),
        (
            "results_s3_bucket_name",
            runtime.required("VECTOR_TEST_RESULTS_S3_BUCKET_NAME")?,
        ),
        ("ssh_cidr", cidr.into()),
    ] {
        result.extend(["-var".into(), format!("{key}={value}")]);
    }
    result.push(format!(
        "-var-file=../configurations/{}/terraform.tfvars",
        case.configuration
    ));
    Ok(result)
}

fn supports(bootstrap: &str, subject: &str) -> bool {
    bootstrap
        .lines()
        .filter_map(|line| line.trim().trim_start_matches("- ").split_once(':'))
        .any(|(key, value)| {
            matches!(key, "name" | "role")
                && value
                    .split('#')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .trim_matches(['\'', '"'])
                    == subject
        })
}

pub fn run(runtime: &Runtime, args: RunArgs) -> Result<()> {
    let case = case(runtime, &args.case)?;
    let bootstrap = fs::read_to_string(case.dir.join("ansible/bootstrap.yml"))?;
    let catalog = subjects(runtime)?;
    let requested = args.subject.or_else(|| runtime.get("VECTOR_TEST_SUBJECT"));
    let selected: Vec<_> = catalog
        .iter()
        .filter(|s| {
            requested.as_ref().is_none_or(|name| name == &s.slug) && supports(&bootstrap, &s.slug)
        })
        .collect();
    ensure!(
        !selected.is_empty(),
        "requested subject is not supported by {}",
        case.name
    );
    let version = args
        .vector_version
        .or_else(|| runtime.get("VECTOR_VERSION_TO_TEST"))
        .unwrap_or_else(|| "0.61.0".into());
    ensure!(
        !version.contains("latest"),
        "specify an exact Vector version; 'latest' cannot identify historical results"
    );
    let deb = args
        .vector_deb
        .or_else(|| {
            runtime
                .get("VECTOR_TEST_VECTOR_DEB_PATH")
                .map(PathBuf::from)
        })
        .map(|p| runtime.path(p))
        .transpose()?;
    if selected.iter().any(|s| s.slug == "vector") && version.starts_with("dev-") {
        check_file(deb.as_deref().context(
            "development versions require --vector-deb or VECTOR_TEST_VECTOR_DEB_PATH",
        )?)?;
    }
    let key = runtime.env_path("VECTOR_TEST_SSH_PRIVATE_KEY")?;
    let timestamp = runtime.get("VECTOR_TEST_TIMESTAMP").unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock precedes epoch")
            .as_secs()
            .to_string()
    });
    let _: u64 = timestamp
        .parse()
        .context("VECTOR_TEST_TIMESTAMP must be an epoch timestamp")?;
    let namespace = format!(
        "tag_TestName_{}:&tag_TestConfiguration_{}:&tag_TestUserID_{}",
        case.name, case.configuration, case.user
    );
    let common_vars = json!({
        "test_name":case.name, "test_configuration":case.configuration, "test_namespace":namespace,
        "test_timestamp":timestamp, "user_id":case.user,
        "results_s3_bucket_name":runtime.required("VECTOR_TEST_RESULTS_S3_BUCKET_NAME")?,
        "ansible_ssh_private_key_file":key, "vector_version":version,
    });
    let ansible = Invocation::new("ansible-playbook", &case.dir.join("ansible"))
        .env(
            "ANSIBLE_CONFIG",
            runtime.get("ANSIBLE_CONFIG").unwrap_or_else(|| {
                runtime
                    .root
                    .join("ansible/ansible.cfg")
                    .to_string_lossy()
                    .into_owned()
            }),
        )
        .args([
            "-u".into(),
            "ubuntu".into(),
            "--extra-vars".into(),
            format!("@../configurations/{}/ansible.yml", case.configuration),
            "--extra-vars".into(),
            common_vars.to_string(),
        ]);
    let mut ansible = if let Some(deb) = deb {
        ansible.args([
            "--extra-vars".into(),
            json!({"local_vector_deb_path":deb}).to_string(),
        ])
    } else {
        ansible
    };
    ansible
        .args
        .extend(runtime.extra_args("ANSIBLE_EXTRA_ARGS")?);
    if !args.run_only && !args.skip_terraform {
        // Resolve and validate all inputs before creating any resources.
        let cidr = ssh_cidr(runtime, args.case.ssh_cidr.as_deref())?;
        let vars = variables(runtime, &case, &cidr)?;
        terraform_init(runtime, &case)?;
        runtime.run(
            terraform(runtime, &case)
                .args(["apply", interactive(runtime), "-auto-approve"])
                .args(vars),
        )?;
    }
    if !args.run_only {
        runtime.run(ansible.clone().args(["bootstrap.yml"]))?;
    }
    if !args.bootstrap_only {
        for subject in selected {
            runtime.run(ansible.clone().args([
                "--extra-vars".into(),
                json!({"test_subject":subject.slug}).to_string(),
                "run.yml".into(),
            ]))?;
        }
    }
    Ok(())
}

pub fn teardown(runtime: &Runtime, args: TeardownArgs) -> Result<()> {
    let case = case(runtime, &args.case)?;
    let cidr = ssh_cidr(runtime, args.case.ssh_cidr.as_deref())?;
    let vars = variables(runtime, &case, &cidr)?;
    terraform_init(runtime, &case)?;
    let yes = args.yes || runtime.get("VECTOR_TEST_YES_PROMPTS").as_deref() == Some("true");
    let mut command = terraform(runtime, &case)
        .args(["destroy", if yes { "-input=false" } else { "-input=true" }]);
    if yes {
        command.args.push("-auto-approve".into());
    }
    runtime.run(command.args(vars))
}

pub fn build_amis(runtime: &Runtime) -> Result<()> {
    let versions = subjects(runtime)?
        .into_iter()
        .filter_map(|s| {
            s.preinstalled_version
                .map(|v| format!("preinstalled_{}_version={v}", s.slug))
        })
        .collect::<Vec<_>>()
        .join(" ");
    runtime.run(
        Invocation::new("packer", &runtime.root.join("packer"))
            .env("PREINSTALLED_TEST_SUBJECT_VERSIONS_ANSIBLE_VARS", versions)
            .args(["build", "ami.json"]),
    )
}

pub fn global_bootstrap(runtime: &Runtime) -> Result<()> {
    let dir = runtime.root.join("global");
    let bucket = runtime.required("VECTOR_TEST_RESULTS_S3_BUCKET_NAME")?;
    let account = runtime.required("VECTOR_TEST_HARNESS_AWS_ACCOUNT_ID")?;
    ensure!(
        account.len() == 12 && account.bytes().all(|b| b.is_ascii_digit()),
        "AWS account ID must have 12 digits"
    );
    let mut init = Invocation::new("terraform", &dir)
        .env("TF_IN_AUTOMATION", "true")
        .args(["init", interactive(runtime)]);
    if let Some(path) = runtime.get("TF_PLUGIN_DIR") {
        init = init.args([
            "-plugin-dir".into(),
            runtime.path(path)?.to_string_lossy().into_owned(),
        ]);
    }
    runtime.run(init)?;
    runtime.run(
        Invocation::new("terraform", &dir)
            .env("TF_IN_AUTOMATION", "true")
            .args([
                "apply".into(),
                interactive(runtime).into(),
                "-auto-approve".into(),
                "-var".into(),
                format!("results_s3_bucket_name={bucket}"),
                "-var".into(),
                format!("test_harness_aws_account_ids=[\"{account}\"]"),
            ]),
    )
}

fn directories(path: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = fs::read_dir(path)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    dirs.retain(|p| p.is_dir());
    dirs.sort();
    Ok(dirs)
}

pub fn meta(runtime: &Runtime, mode: MetaMode) -> Result<()> {
    match mode {
        MetaMode::TestSubjects => {
            for subject in subjects(runtime)? {
                println!("{}", subject.slug);
            }
        }
        MetaMode::TestCases => {
            for dir in directories(&runtime.root.join("cases"))? {
                if dir.join("terraform/main.tf").is_file() {
                    println!("{}", dir.file_name().unwrap().to_string_lossy());
                } else if dir.file_name().is_some_and(|name| name == "performance") {
                    for local in directories(&dir)? {
                        println!(
                            "performance/{}",
                            local.file_name().unwrap().to_string_lossy()
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn lint(runtime: &Runtime) -> Result<()> {
    runtime.run(Invocation::new("terraform", &runtime.root).args(["fmt", "-check", "-recursive"]))
}

pub fn ssh(runtime: &Runtime, args: Vec<String>) -> Result<()> {
    let mut command = Invocation::new("ssh", &runtime.root).args(["-o", "User=ubuntu"]);
    if let Some(key) = runtime.get("VECTOR_TEST_SSH_PRIVATE_KEY") {
        command = command.args([
            "-o".into(),
            format!("IdentityFile={}", runtime.path(key)?.display()),
            "-o".into(),
            "IdentitiesOnly=yes".into(),
        ]);
    }
    command.args.extend(args);
    if runtime.dry_run {
        return runtime.plan(&command);
    }
    let status = runtime.command(&command).status()?;
    std::process::exit(status_code(status));
}

pub fn doctor(runtime: &Runtime) -> Result<()> {
    let mut missing = vec![];
    for (tool, flag, required) in [
        ("cargo", "--version", false),
        ("aws", "--version", true),
        ("terraform", "version", true),
        ("ansible-playbook", "--version", true),
        ("packer", "version", false),
        ("ssh", "-V", true),
        ("perf", "--version", false),
        ("valgrind", "--version", false),
        ("inferno-collapse-perf", "--version", false),
        ("inferno-flamegraph", "--version", false),
    ] {
        let spec = Invocation::new(tool, &runtime.root).args([flag]);
        if runtime.dry_run {
            runtime.plan(&spec)?;
            continue;
        }
        match runtime.output(&spec) {
            Ok(output) if output.status.success() => println!("{tool}: available"),
            _ => {
                println!(
                    "{tool}: missing{}",
                    if required {
                        " (remote harness prerequisite)"
                    } else {
                        " (optional)"
                    }
                );
                if required {
                    missing.push(tool);
                }
            }
        }
    }
    ensure!(
        missing.is_empty(),
        "missing prerequisites: {}",
        missing.join(", ")
    );
    Ok(())
}

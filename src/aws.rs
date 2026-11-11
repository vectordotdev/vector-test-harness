use std::{
    fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};

use crate::{
    cli::BootstrapArgs,
    runtime::{identifier, Invocation, Runtime},
};

pub fn command(runtime: &Runtime, args: impl IntoIterator<Item = impl Into<String>>) -> Invocation {
    Invocation::new("aws", &runtime.root)
        .env("AWS_PAGER", "")
        .args(["--output", "json"])
        .args(args)
}

pub fn json(runtime: &Runtime, args: impl IntoIterator<Item = impl Into<String>>) -> Result<Value> {
    let text = runtime.capture(command(runtime, args))?;
    serde_json::from_str(&text).context("AWS returned invalid JSON")
}

fn exists(runtime: &Runtime, args: &[&str], absent_codes: &[&str]) -> Result<bool> {
    let spec = command(runtime, args.iter().copied());
    if runtime.dry_run {
        runtime.plan(&spec)?;
        return Ok(false);
    }
    let output = runtime.output(&spec)?;
    if output.status.success() {
        return Ok(true);
    }
    let error = String::from_utf8_lossy(&output.stderr);
    if absent_codes.iter().any(|code| error.contains(code)) {
        return Ok(false);
    }
    bail!(
        "AWS resource lookup failed (no resources created for this lookup): {}",
        error.trim()
    )
}

pub fn bootstrap(runtime: &Runtime, args: BootstrapArgs) -> Result<()> {
    let env_path = runtime.root.join(".env");
    ensure!(
        args.force || !env_path.exists(),
        ".env already exists; use --force to replace it"
    );
    let user = args
        .user_id
        .or_else(|| runtime.get("VECTOR_TEST_USER_ID"))
        .or_else(|| runtime.get("USER").map(|user| user.replace('.', "")))
        .context("supply --user-id")?;
    identifier(&user, "user ID")?;
    let key = runtime.path(
        args.ssh_key
            .unwrap_or_else(|| PathBuf::from("~/.ssh/vector_tests")),
    )?;
    let region = runtime
        .get("AWS_DEFAULT_REGION")
        .or_else(|| runtime.get("AWS_REGION"))
        .unwrap_or_else(|| "us-east-1".into());
    let account = if runtime.dry_run {
        runtime.plan(&command(runtime, ["sts", "get-caller-identity"]))?;
        args.account_id
            .context("an offline bootstrap plan requires --account-id")?
    } else {
        let identity = json(runtime, ["sts", "get-caller-identity"])?;
        let actual = identity["Account"]
            .as_str()
            .context("AWS did not return an account ID")?
            .to_owned();
        if let Some(expected) = args.account_id {
            ensure!(expected == actual, "AWS account differs from --account-id");
        }
        actual
    };
    ensure!(
        account.len() == 12 && account.bytes().all(|b| b.is_ascii_digit()),
        "AWS account ID must contain 12 digits"
    );
    let state = format!("vector-test-harness-state-{account}");
    let results = format!("vector-test-harness-results-{account}");
    for bucket in [&state, &results] {
        if !exists(
            runtime,
            &[
                "s3api",
                "head-bucket",
                "--bucket",
                bucket,
                "--region",
                &region,
            ],
            &["(404)", "(NoSuchBucket)"],
        )? {
            let mut spec = command(
                runtime,
                [
                    "s3api",
                    "create-bucket",
                    "--bucket",
                    bucket,
                    "--region",
                    &region,
                ],
            );
            if region != "us-east-1" {
                spec = spec.args([
                    "--create-bucket-configuration".into(),
                    format!("LocationConstraint={region}"),
                ]);
            }
            runtime.run(spec)?;
        }
    }
    if !exists(
        runtime,
        &[
            "dynamodb",
            "describe-table",
            "--table-name",
            "TerraformLocks",
            "--region",
            &region,
        ],
        &["(ResourceNotFoundException)"],
    )? {
        runtime.run(command(
            runtime,
            [
                "dynamodb",
                "create-table",
                "--table-name",
                "TerraformLocks",
                "--attribute-definitions",
                "AttributeName=LockID,AttributeType=S",
                "--key-schema",
                "AttributeName=LockID,KeyType=HASH",
                "--billing-mode",
                "PAY_PER_REQUEST",
                "--region",
                &region,
            ],
        ))?;
        runtime.run(command(
            runtime,
            [
                "dynamodb",
                "wait",
                "table-exists",
                "--table-name",
                "TerraformLocks",
                "--region",
                &region,
            ],
        ))?;
    }
    let role = "vector-packer-build";
    if !exists(
        runtime,
        &["iam", "get-role", "--role-name", role],
        &["(NoSuchEntity)"],
    )? {
        runtime.run(command(runtime, ["iam", "create-role", "--role-name", role, "--assume-role-policy-document", &json!({"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}).to_string()]))?;
    }
    runtime.run(command(
        runtime,
        [
            "iam",
            "attach-role-policy",
            "--role-name",
            role,
            "--policy-arn",
            "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore",
        ],
    ))?;
    if !exists(
        runtime,
        &[
            "iam",
            "get-instance-profile",
            "--instance-profile-name",
            role,
        ],
        &["(NoSuchEntity)"],
    )? {
        runtime.run(command(
            runtime,
            [
                "iam",
                "create-instance-profile",
                "--instance-profile-name",
                role,
            ],
        ))?;
        runtime.run(command(
            runtime,
            [
                "iam",
                "add-role-to-instance-profile",
                "--instance-profile-name",
                role,
                "--role-name",
                role,
            ],
        ))?;
    } else {
        let profile = json(
            runtime,
            [
                "iam",
                "get-instance-profile",
                "--instance-profile-name",
                role,
            ],
        )?;
        let roles = profile["InstanceProfile"]["Roles"]
            .as_array()
            .context("invalid IAM instance profile")?;
        ensure!(
            roles.is_empty() || roles.iter().any(|r| r["RoleName"] == role),
            "instance profile contains an unexpected role"
        );
        if roles.is_empty() {
            runtime.run(command(
                runtime,
                [
                    "iam",
                    "add-role-to-instance-profile",
                    "--instance-profile-name",
                    role,
                    "--role-name",
                    role,
                ],
            ))?;
        }
    }
    runtime.mkdir(key.parent().context("SSH key has no parent")?)?;
    if !key.exists() {
        runtime.run(Invocation::new("ssh-keygen", &runtime.root).args([
            "-t",
            "ed25519",
            "-f",
            &key.to_string_lossy(),
            "-N",
            "",
            "-C",
            "vector-test-harness",
        ]))?;
    }
    let public = PathBuf::from(format!("{}.pub", key.display()));
    if !public.exists() {
        let text = runtime.capture(Invocation::new("ssh-keygen", &runtime.root).args([
            "-y",
            "-f",
            &key.to_string_lossy(),
        ]))?;
        runtime.write(&public, text.as_bytes())?;
    }
    let content = format!("AWS_DEFAULT_REGION={region}\nVECTOR_TEST_SSH_PRIVATE_KEY={}\nVECTOR_TEST_SSH_PUBLIC_KEY={}\nVECTOR_TEST_USER_ID={user}\nVECTOR_TEST_STATE_S3_BUCKET_NAME={state}\nVECTOR_TEST_RESULTS_S3_BUCKET_NAME={results}\n", serde_json::to_string(&key.to_string_lossy())?, serde_json::to_string(&public.to_string_lossy())?);
    if runtime.dry_run {
        runtime.write(&env_path, &[])?;
    } else {
        use std::io::Write;
        let mut options = fs::OpenOptions::new();
        options.write(true);
        if args.force {
            options.create(true).truncate(true);
        } else {
            options.create_new(true);
        }
        options.open(&env_path)?.write_all(content.as_bytes())?;
        eprintln!("AWS prerequisites ready; wrote {}", env_path.display());
    }
    Ok(())
}

pub fn athena(runtime: &Runtime, sql: &str) -> Result<Option<Value>> {
    let database = runtime
        .get("VECTOR_TEST_ATHENA_DATABASE")
        .unwrap_or_else(|| "vector_tests".into());
    let output = runtime
        .get("VECTOR_TEST_ATHENA_OUTPUT")
        .unwrap_or_else(|| "s3://vector-test-athena-results".into());
    let start = command(
        runtime,
        [
            "athena",
            "start-query-execution",
            "--query-execution-context",
            &json!({"Database":database}).to_string(),
            "--query-string",
            sql,
            "--result-configuration",
            &json!({"OutputLocation":output}).to_string(),
        ],
    );
    if runtime.dry_run {
        runtime.plan(&start)?;
        return Ok(None);
    }
    let timeout: u64 = runtime
        .get("VECTOR_TEST_ATHENA_TIMEOUT_SECONDS")
        .unwrap_or_else(|| "300".into())
        .parse()
        .context("invalid Athena timeout")?;
    let response: Value = serde_json::from_str(&runtime.capture(start)?)?;
    let id = response["QueryExecutionId"]
        .as_str()
        .context("Athena returned no execution ID")?;
    let started = Instant::now();
    loop {
        let execution = json(
            runtime,
            ["athena", "get-query-execution", "--query-execution-id", id],
        )?;
        let status = &execution["QueryExecution"]["Status"];
        match status["State"].as_str() {
            Some("SUCCEEDED") => break,
            Some("FAILED" | "CANCELLED") => bail!(
                "Athena query {id} {}: {}",
                status["State"],
                status["StateChangeReason"]
                    .as_str()
                    .unwrap_or("no reason supplied")
            ),
            Some("QUEUED" | "RUNNING") => {}
            other => bail!("Athena query {id} returned unexpected state {other:?}"),
        }
        if started.elapsed() >= Duration::from_secs(timeout) {
            let _ = runtime.run(command(
                runtime,
                ["athena", "stop-query-execution", "--query-execution-id", id],
            ));
            bail!("Athena query {id} exceeded {timeout}s and was cancelled");
        }
        thread::sleep(Duration::from_millis(250));
    }
    let mut all: Option<Value> = None;
    let mut token: Option<String> = None;
    let mut seen = std::collections::BTreeSet::new();
    loop {
        let mut args = vec![
            "athena".into(),
            "get-query-results".into(),
            "--query-execution-id".into(),
            id.into(),
            "--no-paginate".into(),
        ];
        if let Some(token) = &token {
            args.extend(["--next-token".into(), token.clone()]);
        }
        let page = json(runtime, args)?;
        let rows = page["ResultSet"]["Rows"]
            .as_array()
            .context("Athena returned no rows")?;
        if let Some(ref mut all) = all {
            all["ResultSet"]["Rows"]
                .as_array_mut()
                .unwrap()
                .extend(rows.iter().cloned());
        } else {
            all = Some(page.clone());
        }
        token = page["NextToken"].as_str().map(str::to_owned);
        match &token {
            None => break,
            Some(token) => ensure!(
                seen.insert(token.clone()),
                "Athena returned a repeated pagination token"
            ),
        }
    }
    if let Some(ref mut all) = all {
        all.as_object_mut().unwrap().remove("NextToken");
    }
    Ok(all)
}

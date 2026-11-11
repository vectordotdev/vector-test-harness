use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;

pub static INTERRUPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unset_env: Vec<String>,
}

impl Invocation {
    pub fn new(program: &str, cwd: &Path) -> Self {
        Self {
            program: program.into(),
            args: vec![],
            cwd: cwd.into(),
            env: BTreeMap::new(),
            unset_env: vec![],
        }
    }
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }
    pub fn env(mut self, key: &str, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

pub struct Runtime {
    pub root: PathBuf,
    pub dry_run: bool,
    file_env: BTreeMap<String, String>,
}

impl Runtime {
    pub fn new(root: Option<&Path>, no_dotenv: bool, dry_run: bool) -> Result<Self> {
        let root = match root {
            Some(root) => root.canonicalize().context("invalid repository root")?,
            None => env::current_dir()?
                .ancestors()
                .find(|dir| dir.join("cases").is_dir() && dir.join("ansible").is_dir())
                .map(Path::to_owned)
                .context("cannot find repository; use --root")?,
        };
        ensure!(
            root.join("cases").is_dir() && root.join("ansible").is_dir(),
            "invalid harness root {}",
            root.display()
        );
        let mut file_env = BTreeMap::new();
        if !no_dotenv
            && env::var("DO_NOT_LOAD_DOTENV").as_deref() != Ok("true")
            && root.join(".env").exists()
        {
            for item in dotenvy::from_path_iter(root.join(".env"))? {
                let (key, value) =
                    item.context("invalid .env; use KEY=value entries, not shell commands")?;
                if env::var_os(&key).is_none() {
                    file_env.entry(key).or_insert(value);
                }
            }
        }
        Ok(Self {
            root,
            dry_run,
            file_env,
        })
    }

    pub fn get(&self, name: &str) -> Option<String> {
        self.file_env
            .get(name)
            .cloned()
            .or_else(|| env::var(name).ok())
            .filter(|s| !s.is_empty())
    }
    pub fn required(&self, name: &str) -> Result<String> {
        let value = self
            .get(name)
            .with_context(|| format!("set {name} in the environment or .env"))?;
        ensure!(
            !value.contains("<REPLACE_ME>"),
            "configure {name} before continuing"
        );
        Ok(value)
    }
    pub fn path(&self, value: impl AsRef<Path>) -> Result<PathBuf> {
        let value = value.as_ref();
        if let Ok(relative) = value.strip_prefix("~") {
            return Ok(PathBuf::from(self.required("HOME")?).join(relative));
        }
        Ok(if value.is_absolute() {
            value.into()
        } else {
            env::current_dir()?.join(value)
        })
    }
    pub fn env_path(&self, key: &str) -> Result<PathBuf> {
        self.path(self.required(key)?)
    }
    pub fn extra_args(&self, key: &str) -> Result<Vec<String>> {
        shell_words::split(&self.get(key).unwrap_or_default())
            .with_context(|| format!("invalid quoting in {key}"))
    }
    pub fn command(&self, spec: &Invocation) -> Command {
        let mut command = Command::new(&spec.program);
        command
            .current_dir(&spec.cwd)
            .args(&spec.args)
            .envs(&self.file_env)
            .envs(&spec.env);
        for key in &spec.unset_env {
            command.env_remove(key);
        }
        command
    }
    pub fn plan(&self, spec: &Invocation) -> Result<()> {
        println!("{}", serde_json::to_string(spec)?);
        Ok(())
    }
    pub fn run(&self, spec: Invocation) -> Result<()> {
        if self.dry_run {
            return self.plan(&spec);
        }
        let status = self
            .command(&spec)
            .status()
            .with_context(|| format!("could not run {}", spec.program))?;
        ensure!(status.success(), "{} failed with {status}", spec.program);
        Ok(())
    }
    pub fn output(&self, spec: &Invocation) -> Result<Output> {
        ensure!(!self.dry_run, "command output is unavailable in a dry run");
        self.command(spec)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("could not run {}", spec.program))
    }
    pub fn capture(&self, spec: Invocation) -> Result<String> {
        if self.dry_run {
            self.plan(&spec)?;
            return Ok(String::new());
        }
        let output = self.output(&spec)?;
        ensure!(
            output.status.success(),
            "{} failed with {}: {}",
            spec.program,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        String::from_utf8(output.stdout).context("command returned non-UTF-8 output")
    }
    pub fn write(&self, path: &Path, content: &[u8]) -> Result<()> {
        if self.dry_run {
            println!("{}", serde_json::json!({"action":"write", "path":path}));
        } else {
            fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
        }
        Ok(())
    }
    pub fn mkdir(&self, path: &Path) -> Result<()> {
        if self.dry_run {
            println!("{}", serde_json::json!({"action":"mkdir", "path":path}));
        } else {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }
    pub fn container_keys(&mut self) -> Result<()> {
        let generate = self.get("GENERATE_SSH_KEY").is_some();
        let injected = self.get("INJECT_SSH_KEY");
        ensure!(
            !(generate && injected.is_some()),
            "choose GENERATE_SSH_KEY or INJECT_SSH_KEY"
        );
        if !generate && injected.is_none() {
            return Ok(());
        }
        let key = self.path(
            self.get("VECTOR_TEST_SSH_PRIVATE_KEY")
                .unwrap_or_else(|| "~/.ssh/id_rsa".into()),
        )?;
        let public_key = PathBuf::from(format!("{}.pub", key.display()));
        let parent = key.parent().context("SSH key has no parent")?;
        self.mkdir(parent)?;
        #[cfg(unix)]
        if !self.dry_run {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }
        if !key.exists() {
            if generate {
                self.run(Invocation::new("ssh-keygen", &self.root).args([
                    "-b",
                    "4096",
                    "-t",
                    "rsa",
                    "-N",
                    "",
                    "-C",
                    "generated-in-docker",
                    "-f",
                    &key.to_string_lossy(),
                ]))?;
            } else if let Some(value) = injected {
                if self.dry_run {
                    self.write(&key, &[])?;
                } else {
                    use std::io::Write;
                    let mut options = fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.mode(0o600);
                    }
                    options
                        .open(&key)?
                        .write_all(value.replace("\\n", "\n").as_bytes())?;
                }
                let public = self.capture(Invocation::new("ssh-keygen", &self.root).args([
                    "-y",
                    "-f",
                    &key.to_string_lossy(),
                ]))?;
                self.write(&public_key, public.as_bytes())?;
            }
        }
        self.file_env.insert(
            "VECTOR_TEST_SSH_PRIVATE_KEY".into(),
            key.to_string_lossy().into(),
        );
        self.file_env.insert(
            "VECTOR_TEST_SSH_PUBLIC_KEY".into(),
            public_key.to_string_lossy().into(),
        );
        Ok(())
    }
}

pub fn identifier(value: &str, kind: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "{kind} must contain only letters, digits, and underscores"
    );
    Ok(())
}

pub fn status_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

pub fn check_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("file does not exist: {}", path.display());
    }
    Ok(())
}

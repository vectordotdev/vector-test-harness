<p align="center">
  <img src="./README/header.svg" alt="Vector Test Harness">
</p>

# Vector Test Harness

A Rust CLI for Vector correctness checks, performance benchmarks, and profiling.
`vth` runs local checks and workloads, orchestrates remote cases through Terraform
and Ansible, and analyzes recorded results.

[Setup](#setup) · [Commands](#usage) · [Cases](#cases) ·
[Local checks](#local-correctness-checks) · [Remote runs](#how-it-works) ·
[Results](#performance-data) · [Profiling](#standalone-performance-experiments)

> [!NOTE]
> The Vector configs target **0.61.0**. Local wrapped-JSON and SIGHUP checks have
> passed with that version on macOS arm64. The [2026-11-12 AWS baseline](results/2026-11-12/README.md)
> records Linux runs with the pinned comparison agents, including observed
> correctness failures. Historical case tables describe older versions and
> workloads. CPU and allocation profiling remain separate experiments.

## Setup

Build and install `vth` from this checkout with Rust **1.94+**:

```sh
cargo install --locked --path . --bin vth
vth --help
vth list
```

Run commands from this checkout, or pass `--root /path/to/vector-test-harness`.
The installed binary still needs the repository's configs and case files. For
local development without installing, use
`cargo run --locked --bin vth -- <command> ...`.

Install additional tools for the workflows you use:

| Workflow | Additional requirements |
| --- | --- |
| List cases or analyze saved results | None. |
| Local correctness checks | A Vector binary; macOS or Linux. |
| Standalone benchmarks | A Vector binary; Git and its build dependencies if using `vth build`. |
| Profiling and flamegraphs | Linux with `perf` or Valgrind; Inferno tools for flamegraphs; `file_gen` for continuous file input. |
| Remote cases | AWS CLI and credentials, Terraform 1.0+, Ansible with its AWS inventory dependencies, and SSH. See the [setup guide](docs/setup.md) for compatible Ansible/SDK versions. Automatic egress-IP discovery also uses `curl`. |
| Cloud results | AWS CLI and credentials with access to the results bucket and, for comparison/export, Athena. |
| AMI builds | Remote prerequisites plus Packer and its Amazon/Ansible plugins. |

`vth doctor` checks installed tools without contacting AWS; it reports missing
remote prerequisites even if you only intend to run locally.
See the [cloud setup guide](docs/setup.md) for account, inventory, AMI, and
container setup, and the [migration guide](docs/migration.md) for old entry points.

## Usage

Use `vth <command> --help` for flags. `--root`, `--no-dotenv`, and `--dry-run` are
global options. Dry runs validate inputs and print JSON command/filesystem plans
without running external tools or writing files; referenced local files must
still exist. Guest helpers execute directly and do not support dry runs.

| Command | Function |
| --- | --- |
| `vth list` | List remote cases and standalone workloads. |
| `vth meta <mode>` | Print `test-cases` or `test-subjects` for scripting. |
| `vth check <case>` | Run one of the two local Vector correctness checks. |
| `vth run` | Provision, bootstrap, and run a remote case; `test` is an alias. |
| `vth teardown` | Destroy resources for one case, configuration, and user. |
| `vth bootstrap-aws` | Create account prerequisites and write `.env`. |
| `vth build-amis` | Build the benchmark AMI with Packer. |
| `vth global-bootstrap` | Provision the legacy shared Athena/results infrastructure. |
| `vth results` | Summarize dstat CSVs from S3 or local files. |
| `vth compare` | Compare recorded remote measurements through Athena or a saved response. |
| `vth export` | Write website-compatible results JSON and optionally upload it. |
| `vth build` | Build an explicit revision in an isolated clone. |
| `vth bench <case>` | Time a finite stdin workload, optionally against a baseline binary. |
| `vth profile <case>` | Run `perf`, `perf stat`, Massif, or DHAT on a workload. |
| `vth flamegraph` | Generate flamegraphs and optional baseline/candidate differences. |
| `vth generate` | Run `file_gen` for the continuous file workload. |
| `vth ssh` | Connect to a remote machine using the configured SSH identity. |
| `vth doctor` | Report installed infrastructure and profiling tools. |
| `vth lint` | Check Terraform formatting. |
| `vth helper ...` | Run guest utilities; also available through `vth-guest`. |

## Cases

### Remote cases

These definitions run with `vth run -t <case> -c <configuration> -s vector`.
The configuration defaults to `default`; **`file_to_tcp_performance` requires
`-c single_file` or `-c multi_file`**. The two cases marked “also local” can also
run through `vth check` without cloud infrastructure.

| Case | Kind | What it exercises | Configurations |
| --- | --- | --- | --- |
| [disk_buffer_performance](cases/disk_buffer_performance/) | Performance | TCP forwarding with a disk buffer. | `default` |
| [disk_buffer_persistence_correctness](cases/disk_buffer_persistence_correctness/) | Correctness | Deliver buffered records after a downstream outage and subject restart. | `default` |
| [docker_partial_events_merging_correctness](cases/docker_partial_events_merging_correctness/) | Correctness | Merge a large Docker log record and verify its complete payload. | `default` |
| [file_rotate_create_correctness](cases/file_rotate_create_correctness/) | Correctness | Follow a renamed file and its replacement, including late writes to the old file. | `default` |
| [file_rotate_truncate_correctness](cases/file_rotate_truncate_correctness/) | Correctness | Follow copy-truncate rotation and late writes to the copied file. | `default` |
| [file_to_tcp_performance](cases/file_to_tcp_performance/) | Performance | Tail files and forward their contents over TCP. | `single_file`, `multi_file` |
| [file_truncate_correctness](cases/file_truncate_correctness/) | Correctness | Read new records after a file is truncated. | `default` |
| [lua_base_performance](cases/lua_base_performance/) | Performance | Add a field with a Lua hook between TCP input and output. | `default` |
| [real_world_1_performance](cases/real_world_1_performance/) | Performance | Route and normalize five syslog streams into HTTP sinks. | `default`, `high_concurrency`, `high_concurrency_with_pipelines`, `one_sink`, `without_lua`, `without_transforms`, `without_transforms_and_rate_limiting` |
| [regex_parsing_performance](cases/regex_parsing_performance/) | Performance | Extract fields from log messages with a regex and emit JSON. | `default`, `sandbox` |
| [sighup_correctness](cases/sighup_correctness/) | Correctness; also local | Reload config with SIGHUP and switch TCP destinations. | `default` |
| [tcp_to_blackhole_performance](cases/tcp_to_blackhole_performance/) | Performance | Receive TCP records and discard them. | `default` |
| [tcp_to_http_performance](cases/tcp_to_http_performance/) | Performance | Forward TCP input over HTTP. | `default`, `high_concurrency` |
| [tcp_to_tcp_performance](cases/tcp_to_tcp_performance/) | Performance | Forward TCP input to a TCP consumer. | `default`, `high_concurrency`, `16_cores` |
| [wrapped_json_correctness](cases/wrapped_json_correctness/) | Correctness; also local | Decode a JSON envelope and its nested `log` payload. | `default` |

Subject support varies by case. Omitting `-s` runs every subject supported by that
case's bootstrap playbook. The [subject catalog](config/subjects.json) records
preinstalled comparison versions; it does not represent current releases or
certify their installers. Real-world [variant details](cases/real_world_1_performance/README.md)
explain the retained `without_lua` alias and workload differences.

### Standalone workloads

These live under `cases/performance/`. Pass the short name to `bench` or `profile`,
for example `vth bench stdin_to_blackhole --vector-bin /path/to/vector`.
`vth list` prints them with a `performance/` prefix to identify their location.

| Case | Commands | What it exercises |
| --- | --- | --- |
| [stdin_to_blackhole](cases/performance/stdin_to_blackhole/) | `bench`, `profile` | Stream the finite shared gzip fixture through stdin into a blackhole sink. |
| [klog_remap_to_blackhole](cases/performance/klog_remap_to_blackhole/) | `bench`, `profile` | Compare one-node and two-node remapping of text/JSON klog records, plus a stdin baseline. |
| [file_to_blackhole](cases/performance/file_to_blackhole/) | `generate` + `profile` | Continuously tail generated files into a blackhole sink, exposing internal metrics. |

## Local correctness checks

```sh
vth check wrapped_json_correctness --vector-bin /path/to/vector
vth check sighup_correctness --vector-bin /path/to/vector --timeout-secs 30
```

The runner renders the existing case config with isolated loopback ports and a
per-run data directory. Wrapped JSON must produce exactly one event with the
unwrapped message and no `log` field. SIGHUP must deliver exactly one record to
each of two receivers, switching after Vector confirms the reload. Both checks
assert payloads and counts, wait for readiness/delivery/reload conditions, and
drain output during graceful shutdown. An unexpected Vector exit fails the check,
even with exit code zero. The default case budget is 30 seconds.

Artifacts remain under `.cache/runs/check-<case>-<id>/`, or a new directory selected
with `--output`. Existing directories are never overwritten. Each run saves
inputs, expected/received records, rendered configs, raw TCP output, Vector logs,
the command, and binary/config-source hashes. `result.json` records the outcome,
phase, differences, and cleanup. `complete` means execution reached verification
after shutdown; it does not mean the assertions passed. Timeouts and interrupted
or incomplete runs cannot pass.

Exit codes are `0` for success, `1` for incorrect output or an unexpected Vector
exit, and `2` for a timeout, interruption, or runner error after setup. Input
validation errors are reported before the run starts. Local checks currently
support only the two cases listed above.

## How it works

Remote cases provision AWS machines with Terraform, configure them with Ansible,
then run selected subjects sequentially. Producers, consumers, and profiling
steps are defined by each case's playbooks. These runs remain separate from the
local `check`, `bench`, and `profile` commands.

Set up credentials, buckets, SSH identity, and user ID following the
[cloud setup guide](docs/setup.md). `vth bootstrap-aws` can create account
prerequisites, or copy `.env.example` to `.env` for existing resources. It does
not provision the shared Athena dataset; `global-bootstrap` is a separate legacy
workflow with its own Terraform backend.

Explicit CLI options override supported environment settings, which override
`.env`. The file is parsed as data; `vth` never executes `.envrc` or shell code
from `.env`. Use `--no-dotenv` to disable file loading.

```sh
vth run -t tcp_to_tcp_performance -s vector -v 0.61.0 --dry-run
vth run -t tcp_to_tcp_performance -s vector -v 0.61.0
vth run -t file_to_tcp_performance -c single_file -s vector -v 0.61.0
vth run -t tcp_to_tcp_performance -s vector -v 0.61.0 --run-only
vth teardown -t tcp_to_tcp_performance --yes
```

`--bootstrap-only` stops after setup, `--run-only` uses configured machines, and
`--skip-terraform` skips provisioning while retaining Ansible bootstrap. Remote
runs default to Vector 0.61.0; use `-v <version>` to identify the artifact being
tested. `latest` is rejected. Use `--vector-deb /path/to/vector.deb -v dev-name`
for a custom package.

Terraform state and resources are scoped by case, configuration, and user ID.
Use the same `-t`, `-c`, and `-u` values for teardown. Omitting `--yes` retains
Terraform's interactive confirmation. `--ssh-cidr` or `VECTOR_TEST_SSH_CIDR`
overrides automatic egress-IP discovery. Dry runs need configuration values but
no AWS credentials.

Remote machines need an AMI containing the Rust `vth-guest` helper. Rebuild with
`vth build-amis` if the selected image predates the Rust migration. Docker log
fixtures likewise need an image containing the current helper.

## Performance data

The [2026-11-12 AWS baseline](results/2026-11-12/README.md) records all configured
remote cases and the three standalone workloads with Vector 0.61.0, including
versions, hardware, sample counts, and observed correctness failures.

`vth results` summarizes dstat measurements. `vth compare` queries recorded
measurements; it does not run either Vector binary. `vth export` writes the
website's `performance_tests` JSON and uploads it to
`s3://<results-bucket>/summaries/latest.json` unless `--no-upload` is supplied.

```sh
vth results -t tcp_to_tcp_performance
vth compare -t tcp_to_tcp_performance -s vector -v 0.60.0 -v 0.61.0
vth results -t tcp_to_tcp_performance --input /path/to/dstat.csv --json
vth compare --input /path/to/athena-response.json
vth export --input /path/to/athena-response.json --no-upload
```

The `--input` examples above work offline and use existing measurements. Dstat
reports retain positive-sample selection and warmup/tail trimming; see
[measurement compatibility](docs/migration.md#compatibility-and-deliberate-changes).
Historical S3 keys have no user ID partition, so `results --user-id` cannot filter
them and is rejected.

Athena defaults to database `vector_tests` and query output
`s3://vector-test-athena-results`. Override these with
`VECTOR_TEST_ATHENA_DATABASE` and `VECTOR_TEST_ATHENA_OUTPUT` for your account.
Failed queries and timeouts are errors; paginated query results are combined.

## Standalone performance experiments

Supply a binary or build an explicit revision. `vth build` uses an isolated clone
under `.cache/builds/`, requires its own Cargo `target`, and records the resolved
commit and build settings. The [AWS baseline](results/2026-11-12/README.md) records
Vector 0.61.0; establish fresh measurements for other configs and binaries.

```sh
vth build --revision <commit>
vth bench stdin_to_blackhole --vector-bin /path/to/vector --runs 10
vth bench klog_remap_to_blackhole --vector-bin /path/to/candidate --baseline-bin /path/to/baseline
vth profile klog_remap_to_blackhole --vector-bin /path/to/vector --tool perf
vth flamegraph --input /path/to/candidate.perf.data --baseline /path/to/baseline.perf.data
```

`bench` defaults to one warmup and ten measured repetitions per config/binary,
alternates baseline/candidate order, and writes `benchmark.json` under a unique
`.cache/runs/<case>-<id>/` directory or the selected `--output` directory. Artifacts
include individual timings, hashes, and the method
`wall-clock-with-streamed-input-v1`. These timings include process execution and
input streaming; keep them separate from historical hyperfine/dstat results.

For klog, all three configs under `configs/` run by default; `--config` selects
one. `--input` overrides a finite workload's plain-text or gzip fixture. The
continuously running file case uses `generate` and `profile` in separate terminals
instead of `bench`:

```sh
vth generate --filegen-bin /path/to/file_gen
vth profile file_to_blackhole --vector-bin /path/to/vector --tool perf
```

`generate` keeps existing logs unless `--reset-logs` is supplied. Profiling supports
`perf`, `stat`, `massif`, and `dhat`; flamegraphs use `perf` and Inferno. See the
[standalone guide](cases/performance/README.md) for details.

## Development

```sh
cargo fmt --all
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --bin vth -- lint
```

Use `cargo fmt --all -- --check` to check formatting without rewriting files.
While developing the CLI, `cargo run --locked --bin vth -- <command> ...` rebuilds
it before execution.

The Rust suite covers local correctness comparisons and process cleanup, command
construction, remote lifecycle failures, Athena pagination, CSV analysis, exports,
gzip input, and guest helpers. External infrastructure tools are replaced with
local Rust test doubles; the suite does not provision AWS resources or establish
benchmark results.

Add remote cases under `cases/<name>/` with `terraform/`, `ansible/`, and
`configurations/<variant>/`. Add standalone workloads under `cases/performance/`.
Local correctness support is explicitly selected in [the CLI](src/cli.rs) and
[runner](src/correctness.rs); creating a case directory alone does not add a
`vth check` case. Embedded Vector unit tests in configs run with Vector after
rendering any templates.

The [subject catalog](config/subjects.json), [metric definitions](config/metrics.json),
and [Athena queries](queries/) supply shared metadata. The dependency-free
`vth-guest` provides the timed socat loop, parallel argv execution, and Docker
long-message fixture. AMI and fixture builds compile it from the same Rust source.
The Dockerfile uses the internal `vth container` entrypoint to prepare optional
SSH identities and forward to the regular CLI.

## License

[MPL 2.0](LICENSE). Keep benchmark methods and modifications public so that results
can be reproduced and assessed.

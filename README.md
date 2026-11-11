<p align="center">
  <img src="./README/header.svg" alt="Vector Test Harness">
</p>

# Vector Test Harness

A Rust command-line harness for Vector end-to-end correctness tests, performance
benchmarks, and profiling experiments. `vth` owns orchestration and analysis;
Terraform and Ansible define the existing remote environments and workloads.
The optional `justfile` provides convenient commands for the same Rust CLI.

> [!NOTE]
> Case configurations target Vector 0.61.0. Cloud execution, other subject
> installers, and historical performance claims still require validation in the
> appropriate AWS account and on controlled Linux benchmark machines. Updated
> workloads require fresh performance baselines.

## Getting started

Install Rust 1.94+, Terraform 1.0+, Ansible 2.16+, and the AWS CLI. Packer is
needed for AMI builds; `just` is optional. See [the setup guide](docs/setup.md)
for AWS prerequisites and [the migration guide](docs/migration.md) for old commands.

```sh
cargo build --locked --bins
cargo run --locked --bin vth -- --help
just doctor
just list
```

All recipes delegate to Rust. Without `just`, use
`cargo run --locked --bin vth -- <command> ...`, or install the CLI with
`cargo install --locked --path . --bin vth`.

## Local correctness checks

Run the existing wrapped-JSON and SIGHUP cases with a supplied Vector binary on
macOS or Linux. These checks need Rust and Vector; they run entirely on loopback
without AWS, Terraform, Ansible, or Docker.

```sh
just check-case wrapped_json_correctness --vector-bin /path/to/vector
just check-case sighup_correctness --vector-bin /path/to/vector --timeout-secs 30
```

`vth check` renders the case's existing Vector config with isolated ports and a
per-run data directory. Wrapped JSON must produce exactly one event with the
unwrapped message and no `log` field. SIGHUP must deliver exactly one record to
each of two receivers, switching destinations after Vector confirms the reload.
Both checks validate payloads as well as counts, then drain output during
graceful shutdown. Readiness, delivery, and reload use bounded condition waits.
Both cases have been exercised with Vector 0.61.0 on macOS.

Artifacts are retained under `.cache/runs/check-<case>-<id>/`. Use `--output` to
select a new directory; an existing directory is never overwritten. Each run
saves its inputs, expected and received records, rendered configs, raw TCP output,
Vector logs, command, and binary/config hashes. `result.json` records the outcome,
phase, differences, and process cleanup. `complete` means the run reached output
verification after shutdown; an incomplete run cannot pass. Exit codes are `0`
for success, `1` for incorrect output or an unexpected Vector exit, and `2` for a
timeout, interruption, or runner error after setup. `--dry-run` previews the
command without starting Vector or creating artifacts.

## Remote cases

Copy `.env.example` to `.env` and configure the account, buckets, SSH identity,
and user ID. Existing `VECTOR_TEST_*` variables remain supported. Explicit CLI
options take precedence over environment variables; the environment takes
precedence over `.env`. The harness parses `.env` as data and never executes
`.envrc` or shell commands in configuration. `--no-dotenv` disables loading.

```sh
just run -t tcp_to_tcp_performance -s vector -v 0.61.0
just run -t tcp_to_tcp_performance -s vector --run-only
just results -t tcp_to_tcp_performance
just compare -t tcp_to_tcp_performance -s vector -v 0.60.0 -v 0.61.0
just teardown -t tcp_to_tcp_performance --yes
```

`run` provisions with Terraform, bootstraps with Ansible, then runs the selected
subjects sequentially. `--bootstrap-only`, `--run-only`, and `--skip-terraform`
retain their previous meanings. Use `--vector-deb /path/to/vector.deb -v dev-name`
for a custom package. Versions must identify the tested artifact; `latest` is
rejected.

Resources and state remain scoped by case, configuration, and user ID. Both
creation and teardown initialize the same backend key. `--ssh-cidr` (or
`VECTOR_TEST_SSH_CIDR`) overrides automatic egress-IP detection.

Inspect the commands without running external tools or writing files:

```sh
just run -t tcp_to_tcp_performance -s vector --dry-run
just teardown -t tcp_to_tcp_performance --yes --dry-run
```

Dry runs produce JSON lines containing argv arrays and planned filesystem actions.
They still validate configuration and case files. AWS credentials are unnecessary
for dry runs. No provisioning is performed by the harness's own test suite.

Remote cases continue to run in isolated AWS environments. Performance
measurements should use controlled Linux benchmark machines.

## Results

`compare` uses the existing Athena query. `export` preserves the website's
`performance_tests` structure and publishes to `summaries/latest.json` unless
`--no-upload` is supplied. Athena failures and timeouts produce errors; all
result pages are read.

Offline analysis is available too:

```sh
just results -t tcp_to_tcp_performance --input /path/to/dstat.csv --json
just compare --input /path/to/athena-response.json
just export-results --input /path/to/athena-response.json --no-upload
```

The dstat summary retains the old positive-sample selection and warmup/tail
trimming. This migration does not change historical measurement formulas or
S3 partitions. S3 summaries identify configurations separately. Historical keys
contain no user ID, so `results --user-id` reports an error rather than silently
ignoring that filter.

Athena defaults remain database `vector_tests` and query output
`s3://vector-test-athena-results`. Override these with
`VECTOR_TEST_ATHENA_DATABASE` and `VECTOR_TEST_ATHENA_OUTPUT`.

## Standalone performance experiments

The old per-case build/profiling scripts now share Rust commands. Supply a binary
or build an explicit revision in an isolated clone. The build checks that Cargo
uses that clone's own `target` and records the resolved commit and build settings.

```sh
just build --revision <commit> --no-default-features --features sources-stdin,sinks-blackhole
just bench stdin_to_blackhole --vector-bin /path/to/vector --runs 10
just bench klog_remap_to_blackhole --vector-bin /path/to/candidate --baseline-bin /path/to/baseline
just profile klog_remap_to_blackhole --vector-bin /path/to/vector --tool perf
just flamegraph --input /path/to/profile.perf.data --baseline /path/to/baseline.perf.data
```

`bench` records each repetition, alternates baseline/candidate order, and writes
`benchmark.json` with artifact hashes and explicit measurement-method metadata.
These timings have a new method identity and must not be pooled with historical
hyperfine or dstat results. The case configs target Vector 0.61.0; updated remap
and real-world workloads require fresh baselines. Use an explicit `--config`
when evaluating another workload definition.

`profile` supports `perf`, `stat`, `massif`, and `dhat`. Profiling tools remain
external dependencies and generally require Linux. For the continuously running
file workload, use `generate --filegen-bin /path/to/file_gen` and
`profile file_to_blackhole --vector-bin /path/to/vector`. Generated logs are only
removed when `generate --reset-logs` is explicitly requested.

See [standalone workflows](cases/performance/README.md) for more examples.

## Development

```sh
just fmt
just check
just test
just lint
```

The Rust tests exercise command construction, remote lifecycle failures, Athena
pagination, CSV analysis, website exports, gzip input, and guest helpers using
local Rust test doubles. `lint` checks Terraform formatting separately.

Add remote cases under `cases/<name>/` with the existing `terraform/`, `ansible/`,
and `configurations/<variant>/` structure. The shared subject/version catalog is
in `config/subjects.json`, metric presentation metadata is in
`config/metrics.json`, and historical analysis queries are in `queries/`.

The small dependency-free `vth-guest` binary implements the VM's timed socat loop,
parallel argv execution, and Docker long-message fixture. AMI and fixture image
builds compile it from the same Rust source. **Rebuild the AMI before running
migrated cloud cases**, since older images only contain the shell helper.

## License

[MPL 2.0](LICENSE). Keep benchmark methods and modifications public so that results
can be reproduced and assessed.

<p align="center">
  <img src="./README/header.svg" alt="Vector Test Harness">
</p>

# Vector Test Harness

A Rust CLI for local correctness checks, Vector workload timing and profiling,
and remote AWS cases using Terraform and Ansible. Vector configs target **0.61.0**.

## Comparisons

Historical cross-product performance and correctness scorecards were withdrawn:
the cases did not consistently establish equivalent work and verified delivery.
See the [retained results](results/2026-11-12/README.md).

A comparison must verify the same logical task from input A to receiver B:
matching application records, counts and payloads under a shared delivery and
durability contract. Native protocols, batching and encodings may differ.
See the [comparison requirements](docs/comparisons.md).

## Setup

Install with Rust **1.94+**:

```sh
cargo install --locked --path . --bin vth
vth --help
vth list
```

Run from this checkout or pass `--root /path/to/vector-test-harness`.
For development, use `cargo run --locked --bin vth -- <command> ...`.
Local checks and workloads require a Vector binary. Profiling requires Linux
with perf or Valgrind; flamegraphs also require Inferno.

## Usage

Use `vth <command> --help` for flags. Global `--dry-run` prints plans without
running tools or writing files; referenced input files must exist. Guest helpers
do not support dry runs.

| Command | Function |
| --- | --- |
| `vth list` | List remote cases and standalone workloads. |
| `vth meta <mode>` | Print case or subject metadata. |
| `vth check <case>` | Run a local Vector correctness check. |
| `vth run` | Provision and run a remote case; `test` is an alias. |
| `vth teardown` | Destroy a case's cloud resources. |
| `vth bootstrap-aws` | Create account prerequisites and write `.env`. |
| `vth build-amis` | Build the AMI with Packer. |
| `vth global-bootstrap` | Provision legacy Athena/results infrastructure. |
| `vth results` | Summarize dstat CSVs from S3 or local files. |
| `vth compare` | Inspect archived metrics through Athena or saved input. |
| `vth export --no-upload` | Export archived metrics locally; publication is disabled. |
| `vth build` | Build an explicit revision in an isolated clone. |
| `vth bench <case>` | Time a finite workload, optionally against a baseline binary. |
| `vth profile <case>` | Run perf, perf stat, Massif or DHAT. |
| `vth flamegraph` | Generate flamegraphs and optional baseline differences. |
| `vth generate` | Run file_gen for continuous file input. |
| `vth ssh` | Connect to a remote machine. |
| `vth doctor` | Report installed infrastructure and profiling tools. |
| `vth lint` | Check Terraform formatting. |
| `vth helper ...` | Run guest utilities; also available as `vth-guest`. |

## Cases

| Case | Commands | Purpose |
| --- | --- | --- |
| [wrapped_json_correctness](cases/wrapped_json_correctness/) | `check`, `run` | Decode one nested JSON input into exactly one expected application record. |
| [sighup_correctness](cases/sighup_correctness/) | `check` | Verify Vector switches receivers after reload, with correct payloads and counts. |
| [docker_partial_events_merging_correctness](cases/docker_partial_events_merging_correctness/) | `run` | Verify one complete Docker input produces one complete output. |
| [stdin_to_blackhole](cases/performance/stdin_to_blackhole/) | `bench`, `profile` | Stream a finite gzip fixture through Vector. |
| [klog_remap_to_blackhole](cases/performance/klog_remap_to_blackhole/) | `bench`, `profile` | Compare equivalent one-node and two-node remapping. |
| [file_to_blackhole](cases/performance/file_to_blackhole/) | `generate`, `profile` | Profile a continuous file workload; no capacity result is published. |

`check` is Vector-only. Remote wrapped JSON supports Filebeat, Fluent Bit,
Fluentd, Logstash and Vector; the Docker case is Vector-only. Both remote cases
use the `default` configuration. [subjects.json](config/subjects.json) records versions.

## Local correctness checks

```sh
vth check wrapped_json_correctness --vector-bin /path/to/vector
vth check sighup_correctness --vector-bin /path/to/vector
```

Checks verify payloads and counts after shutdown. Artifacts and `result.json`
are saved under `.cache/runs/check-<case>-<id>/`, or a new `--output` directory.
Exit codes: `0` passed; `1` incorrect output, unexpected exit or setup error;
`2` timeout, interruption or runner error after setup.

## Remote runs

Install the AWS CLI, Terraform 1.0+, Ansible, SSH and curl. AMI builds also need
Packer and the [AWS Session Manager plugin](https://developer.hashicorp.com/packer/integrations/hashicorp/amazon/latest/components/builder/ebs#session-manager-plugin).
Ansible needs
[amazon.aws](https://github.com/ansible-collections/amazon.aws#requirements) and
compatible Python boto3/botocore dependencies. Configure AWS credentials, then:

```sh
vth bootstrap-aws --user-id <user>
packer plugins install github.com/hashicorp/amazon
packer plugins install github.com/hashicorp/ansible
vth build-amis
vth run -t wrapped_json_correctness -s vector -v 0.61.0 --dry-run
vth run -t wrapped_json_correctness -s vector -v 0.61.0
vth teardown -t wrapped_json_correctness --yes
```

For existing resources, copy `.env.example` to `.env` instead of bootstrapping.
CLI options override environment settings, which override `.env`. Set `--ssh-cidr`
if automatic egress-IP discovery is unsuitable. The AMI must include `vth-guest`.

Use the same case, configuration (`-c`) and user (`-u`) for teardown.
For retired cases, use a checkout containing their original Terraform files.

## Standalone workloads

Supply a binary or build an explicit revision:

```sh
vth build --revision <commit>
vth bench stdin_to_blackhole --vector-bin /path/to/vector --runs 10
vth bench klog_remap_to_blackhole --vector-bin /path/to/candidate --baseline-bin /path/to/baseline
vth profile klog_remap_to_blackhole --vector-bin /path/to/vector --tool perf
vth flamegraph --input /path/to/candidate.perf.data --baseline /path/to/baseline.perf.data
```

`bench` defaults to one warmup and ten measured runs per configuration/binary.
It saves individual timings and hashes in `.cache/runs/<case>-<id>/benchmark.json`.
Elapsed time includes process execution and input streaming. Verify full-fixture
output equivalence before publishing a comparison. See the [workload guide](cases/performance/README.md).

## Archived results

Archived host metrics do not establish delivered throughput or product rankings.
Website publication is disabled; `export` requires `--no-upload`.

```sh
vth results -t archived_case --input /path/to/dstat.csv --json
vth compare --input /path/to/athena-response.json
vth export --input /path/to/athena-response.json --no-upload
```

For cloud queries, set `VECTOR_TEST_ATHENA_DATABASE` and
`VECTOR_TEST_ATHENA_OUTPUT`; legacy infrastructure uses `vth global-bootstrap`.

## Development

```sh
cargo fmt --all
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --bin vth -- lint
```

The Rust suite uses local test doubles; it does not provision AWS or validate
benchmark results. Add remote cases under `cases/<name>/` and standalone workloads
under `cases/performance/`. Local checks require explicit runner support.

## License

[MPL 2.0](LICENSE).

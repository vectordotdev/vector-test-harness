<p align="center">
  <img src="./README/header.svg" alt="Vector Test Harness">
</p>

# Vector Test Harness

A Rust CLI for local correctness checks, finite workloads, profiling and remote
AWS cases using Terraform and Ansible. Vector configs target **0.61.0**.

## Comparisons

Historical cross-product performance and correctness scorecards were withdrawn:
the cases did not consistently establish equivalent work and verified delivery.
See the [retained results](results/2026-11-12/README.md).

A comparison must verify the same logical task from input A to receiver B:
matching application records, counts and payloads under a shared delivery and
durability contract. Native protocols, batching and encodings may differ.
See the [comparison requirements](docs/comparisons.md).

## Vector-only finite-batch results

Measured on 2026-11-13. Input sizes include line delimiters. One AWS
`c5.4xlarge` ran Ubuntu 22.04.5 with SMT disabled. Vector used two workers on
two CPUs; the harness used two different CPUs. Affinity did not reserve CPUs
exclusively.

| Workload | Vector | Measured records | Input size | Runs | Median completion | Observed min–max |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| TCP → TCP | 0.61.0 | 1,000,000 | 187 MB | 5 | 1.148 s | 1.148–1.196 s |
| TCP → HTTP | 0.61.0 | 1,000,000 | 187 MB | 5 | 1.619 s | 1.611–1.622 s |
| File → TCP | 0.61.0 | 1,000,000 | 187 MB | 5 | 3.583 s | 3.572–3.753 s |
| TCP → regex parsing → TCP | 0.61.0 | 1,000,000 | 187 MB | 5 | 4.749 s | 4.735–4.768 s |

Timing covers input publication through verification of the last expected
record. Startup, canary, warmup and preparation of the 187.2 MB verification
reference are excluded. All 20 Vector runs verified every record with zero
anomalies, followed by clean shutdown and receiver drain. These are loopback,
memory-queue runs with sink acknowledgements disabled; file input was freshly
written and filesystem caches were not evicted. They establish finite
completion time, not sustained capacity or durable delivery.

[Individual timings, verification outcomes and provenance](results/2026-11-13/finite-batches.json)
include the matching source revision, config hashes and 20 direct-path
diagnostics. Direct durations are not subtracted. See the
[finite workload contract](#finite-workload-contract) for measurement details.

## Setup

Install with Rust **1.94+**:

```sh
cargo install --locked --path . --bin vth
vth --help
vth list
```

Run from this checkout or pass `--root /path/to/vector-test-harness`.
For development, use `cargo run --locked --bin vth -- <command> ...`.
Local checks and workloads require a supplied product binary. Profiling requires
Linux with perf or Valgrind; flamegraphs also require Inferno.

## Usage

Use `vth <command> --help` for flags. Global `--dry-run` prints plans without
running tools or writing files; referenced input files must exist. Guest helpers
do not support dry runs.

| Command | Function |
| --- | --- |
| `vth list` | List remote cases and standalone workloads. |
| `vth meta <mode>` | Print case or subject metadata. |
| `vth check <case>` | Run a local correctness or delivery check. |
| `vth matrix` | Run the current cases across supplied product binaries and retain every outcome. |
| `vth calibrate <tcp\|parsed-json\|http>` | Measure the direct finite producer-to-verifier path. |
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
| [sighup_correctness](cases/sighup_correctness/) | `check` | Verify an output switch after reload, with the same main PID and exact payloads/counts. |
| [docker_partial_events_merging_correctness](cases/docker_partial_events_merging_correctness/) | `check`, `run` | Reassemble a real container's fragments into exactly one intact record. |
| [file_to_tcp_performance](cases/file_to_tcp_performance/) | `check` | Verify every file record at a TCP receiver. |
| [tcp_to_tcp_performance](cases/tcp_to_tcp_performance/) | `check` | Verify forwarding, including duplicates, corruption and losses. |
| [regex_parsing_performance](cases/regex_parsing_performance/) | `check` | Verify every parsed field and output record. |
| [tcp_to_http_performance](cases/tcp_to_http_performance/) | `check` | Verify records inside complete HTTP batches. |
| [tcp_to_blackhole_performance](cases/tcp_to_blackhole_performance/) | `check` | Check the terminal processed count; no payload/delivery proof. |
| [stdin_to_blackhole](cases/performance/stdin_to_blackhole/) | `check`, `bench`, `profile` | Feed the original finite gzip fixture; `check` requires an exact terminal count. |
| [klog_remap_to_blackhole](cases/performance/klog_remap_to_blackhole/) | `check`, `bench`, `profile` | Verify multiline/JSON semantics and full-fixture output; retain Vector's one-node/two-node variants. |
| [file_to_blackhole](cases/performance/file_to_blackhole/) | `check`, `generate`, `profile` | Profile the original continuous file workload; no delivery or capacity claim. |

Local adapters cover Vector, Fluent Bit, Telegraf, Filebeat, Fluentd, Logstash,
Splunk HF and Splunk UF. Not every product supports every contract; the matrix
keeps unsupported cases, missing environments, unfinished adapters and failures
visible. Remote wrapped JSON supports five products; the legacy remote Docker
case is Vector-only. [subjects.json](config/subjects.json) records remote versions;
local reports record the supplied binary's actual version and hash.

## Local checks

```sh
vth check wrapped_json_correctness --vector-bin /path/to/vector
vth check sighup_correctness --vector-bin /path/to/vector
vth check file_to_tcp_performance --vector-bin /path/to/vector
vth check tcp_to_tcp_performance --vector-bin /path/to/vector --seed 7
vth check tcp_to_tcp_performance --fluent-bit-bin /path/to/fluent-bit --seed 7
vth check tcp_to_tcp_performance --subject telegraf --subject-bin /path/to/telegraf --seed 7
```

`vth matrix --runtimes runtimes.json --output results` runs cases sequentially
and updates `results/matrix.json` after each one. Supply binary paths and optional
scoped environment variables:

```json
{"subjects":{"vector":{"binary":"/path/to/vector"},"telegraf":{"binary":"/path/to/telegraf"}}}
```

Use `--subjects vector,telegraf` or `--cases tcp_to_tcp_performance` to select a
subset. Missing runtimes remain explicit. Matrix budgets default to 90 seconds
per case and 300 seconds for klog; `--timeout-secs` overrides either default.
Individual `check` budgets default to 30 seconds, or 300 seconds for klog.
Docker checks require a local Linux
daemon and `VTH_DOCKER_IMAGE` set to a prepared immutable image ID/digest. Continuous
file checks require Linux `perf`, a pinned `VTH_FILEGEN_BIN`, and port 9000 free.
Complete Splunk's native first-run setup, including terms acceptance and an
administrator account; the harness neither accepts licenses nor provisions credentials.

The same verifier checks application records across native text, JSON, Forward,
Beats and HTTP Bulk envelopes. Bulk endpoints emulate acknowledgement only;
there is no Elasticsearch indexing or durability proof. Count-only stdin/discard
checks and process profiles never receive delivery timings. Klog checks use a
small interleaved continuation canary and the full original fixture, capturing
output to files for exact payload checks rather than timing discard. Filebeat's
stdin and klog profiles use native CEL and buffer the finite input through EOF;
this does not establish streaming capacity. A failed health, shutdown or
verification check invalidates any candidate timing. Matrix execution is
sequential, with no exclusive host reservation or headroom calibration. Its
finite completion times are diagnostics, not a product ranking.

### Finite workload contract

Receiver-backed workloads measure completion of known input from A to B. They
use deterministic Apache-like records with sequential IDs. Payloads are unique
to each run by default; use the same `--seed` to reuse identical input across
subjects and direct baselines.
Continuous-load regression testing is outside this work.

- Verify one canary, then 1,000 warmup records, before releasing the measured
  fixture: 10,000 records by default, configurable with `--events`.
- Verify every expected application payload or parsed field, with no missing,
  duplicate, unexpected, malformed or corrupted records. Output order is not
  asserted.
- Time from immediately before measured input is published until the final
  expected record is fully verified. Fixture preparation, startup and warmup
  are excluded; input publication and receiver verification are included.
  Expected record data is prepared before timing; its memory size is recorded.
- Accept the duration only after clean shutdown, complete receiver drain and
  final error checks. A failure clears the duration.

These are normal-operation runs with memory queues; crash recovery and durable
delivery are not established. Where the protocol requires an application
acknowledgement, the receiver sends it after validation. TCP acknowledgements
alone do not prove delivery, and HTTP/Bulk success does not imply durable storage.
Logical input bytes include line delimiters;
receiver byte counters cover all phases and count encoded application payload,
excluding HTTP headers. File inputs are freshly written and the filesystem
cache is not evicted. Blackhole verifies only its processed count and has no
receiver completion time.

`vth calibrate tcp` measures the direct producer-to-verifier path without Vector;
`parsed-json` and `http` cover plain parsed-JSON and HTTP-message paths. Native
Beats, Forward and Bulk paths need separate headroom validation. Use the same
`--events` for checks and baselines. `--producer-chunk-bytes` and TCP-only
`--receiver-reads-per-pass` support IO batching sensitivity checks. These are
diagnostics; baseline durations are not subtracted from Vector durations.
On Linux, `--subject-cpus` (`--vector-cpus` remains an alias) places the subject
on a CPU list independently of a `taskset`-placed harness; reports record
observed placement and host metadata.
Affinity does not reserve CPUs exclusively.
Check producer and receiver headroom for each workload and execution environment.
Finite completion timings do not establish sustained capacity or product rankings.

Artifacts and `result.json` are saved under `.cache/runs/check-<case>-<id>/`,
or a new `--output` directory.
For `check`, exit codes are `0` passed, `1` a failed check or preflight error,
and `2` timeout, interruption, unavailable environment or runner error.
The matrix also returns nonzero when pending or missing cases remain.

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

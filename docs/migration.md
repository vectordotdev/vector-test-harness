# Script migration

Harness orchestration and analysis now live in the Rust `vth` CLI. Infrastructure
remains declarative Terraform and Ansible; AWS, SSH, Packer, Cargo, perf, Valgrind,
and Inferno remain external tools.

Use an installed `vth`, or `cargo run --locked --bin vth -- ...` while developing
the CLI.

| Previous entry point | Rust replacement |
| --- | --- |
| `bin/test` | `vth run` (`test` is also an alias) |
| `bin/teardown` | `vth teardown` |
| `bin/build-amis` | `vth build-amis` |
| `bin/global-bootstrap` | `vth global-bootstrap` |
| `scripts/bootstrap-aws.sh` | `vth bootstrap-aws` |
| `bin/compare` | `vth compare` |
| `bin/results` | `vth results` |
| `bin/export` | `vth export` |
| `bin/meta test-cases` | `vth list` or `vth meta test-cases` |
| `bin/meta test-subjects` | `vth meta test-subjects` |
| `bin/ssh` | `vth ssh` |
| `bin/lint` | `vth lint` |
| Per-case Vector cloning and builds | `vth build --revision ...` |
| Per-case hyperfine / perf-stat timing | `vth bench` / `vth profile --tool stat` |
| Per-case perf / Massif / DHAT scripts | `vth profile --tool perf\|massif\|dhat` |
| Per-case Inferno pipelines | `vth flamegraph` |
| `filegen.sh` | `vth build --repository https://github.com/blt/file_gen.git --revision ... --binary file_gen --no-default-features`, then `vth generate --filegen-bin ...` |
| Docker entrypoint | `vth container` (configured by the Dockerfile) |
| `packer/bootstrap.sh` | AMI playbook's declarative apt pre-task |
| `socat_loop` | `vth-guest socat-loop` |
| `parallel_commands` | `vth-guest parallel COMMAND ARGS ::: COMMAND ARGS` |
| Ruby `longlog` fixture | `vth-guest longlog` |

The Bash library and Ruby reporting utilities are removed. Their behavior is
implemented in the Rust modules, with shared data in `config/` and SQL in
`queries/`. No forwarding shell scripts are required.

## Compatibility and deliberate changes

- Existing remote case names, configurations, subject versions, Terraform state
  keys, Ansible host filters, result partitions, and website export fields remain.
- Common short flags and `VECTOR_TEST_*` settings remain. `ANSIBLE_EXTRA_ARGS`
  accepts shell-style quoting but is parsed into argv without shell evaluation.
- `.env` accepts `KEY=value` and `export KEY=value`. Use literal values and quoted
  paths. Executable `.envrc` files are handled by your shell/direnv, never by `vth`.
- `teardown` initializes the backend itself and prompts unless `--yes` is set.
- Unsupported subjects and configurations fail before provisioning. AWS lookup
  errors do not count as missing resources, and failed Athena queries cannot be
  printed as successful results.
- `results --user-id` used to be accepted without filtering. It now fails with an
  explanation because historical S3 keys cannot establish the owning user.
- Historical dstat aggregation and Athena SQL retain their existing formulas,
  including the old positive-only sample selection and lexical latest-version
  selection. Changing measurement methodology is separate work.
- Standalone builds require an explicit revision or supplied binary. Historical
  hardcoded SHAs and workstation paths are not defaults. Builds use isolated
  clones under `.cache/builds/` and preserve other Vector checkouts.
- Native standalone timing results have method `wall-clock-with-streamed-input-v1`
  and a separate JSON artifact. Profiling is invoked separately.
- Case configurations now target Vector 0.61.0. Removed transforms and conditions
  have been migrated to current APIs, real-world variants deploy YAML, and file
  sources use explicit checkpoint/read-position options. Remap and real-world
  workload updates require new baselines; historical results remain historical.
- Real-world variants now share one template and common normalized output. The
  old `without_lua` name remains an alias, `one_sink` retains all five streams,
  and concurrency variants use the baseline hardware. Klog parses once per
  record; wrapped JSON uses source decoding; Docker correctness checks the full
  raw merged record. The regex case now emits its parsed fields as JSON. These
  workload and output changes require fresh baselines.
- Rebuild AMIs and Docker fixtures so they include `vth-guest`. The guest source is
  dependency-free and compiled natively for each environment.

Cloud provisioning, AMI builds, Docker images, real Vector workloads, and Linux
profilers need environment-specific validation; the local Rust integration suite
uses test doubles and does not make performance or cloud compatibility claims.

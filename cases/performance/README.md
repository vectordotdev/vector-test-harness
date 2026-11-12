# Standalone performance workflows

These workloads share the Rust harness's build, timing, and profiling commands.
Run comparable measurements on controlled Linux machines. The configs target
Vector 0.61. The remap variants handle both plain klog and structured JSON input,
and share a focused check for plain and structured records.

The two-node and single-node remap variants parse each record once and perform
identical timestamp handling. Text records pass through reduction once; JSON
records pass directly to the sink. Container-envelope parsing lives in the
separate correctness cases. The [2026-11-12 AWS baseline](../../results/2026-11-12/README.md)
records measurements for the current definitions with Vector 0.61.0.

```sh
vth build --revision <commit>
vth bench stdin_to_blackhole --vector-bin /path/to/vector --runs 10
vth bench klog_remap_to_blackhole --vector-bin /path/to/candidate --baseline-bin /path/to/baseline
vth profile klog_remap_to_blackhole --vector-bin /path/to/vector --tool perf
vth profile klog_remap_to_blackhole --vector-bin /path/to/vector --tool dhat
vth flamegraph --input /path/to/candidate.perf.data --baseline /path/to/baseline.perf.data
```

For remap, the harness runs every config under `configs/` unless `--config` is
provided. Stdin cases stream their gzip fixtures without a shell or `zcat`.
`stdin_to_blackhole` shares `file_to_blackhole/baked.log.gz` by default.

Each timing repetition is recorded. Baseline/candidate order alternates and
warmup is explicit (`--warmup`, default 1). Results go into a unique
`.cache/runs/<case>-<timestamp>/benchmark.json`, or a selected `--output` directory.
These measurements use a new explicit method identity and are separate from
historical hyperfine and dstat measurements.

For continuous file input, run file_gen and Vector in separate terminals:

```sh
vth generate --filegen-bin /path/to/file_gen --reset-logs
vth profile file_to_blackhole --vector-bin /path/to/vector --tool perf
```

`generate` retains existing logs unless `--reset-logs` is supplied. `bench` requires
a finite stdin workload. `perf`, Valgrind, and Inferno remain external tools.
Use `--dry-run` to inspect command and artifact paths without running them.

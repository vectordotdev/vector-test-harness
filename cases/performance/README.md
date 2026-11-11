# Standalone performance workflows

These archived workloads share the Rust harness's build, timing, and profiling
commands. Run comparable measurements on controlled Linux machines. The configs
retain their original syntax and behavior, so supply a compatible Vector binary
or an explicit updated `--config`; modernizing them is separate work.

```sh
just build --revision <commit>
just bench stdin_to_blackhole --vector-bin /path/to/vector --runs 10
just bench klog_remap_to_blackhole --vector-bin /path/to/candidate --baseline-bin /path/to/baseline
just profile klog_remap_to_blackhole --vector-bin /path/to/vector --tool perf
just profile klog_remap_to_blackhole --vector-bin /path/to/vector --tool dhat
just flamegraph --input /path/to/candidate.perf.data --baseline /path/to/baseline.perf.data
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
just generate --filegen-bin /path/to/file_gen --reset-logs
just profile file_to_blackhole --vector-bin /path/to/vector --tool perf
```

`generate` retains existing logs unless `--reset-logs` is supplied. `bench` requires
a finite stdin workload. `perf`, Valgrind, and Inferno remain external tools.
Use `--dry-run` to inspect command and artifact paths without running them.

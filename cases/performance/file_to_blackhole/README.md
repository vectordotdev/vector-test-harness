# File-to-blackhole profiling

Continuously tail `logs/*.log` into a blackhole sink. The config also exports
Vector's internal metrics on port 9001. This workload runs until interrupted;
use `profile` with generated files instead of the finite-stdin `bench` command.

Run these commands in separate terminals from the repository root, with an
installed `vth` and supplied `file_gen` and Vector binaries:

```sh
vth generate --filegen-bin /path/to/file_gen
vth profile file_to_blackhole --vector-bin /path/to/vector --tool perf
```

Logs and Vector checkpoints are retained across runs. `generate --reset-logs`
removes existing regular `*.log` files before starting the generator; it does
not reset Vector's `data/` directory. Keep the input and checkpoint state
consistent when comparing profiles.

The Vector binary needs file and internal-metrics sources, a blackhole sink,
and a Prometheus exporter. To build a revision, use `vth build --revision <commit>`.
See the [standalone guide](../README.md) for profiling tools and artifact details.

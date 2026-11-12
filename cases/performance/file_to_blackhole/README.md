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

The generator used by the current baseline can be built from its preserved
revision in Lading:

```sh
vth build --repository https://github.com/DataDog/lading.git \
  --revision 9e13b40919109e1737ba5388ef9eeb07d0230608 \
  --binary file_gen --no-default-features
```

Logs and Vector checkpoints are retained across runs. `generate --reset-logs`
removes existing regular `*.log` files before starting the generator; it does
not reset Vector's `data/` directory. Keep the input and checkpoint state
consistent when comparing profiles.

The Vector binary needs file and internal-metrics sources, a blackhole sink,
and a Prometheus exporter. To build a revision, use `vth build --revision <commit>`.
See the [standalone guide](../README.md) for profiling tools and artifact details.

## AWS observation — 2026-11-12

Vector 0.61.0 on an AWS `c5.large` (2 vCPUs, 4 GiB RAM), Ubuntu 22.04.5,
with the checked-in config and generator settings. The run began with empty
logs and checkpoints, generated 480 files, warmed up for 15 seconds, and
observed Prometheus counter changes over the next 60 seconds.

| Measurement | Observed rate |
| --- | --- |
| Raw file bytes received | 33.56 MiB/s |
| File events received | 69,658 events/s |
| Events received by the blackhole | 69,646 events/s |

Rates use the exporter's 60-second timestamp interval. This bounded observation
measures the configured generator load; it does not establish maximum throughput.
No CPU or allocation profile was collected.
[Counter deltas, generator revision, and config hashes](../../../results/2026-11-12/standalone/file_to_blackhole.json)
record the measurement setup.

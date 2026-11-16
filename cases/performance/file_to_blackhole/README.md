# File-to-blackhole profiling

`vth check file_to_blackhole --subject <product> --subject-bin /path/to/binary`
performs a bounded Linux process profile with fresh inputs and state. It requires
`perf`, a pinned `VTH_FILEGEN_BIN` and port 9000 free, verifies that all 480 generator
files grow, and retains a ten-second perf-stat sample (twenty seconds for Splunk's
minimum ten-second counter cadence). Passing establishes native
process/profiler health, not identities, complete delivery or capacity. Native
discard-counter samples remain diagnostic.

The older Vector `generate`/`profile` workflow below runs until interrupted and
retains its existing input and checkpoint state.

Continuously tail `logs/*.log` into a blackhole sink. The config also exports
Vector's internal metrics on port 9001. This workload runs until interrupted;
use `profile` with generated files instead of the finite-stdin `bench` command.

Run these commands in separate terminals from the repository root, with an
installed `vth` and supplied `file_gen` and Vector binaries:

```sh
vth generate --filegen-bin /path/to/file_gen
vth profile file_to_blackhole --vector-bin /path/to/vector --tool perf
```

The generator used during the profiling campaign can be built from its preserved
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

This is a developer profiling workload. Its configured generator rate and
internal counter samples do not establish maximum throughput or equivalent
end-to-end delivery, so no numerical capacity result is published.

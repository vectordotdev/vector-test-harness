# Klog checks and Vector benchmarking

The baked million-line fixture contains plain klog and structured JSON records.
Each record is decoded once into `log`, classified as text or JSON, and given the
structured record's numeric `ts` timestamp when present. Source-provided host
metadata is retained. The payload is not duplicated into top-level fields.

`remap-multihop.yaml` performs parsing and timestamp handling in two VRL nodes;
`remap-nohop.yaml` performs the same work in one node. Both route JSON directly to
the blackhole and pass text through the same multiline reducer. Klog headers
start new text records; continuation lines are joined with newlines.

`vth check klog_remap_to_blackhole --subject <product> --subject-bin /path/to/binary`
runs an interleaved JSON/continuation canary and the full fixture, capturing native
output to files for exact multiset, host/source and nanosecond timestamp checks.
Vector runs both variants; other products use one native logical pipeline.
This proves output semantics with file capture, not discard throughput.

The check defaults to 300 seconds, including native 30-second idle flushes where
required. Explicit `--timeout-secs` values take precedence. Filebeat's CEL profile
buffers the finite input through EOF; its result is no streaming-capacity claim.

The fixture contains no Docker/container envelopes; those have dedicated cases.

Run `vth bench klog_remap_to_blackhole --vector-bin /path/to/vector`. Use
`--config` to select one variant. The untransformed stdin reference was removed
because it performs different work.

## Historical Vector process timings — 2026-11-12

Vector 0.61.0 on an AWS `c5.large` (2 vCPUs, 4 GiB RAM), Ubuntu 22.04.5.
Each config used the checked-in fixture, one warmup, and ten measured runs.

| Config | Mean elapsed | Sample standard deviation |
| --- | --- | --- |
| `remap-multihop.yaml` | 10.6829 s | 0.0886 s |
| `remap-nohop.yaml` | 10.0367 s | 0.0584 s |

The `wall-clock-with-streamed-input-v1` measurement includes process execution
and input streaming. [Individual timings and hashes](../../../results/2026-11-12/standalone/klog_remap_to_blackhole.json)
identify the binary, fixture, configs, and harness revision.

Elapsed time includes reduction, drain and shutdown. It is not an isolated VRL
execution cost. Full-fixture output equivalence is checked separately before
these two configurations are compared.

# Klog remapping performance

The baked million-line fixture contains plain klog and structured JSON records.
Each record is decoded once into `log`, classified as text or JSON, and given the
structured record's numeric `ts` timestamp when present. Source-provided host
metadata is retained. The payload is not duplicated into top-level fields.

`remap-multihop.yaml` performs parsing and timestamp handling in two VRL nodes;
`remap-nohop.yaml` performs the same work in one node. Both route JSON directly to
the blackhole and pass text through the same multiline reducer. Klog headers
start new text records; continuation lines are joined with newlines.

The fixture does not contain Docker/container envelopes. That parsing belongs in
the dedicated wrapped-JSON and Docker cases rather than unused stages here.

Run `vth bench klog_remap_to_blackhole --vector-bin /path/to/vector`. Use
`--config` to select one variant. The untransformed stdin reference was removed
because it performs different work.

## Vector-only elapsed-time measurements — 2026-11-12

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

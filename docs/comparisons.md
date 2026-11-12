# Comparison requirements and retired cases

A published comparison must establish equivalent input, completed work and output.
Successful commands, valid CSV arithmetic and network counters do not establish
that contract. A case is removed when the available evidence cannot establish it.

Before a performance comparison is introduced, it must demonstrate:

- Identical fixtures and a defined count or identity for input records.
- Equivalent parsing, transformations and application output, checked at the receiver.
- Matching delivery, acknowledgement, persistence and shutdown guarantees.
- A metric tied to verified application payload or events; encoding and compression
  cannot turn wire-byte rates into a processing-throughput ranking.
- A measurement window with established readiness and input available throughout it,
  followed by output drain and checks for missing or duplicate records.
- Recorded versions, hardware, configurations, repetitions and measurement method.

Correctness comparisons may use different transport adapters after decoding, but
must apply the same logical input, output-count and payload assertions. Packet or
newline-frame counts alone cannot establish data loss or duplication.

## Removed remote cases

The November AWS campaign exposed these gaps. Runnable definitions, numerical
performance results and historical scorecards for these cases have been removed.
Private raw artifacts are retained for investigation, not publication as comparisons.

| Case | Reason for removal |
| --- | --- |
| `disk_buffer_performance` | Durability and acknowledgement policies differ across products. Host traffic does not establish equivalent persistence work. |
| `file_to_tcp_performance` | Finite files can be consumed during warmup, and disk reads, compressed wire bytes and delivered payload bytes are different metrics. End-to-end records were not verified. |
| `lua_base_performance` | The remote capture records host traffic without verifying delivered records or the added field. It does not establish completed Lua work. |
| `real_world_1_performance` | Variants perform different work and the remote capture does not verify delivery or normalized output across all five streams. |
| `regex_parsing_performance` | Parser patterns, error handling and output fields differ; output equivalence and successfully parsed event counts were not verified. |
| `tcp_to_blackhole_performance` | Received network bytes do not verify that each product decoded and processed the same records before discarding them. |
| `tcp_to_http_performance` | Encoding, batching and HTTP request semantics differ. Network counters were not tied to verified equivalent payload delivery. |
| `tcp_to_tcp_performance` | Raw forwarding, event construction, encodings and receiver protocols differ. Host traffic alone cannot establish comparable event throughput. |
| `disk_buffer_persistence_correctness` | Shutdown, acknowledgements and queue semantics differ, and a positive record count does not verify which input records survived. |
| `file_rotate_create_correctness` | The assertion counts frames without verifying all three unique payloads; product-specific framing can change the outcome. |
| `file_rotate_truncate_correctness` | The fixture writes to a copied archive and rewards a particular file-identity policy. Frame counts cannot establish equivalent no-loss/no-duplicate behavior. |
| `file_truncate_correctness` | A last-frame substring and fixed sleeps can confuse framing or startup timing with truncation behavior. |
| `sighup_correctness` | A fixed one-second delay does not establish comparable reload completion across products. The Vector-only local check remains supported. |

## Retained checks

- `wrapped_json_correctness`: one identical nested JSON input must produce exactly
  one decoded application record with `message: Hello world`. All five supported
  subjects use that contract; adapters decode their transport before verification.
  This is a functional check and makes no throughput comparison.
- `docker_partial_events_merging_correctness`: a Vector-only diagnostic. The fixture
  proves that Docker emitted one complete record, then checks the received count
  and entire decoded input payload. Its observed failure is retained without assigning a
  cause or drawing conclusions about other products.
- Local Vector wrapped-JSON and SIGHUP checks verify counts and payloads. The SIGHUP
  check waits for Vector's reload acknowledgement, rather than using a comparative
  fixed-delay threshold.

Standalone timing and profiling commands remain development tools. Published
one-node/two-node klog timings require full-fixture output equivalence, and the
untransformed stdin reference has been removed from that comparison. Continuous
file generation remains a profiling workload with no published capacity result.

Website publication through `vth export` is disabled. `--no-upload` preserves
local inspection of archived metric JSON. Future publication requires cases that
meet the contract above; restoring an old scorecard is insufficient.

# Retained checks and Vector-only timings — 2026-11-12

The AWS campaign captured 71 remote performance measurements and 39 correctness
outcomes. The [comparability audit](../../docs/comparisons.md) subsequently
rejected all remote performance comparisons and 33 correctness outcomes for
publication. Their runnable cases, numerical tables and historical scorecards
have been removed. Private raw artifacts remain available for investigation.
Execution success and correct CSV arithmetic did not establish equivalent work.

This report retains six narrowly defined remote functional outcomes and two
Vector-only timing workloads. It publishes no cross-product throughput ranking.
[Environment and pinned versions](environment.json) describe the measured image.

## Nested JSON decoding

Each subject received the same single nested JSON record. The shared contract is
exactly one decoded application record with `message: Hello world`; additional
product metadata is outside that contract. Beats/JSON adapters decode transport
before checking the application output. File output is parsed as JSON.

| Subject | Received application records | Expected message | Result |
| --- | --- | --- | --- |
| Filebeat 8.14.3 | 1 | `Hello world` | Passed |
| Fluent Bit 3.2.7 | 1 | `Hello world` | Passed |
| Fluentd (td-agent 4.5.2-1) | 1 | `Hello world` | Passed |
| Logstash 8.17.3 | 1 | `Hello world` | Passed |
| Vector 0.61.0 | 1 | `Hello world` | Passed |

Captured receiver summaries and executed file-output assertions were rechecked
against that common contract. The TCP handlers now enforce the same exact count.
[Machine-readable outcomes and evidence hashes](correctness.json) retain the
original run timestamps, config/source hashes and verification basis. This check
establishes this decoding behavior, not general product reliability or speed.

## Docker partial-event diagnostic

This is Vector-only. A fresh reproduction verified that Docker emitted one
complete input record, including its final field. The receiver obtained two
identical complete records. The output-count assertion failed and remains
unchanged. This observation does not assign the cause to Vector or the fixture.
[Recorded observation](docker-observation.json)

## Finite Vector-only timings

These workloads ran on the same AWS `c5.large` (2 vCPUs, 4 GiB RAM), Ubuntu
22.04.5, with the same Vector 0.61.0 binary. Each configuration used one warmup
and ten measured repetitions. Elapsed time includes process startup, gzip/input
streaming, reduction, output drain and shutdown.

The standalone stdin workload took **1.3272 s** on average, with sample standard
deviation **0.0087 s**. It is not a baseline for the remap comparison.

The equivalent klog configurations measured:

| Configuration | Mean elapsed | Sample standard deviation |
| --- | --- | --- |
| Two remap nodes | 10.6829 s | 0.0886 s |
| One remap node | 10.0367 s | 0.0584 s |

The stdin workload is a separate workload, not a baseline for the remap pair.
The untransformed reference inside the klog case was removed because it performs
different work. These elapsed times do not isolate VRL execution cost.

Full-fixture verification replayed all **1,000,000 input records** with JSON file
output replacing blackhole. Stdin reconstructed the full uncompressed input
byte-for-byte. Both remap variants delivered exactly 1,000,000 events with
identical normalized payload multisets. Only source-generated `timestamp` and
`timestamp_end` values without an input timestamp were excluded; input-derived
timestamps and all application fields were compared. The verification ran
separately on macOS arm64 with Vector 0.61.0; the timings above were measured on
Linux x86_64. Original verification captures remain private.

- [Stdin timings and fixture/config hashes](standalone/stdin_to_blackhole.json)
- [Remap timings and fixture/config hashes](standalone/klog_remap_to_blackhole.json)
- [Full-fixture stdin delivery verification](verification/stdin-delivery.json)
- [Full-fixture remap equivalence verification](verification/klog-equivalence.json)

The continuous file generator remains a development profiling workload. Its
configured load and sampled counters do not establish maximum capacity, so its
numerical observation is not published here.

## Publication boundary

Retired cases are absent from the runnable catalog. Their README notices explain
the specific validity gap, and the previous scorecards are removed. Archived
metric analysis remains available without winner markers. `vth export` requires
`--no-upload`; website publication is disabled until comparable cases are
established. Future results must satisfy the [comparison requirements](../../docs/comparisons.md).

# AWS baseline — 2026-11-12

The configured remote cases and standalone workloads were exercised on AWS in
`us-east-1` with Vector 0.61.0. Comparison agents use the repository’s pinned
versions. [Environment and versions](environment.json) record
the image, tool versions, binary hashes, and sampling policy. The `fluentd`
version in remote results identifies the td-agent package; it contains Fluentd
1.16.3 and the HTTP output plugin 1.3.5.

Remote performance figures use one measured run per subject/configuration,
following each case's checked-in warmup and 60-second workload interval. The
dstat capture also includes orchestration overhead; its actual sample counts
and epoch spans are retained in the JSON. The tables report host-wide counters. Network rates include wire encoding and compression;
file-to-TCP outgoing traffic rates cannot rank logical payload throughput.
Disk-buffer configurations also use different synchronization policies: Logstash
checkpoints each event, while Vector synchronizes its disk buffer every 500ms.

The Athena summaries use the verified dstat 0.8.0 column order, skip its six
metadata/header lines and initial since-boot sample, and aggregate the remaining
samples with the repository's comparison query. The detailed JSON also retains
legacy socket/TCP fields that sum sampled state gauges across the capture. Disk and network fields already
contain per-second rates, so their reported average uses the mean of those samples. This differs from `vth results`,
which retains historical positive-sample selection and trimming. Diagnostic runs
and failed setup attempts are excluded from the performance aggregates. These
performance cases measure traffic and host load; their successful execution does
not assert complete payload delivery. Vector reported regex parse errors near
producer termination in the regex case; those observations are retained in its
run logs.

Completed 26 remote configurations: 71 performance and 39 correctness subject/configuration pairs (24 passed; 15 failed). The three standalone workloads are recorded below. Raw CSVs and detailed run manifests are retained in the private results storage; the JSON here records the selected aggregates and input/config hashes.

Observed correctness failures:

- `disk_buffer_persistence_correctness` / splunk_heavy_forwarder: Initial delivery passed, but no test records were recovered after restarting with ten records sent during the outage. Internal logs were excluded; the recovery assertion failed.
- `disk_buffer_persistence_correctness` / splunk_universal_forwarder: Initial delivery passed, but no test records were recovered after restarting with ten records sent during the outage. Built-in wildcard log routes were excluded; the recovery assertion failed.
- `docker_partial_events_merging_correctness` / vector: One complete container input record produced two identical complete records; input-count assertion passed in a fresh reproduction. Exact output-count assertion retained.
- `file_rotate_create_correctness` / splunk_heavy_forwarder: Expected three records after rename; received four. Internal logs were excluded, and both original and rotated paths were monitored.
- `file_rotate_create_correctness` / splunk_universal_forwarder: Expected three newline-framed records after rename; received eight, with an empty final frame. Internal log monitors were excluded, and both paths were monitored.
- `file_rotate_truncate_correctness` / filebeat: Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject.
- `file_rotate_truncate_correctness` / fluentbit: Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject.
- `file_rotate_truncate_correctness` / fluentd: Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject.
- `file_rotate_truncate_correctness` / logstash: Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject.
- `file_rotate_truncate_correctness` / splunk_heavy_forwarder: Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject.
- `file_rotate_truncate_correctness` / splunk_universal_forwarder: Expected three records without replaying copied content; received 8 newline-framed records. Both active and copied paths were monitored and reset before this subject.
- `file_truncate_correctness` / splunk_universal_forwarder: The final newline-framed record was empty, so the existing check for Hello world as the last message failed. Internal log monitors were excluded; this does not establish whether the marker appeared earlier in the stream.
- `sighup_correctness` / logstash: The second TCP connection was refused one second after SIGHUP while Logstash was reloading its pipeline; destination assertions were not reached.
- `sighup_correctness` / splunk_heavy_forwarder: After SIGHUP, both fixture records went to the original receiver (2 records versus 0 at the new destination); expected one at each.
- `sighup_correctness` / splunk_universal_forwarder: After SIGHUP, the original receiver counted four newline frames, ending empty, and the new receiver counted zero; expected one record at each. Internal log monitors were excluded.

## Fixture and analysis updates

The current runs use actual Beats decoding for Filebeat, framed TCP output for
Fluent Bit and Logstash correctness checks, and explicit read/rotation settings
for file fixtures. The SIGHUP topology creates its two required receivers, and
file truncation waits for initial Logstash delivery before altering the input.
Splunk forwards only fixture input to the test receiver; its internal records
use a disabled output group. Fluentd HTTP performance uses
buffered bulk requests after the earlier per-event connection setup exhausted
local source ports. Interrupted and superseded measurements are excluded.

## Remote performance

### `disk_buffer_performance/default`

1 × c5d.xlarge consumer, 5 × c5.large producer, 1 × c5.large subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Fluent Bit | 3.2.7 | 1 | 48.97 | 56.78 | 227.65 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 37.49 | 50.25 | 382.60 |
| Logstash | 8.17.3 | 1 | 0.04 | 31.80 | 955.39 |
| Splunk HF | 9.2.2-d76edf6f0a15 | 1 | 6.06 | 87.00 | 780.34 |
| Splunk UF | 9.2.2-d76edf6f0a15 | 1 | 452.74 | 44.62 | 314.66 |
| Vector | 0.61.0 | 1 | 19.80 | 93.50 | 358.13 |

[All host metrics](performance/disk_buffer_performance/default.json)

### `file_to_tcp_performance/multi_file`

1 × c5.xlarge consumer, 1 × c5.large subject. One run per subject.

| Subject | Version | Runs | Host network TX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Filebeat | 8.14.3 | 1 | 1.32 | 40.10 | 318.97 |
| Fluent Bit | 3.2.7 | 1 | 31.61 | 43.66 | 292.42 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 38.06 | 66.83 | 416.96 |
| Logstash | 8.17.3 | 1 | 1.25 | 91.00 | 1320.11 |
| Splunk HF | 9.2.2-d76edf6f0a15 | 1 | 15.73 | 89.45 | 723.98 |
| Splunk UF | 9.2.2-d76edf6f0a15 | 1 | 23.75 | 7.35 | 317.31 |
| Vector | 0.61.0 | 1 | 24.38 | 79.08 | 295.48 |

[All host metrics](performance/file_to_tcp_performance/multi_file.json)

### `file_to_tcp_performance/single_file`

1 × c5.xlarge consumer, 1 × c5.large subject. One run per subject.

| Subject | Version | Runs | Host network TX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Filebeat | 8.14.3 | 1 | 1.20 | 39.09 | 296.23 |
| Fluent Bit | 3.2.7 | 1 | 32.40 | 45.50 | 264.87 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 40.52 | 68.50 | 400.82 |
| Logstash | 8.17.3 | 1 | 1.10 | 93.03 | 1318.45 |
| Splunk HF | 9.2.2-d76edf6f0a15 | 1 | 17.22 | 89.45 | 747.54 |
| Splunk UF | 9.2.2-d76edf6f0a15 | 1 | 0.00 | 10.55 | 340.82 |
| Vector | 0.61.0 | 1 | 23.39 | 74.48 | 312.25 |

[All host metrics](performance/file_to_tcp_performance/single_file.json)

### `lua_base_performance/default`

1 × c5d.xlarge consumer, 5 × c5.large producer, 1 × c5.large subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Vector | 0.61.0 | 1 | 5.70 | 83.67 | 319.01 |

[All host metrics](performance/lua_base_performance/default.json)

### `real_world_1_performance/default`

1 × c5d.2xlarge consumer, 5 × c5.large producer, 1 × c5.xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Vector | 0.61.0 | 1 | 29.31 | 97.25 | 671.67 |

[All host metrics](performance/real_world_1_performance/default.json)

### `real_world_1_performance/high_concurrency`

1 × c5d.2xlarge consumer, 5 × c5.large producer, 1 × c5.xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Vector | 0.61.0 | 1 | 29.76 | 97.24 | 666.71 |

[All host metrics](performance/real_world_1_performance/high_concurrency.json)

### `real_world_1_performance/high_concurrency_with_pipelines`

1 × c5d.2xlarge consumer, 5 × c5.large producer, 1 × c5.xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Vector | 0.61.0 | 1 | 46.23 | 97.00 | 748.87 |

[All host metrics](performance/real_world_1_performance/high_concurrency_with_pipelines.json)

### `real_world_1_performance/one_sink`

1 × c5d.2xlarge consumer, 5 × c5.large producer, 1 × c5.xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Vector | 0.61.0 | 1 | 30.62 | 97.74 | 464.95 |

[All host metrics](performance/real_world_1_performance/one_sink.json)

### `real_world_1_performance/without_lua`

1 × c5d.2xlarge consumer, 5 × c5.large producer, 1 × c5.xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Vector | 0.61.0 | 1 | 29.54 | 96.99 | 662.53 |

[All host metrics](performance/real_world_1_performance/without_lua.json)

### `real_world_1_performance/without_transforms`

1 × c5d.2xlarge consumer, 5 × c5.large producer, 1 × c5.xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Vector | 0.61.0 | 1 | 34.54 | 95.49 | 559.50 |

[All host metrics](performance/real_world_1_performance/without_transforms.json)

### `real_world_1_performance/without_transforms_and_rate_limiting`

1 × c5d.2xlarge consumer, 5 × c5.large producer, 1 × c5.xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Vector | 0.61.0 | 1 | 34.95 | 95.50 | 558.48 |

[All host metrics](performance/real_world_1_performance/without_transforms_and_rate_limiting.json)

### `regex_parsing_performance/default`

1 × c5d.xlarge consumer, 5 × c5.large producer, 1 × c5.large subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Fluent Bit | 3.2.7 | 1 | 18.62 | 91.50 | 277.36 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 7.35 | 51.01 | 457.11 |
| Logstash | 8.17.3 | 1 | 3.02 | 97.00 | 1395.39 |
| Splunk HF | 9.2.2-d76edf6f0a15 | 1 | 4.69 | 89.00 | 745.53 |
| Vector | 0.61.0 | 1 | 16.41 | 98.52 | 342.44 |

[All host metrics](performance/regex_parsing_performance/default.json)

### `regex_parsing_performance/sandbox`

1 × m5.large consumer, 3 × m5.large producer, 1 × m5.large subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Fluent Bit | 3.2.7 | 1 | 17.05 | 87.50 | 300.85 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 7.10 | 51.01 | 514.35 |
| Logstash | 8.17.3 | 1 | 2.89 | 96.52 | 1339.48 |
| Splunk HF | 9.2.2-d76edf6f0a15 | 1 | 4.41 | 90.00 | 726.75 |
| Vector | 0.61.0 | 1 | 14.79 | 97.50 | 362.95 |

[All host metrics](performance/regex_parsing_performance/sandbox.json)

### `tcp_to_blackhole_performance/default`

10 × c5.large producer, 1 × c5.large subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Fluent Bit | 3.2.7 | 1 | 169.59 | 45.60 | 499.00 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 62.84 | 49.75 | 402.62 |
| Logstash | 8.17.3 | 1 | 21.45 | 96.48 | 1364.93 |
| Vector | 0.61.0 | 1 | 78.12 | 97.99 | 320.89 |

[All host metrics](performance/tcp_to_blackhole_performance/default.json)

### `tcp_to_http_performance/default`

1 × c5d.xlarge consumer, 5 × c5.large producer, 1 × c5.large subject. One run per subject.

| Subject | Version | Runs | Host network TX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Fluent Bit | 3.2.7 | 1 | 54.28 | 97.99 | 345.84 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 31.50 | 50.50 | 460.49 |
| Logstash | 8.17.3 | 1 | 2.98 | 88.38 | 1427.20 |
| Vector | 0.61.0 | 1 | 98.82 | 96.52 | 350.09 |

[All host metrics](performance/tcp_to_http_performance/default.json)

### `tcp_to_http_performance/high_concurrency`

1 × c5d.9xlarge consumer, 20 × c5.large producer, 1 × c5.9xlarge subject. One run per subject.

| Subject | Version | Runs | Host network TX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Fluent Bit | 3.2.7 | 1 | 115.95 | 6.85 | 862.80 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 36.94 | 2.75 | 724.52 |
| Logstash | 8.17.3 | 1 | 34.82 | 34.91 | 2179.07 |
| Vector | 0.61.0 | 1 | 630.13 | 42.51 | 1328.54 |

[All host metrics](performance/tcp_to_http_performance/high_concurrency.json)

### `tcp_to_tcp_performance/16_cores`

1 × c5d.4xlarge consumer, 5 × c5.large producer, 1 × c5d.4xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Filebeat | 8.14.3 | 1 | 3.92 | 5.46 | 459.30 |
| Fluent Bit | 3.2.7 | 1 | 187.05 | 18.14 | 5153.45 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 41.01 | 6.09 | 694.50 |
| Logstash | 8.17.3 | 1 | 16.24 | 18.13 | 1905.46 |
| Splunk HF | 9.2.2-d76edf6f0a15 | 1 | 11.23 | 17.47 | 859.77 |
| Splunk UF | 9.2.2-d76edf6f0a15 | 1 | 499.80 | 2.24 | 470.86 |
| Vector | 0.61.0 | 1 | 133.00 | 20.00 | 668.85 |

[All host metrics](performance/tcp_to_tcp_performance/16_cores.json)

### `tcp_to_tcp_performance/default`

1 × c5d.xlarge consumer, 5 × c5.large producer, 1 × c5.large subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Filebeat | 8.14.3 | 1 | 3.63 | 64.32 | 278.60 |
| Fluent Bit | 3.2.7 | 1 | 50.97 | 97.51 | 975.31 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 23.07 | 50.76 | 504.00 |
| Logstash | 8.17.3 | 1 | 7.90 | 97.50 | 1365.95 |
| Splunk HF | 9.2.2-d76edf6f0a15 | 1 | 4.87 | 89.50 | 731.58 |
| Splunk UF | 9.2.2-d76edf6f0a15 | 1 | 439.24 | 43.88 | 337.07 |
| Vector | 0.61.0 | 1 | 48.97 | 96.50 | 349.14 |

[All host metrics](performance/tcp_to_tcp_performance/default.json)

### `tcp_to_tcp_performance/high_concurrency`

1 × c5d.9xlarge consumer, 20 × c5.large producer, 1 × c5d.9xlarge subject. One run per subject.

| Subject | Version | Runs | Host network RX (MiB/s) | Host CPU user max (%) | Peak host memory (MiB) |
| --- | --- | --- | --- | --- | --- |
| Filebeat | 8.14.3 | 1 | 4.25 | 2.73 | 648.52 |
| Fluent Bit | 3.2.7 | 1 | 186.28 | 8.01 | 5300.57 |
| Fluentd (td-agent) | 4.5.2-1 | 1 | 39.84 | 2.73 | 1116.26 |
| Logstash | 8.17.3 | 1 | 15.15 | 9.81 | 2057.65 |
| Splunk HF | 9.2.2-d76edf6f0a15 | 1 | 12.06 | 8.55 | 1131.53 |
| Splunk UF | 9.2.2-d76edf6f0a15 | 1 | 481.48 | 1.06 | 640.31 |
| Vector | 0.61.0 | 1 | 133.98 | 11.41 | 1199.82 |

[All host metrics](performance/tcp_to_tcp_performance/high_concurrency.json)


## Remote correctness

These outcomes describe the checked-in configurations and assertions. The disk
persistence case verifies recovery after a restart, allowing partial
loss; it does not establish crash-safe or exactly-once delivery. Rotation and
reload cases retain their event-count and destination assertions.

### `disk_buffer_persistence_correctness/default`

| Subject | Result | Observation |
| --- | --- | --- |
| vector | Passed | Existing assertions passed. |
| filebeat | Passed | Existing assertions passed. |
| fluentd | Passed | Existing assertions passed. |
| logstash | Passed | Existing assertions passed. |
| fluentbit | Passed | Existing assertions passed. |
| splunk_heavy_forwarder | Failed | Initial delivery passed, but no test records were recovered after restarting with ten records sent during the outage. Internal logs were excluded; the recovery assertion failed. |
| splunk_universal_forwarder | Failed | Initial delivery passed, but no test records were recovered after restarting with ten records sent during the outage. Built-in wildcard log routes were excluded; the recovery assertion failed. |

### `docker_partial_events_merging_correctness/default`

| Subject | Result | Observation |
| --- | --- | --- |
| vector | Failed | One complete container input record produced two identical complete records; input-count assertion passed in a fresh reproduction. Exact output-count assertion retained. |

### `file_rotate_create_correctness/default`

| Subject | Result | Observation |
| --- | --- | --- |
| vector | Passed | Existing assertions passed. |
| filebeat | Passed | Existing assertions passed. |
| fluentbit | Passed | Existing assertions passed. |
| fluentd | Passed | Existing assertions passed. |
| logstash | Passed | Existing assertions passed. |
| splunk_heavy_forwarder | Failed | Expected three records after rename; received four. Internal logs were excluded, and both original and rotated paths were monitored. |
| splunk_universal_forwarder | Failed | Expected three newline-framed records after rename; received eight, with an empty final frame. Internal log monitors were excluded, and both paths were monitored. |

### `file_rotate_truncate_correctness/default`

| Subject | Result | Observation |
| --- | --- | --- |
| vector | Passed | Existing assertions passed. |
| filebeat | Failed | Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject. |
| fluentbit | Failed | Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject. |
| fluentd | Failed | Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject. |
| logstash | Failed | Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject. |
| splunk_heavy_forwarder | Failed | Expected three records without replaying copied content; received 4 newline-framed records. Both active and copied paths were monitored and reset before this subject. |
| splunk_universal_forwarder | Failed | Expected three records without replaying copied content; received 8 newline-framed records. Both active and copied paths were monitored and reset before this subject. |

### `file_truncate_correctness/default`

| Subject | Result | Observation |
| --- | --- | --- |
| vector | Passed | Existing assertions passed. |
| fluentbit | Passed | Existing assertions passed. |
| fluentd | Passed | Existing assertions passed. |
| splunk_heavy_forwarder | Passed | Existing assertions passed. |
| logstash | Passed | Existing assertions passed. |
| splunk_universal_forwarder | Failed | The final newline-framed record was empty, so the existing check for Hello world as the last message failed. Internal log monitors were excluded; this does not establish whether the marker appeared earlier in the stream. |

### `sighup_correctness/default`

| Subject | Result | Observation |
| --- | --- | --- |
| vector | Passed | Existing assertions passed. |
| fluentbit | Passed | Existing assertions passed. |
| fluentd | Passed | Existing assertions passed. |
| logstash | Failed | The second TCP connection was refused one second after SIGHUP while Logstash was reloading its pipeline; destination assertions were not reached. |
| splunk_heavy_forwarder | Failed | After SIGHUP, both fixture records went to the original receiver (2 records versus 0 at the new destination); expected one at each. |
| splunk_universal_forwarder | Failed | After SIGHUP, the original receiver counted four newline frames, ending empty, and the new receiver counted zero; expected one record at each. Internal log monitors were excluded. |

### `wrapped_json_correctness/default`

| Subject | Result | Observation |
| --- | --- | --- |
| filebeat | Passed | Existing assertions passed. |
| fluentd | Passed | Existing assertions passed. |
| logstash | Passed | Existing assertions passed. |
| fluentbit | Passed | Existing assertions passed. |
| vector | Passed | Existing assertions passed. |


## Standalone workloads

The finite workloads ran on a `c5.large` (2 vCPUs, 4 GiB RAM) with one warmup and
ten measured repetitions. Elapsed time includes process execution, gzip decoding,
and input streaming. Raw timings, fixture/config hashes, and the harness revision
are in the linked JSON artifacts.

| Workload / config | Mean elapsed | Sample standard deviation | Artifact |
| --- | --- | --- | --- |
| stdin to blackhole | 1.3272 s | 0.0087 s | [JSON](standalone/stdin_to_blackhole.json) |
| klog, two remap nodes | 10.6829 s | 0.0886 s | [JSON](standalone/klog_remap_to_blackhole.json) |
| klog, one remap node | 10.0367 s | 0.0584 s | [JSON](standalone/klog_remap_to_blackhole.json) |
| klog stdin baseline | 1.3277 s | 0.0101 s | [JSON](standalone/klog_remap_to_blackhole.json) |

The continuous file workload generated 480 files from empty input and checkpoint
state, warmed up for 15 seconds, then observed exporter counter deltas over 60
seconds. It received 33.56 MiB/s of raw file bytes and 69,658 file events/s; the
blackhole received 69,646 events/s. This observes the configured generator load.
No CPU or allocation profile was collected. [Measurement details](standalone/file_to_blackhole.json)
include the pinned generator revision and config hashes.

## Reproduce

Use the repository setup guide with your own AWS credentials and account-scoped
infrastructure, then run the selected case and configuration. Athena comparison
requires a catalog table with the matching dstat schema and registered result
partitions; set `VECTOR_TEST_ATHENA_DATABASE` and `VECTOR_TEST_ATHENA_OUTPUT` for
your account. This campaign used a private catalog containing only the selected
measurement partitions:

```sh
vth run -t tcp_to_tcp_performance -c default -s vector -v 0.61.0
vth compare -t tcp_to_tcp_performance -c default --json
vth teardown -t tcp_to_tcp_performance -c default --yes
```

The standalone case READMEs give their native `vth bench`, `generate`, and
`profile` commands. Historical profiling notes remain dated experiments; the
current artifacts above are the measurements made in this campaign.

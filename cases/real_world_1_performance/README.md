# Real-world syslog pipeline performance

Five producer streams exercise API, admin, media-proxy, unfurler, and audit logs.
All variants render the same `ansible/config_files/vector.yaml` template; variant
variables select the source topology, processing, and HTTP limits.

| Configuration | Difference from the baseline |
| --- | --- |
| `default` | One syslog source, one route, one VRL normalizer per stream, and five HTTP sinks. |
| `high_concurrency` | Raises per-sink request concurrency from 100 to 1000 on the same hardware. |
| `high_concurrency_with_pipelines` | Uses the high-concurrency limits with five separate syslog sources and ports; each stream retains its own normalizer and sink. |
| `one_sink` | Sends all five normalized streams to one HTTP sink. Concurrency, request-rate, and buffer budgets are scaled to retain the baseline's aggregate limits. |
| `without_lua` | Alias of `default`, retained for historical result lookup. The baseline already uses VRL. |
| `without_transforms` | Routes complete syslog events to the HTTP sinks without application normalization. This intentionally changes the output payload. |
| `without_transforms_and_rate_limiting` | The same processing bypass with Vector's default unrestricted request rate. |

## Processing and output

The syslog source supplies `hostname` and a typed `timestamp` directly. API and
admin use the syslog timestamp; media-proxy and unfurler parse their RFC 3339
body `ts` into `timestamp`. There are no timestamp-to-string round trips or
`host`/`time` renaming stages.

For the four application JSON streams, one remap decodes the body, retains its
application fields, and supplies `appname`, `hostname`, and `timestamp`. API and
admin also move their selected metadata fields to the existing flat names.
Malformed application JSON is dropped, and unfurler keeps only `msg: unfurl`.
Audit retains its raw message and syslog process ID in a small common envelope.

All HTTP sinks serialize the complete result as newline-delimited JSON with
consistent batching, request limits, and timestamp formatting. Two shared Vector
checks assert exactly one output per normalizer and zero outputs across the four
malformed-JSON paths, using `expected_event_count` alongside field assertions.
The Lua and regex microbenchmarks remain separate cases.

Run a selected variant with `vth run -t real_world_1_performance -c <configuration> -s vector`.
Validate the Ansible-rendered config with Vector 0.61 before provisioning.

These definitions replace the historical pipelines and allowlists, and the
concurrency variants now use the baseline machine sizes. Establish fresh
performance baselines; earlier results do not describe the same workloads.

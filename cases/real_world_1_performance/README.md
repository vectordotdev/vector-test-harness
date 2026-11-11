# Real-world syslog pipeline performance

Five producer streams exercise API, admin, media-proxy, unfurler, and audit logs.
The Vector 0.61 configurations retain the variant-specific topology, filtering,
field selection, metadata processing, batching, and request limits.

| Configuration | Vector workload |
| --- | --- |
| `default`, `high_concurrency` | The shared syslog source and current VRL processing in `vector.yaml`. |
| `high_concurrency_with_pipelines` | Separate syslog sources and processing stages, including Lua v2 metadata extraction. |
| `one_sink` | Only the API filter and its HTTP sink. |
| `without_lua` | Shared syslog source, VRL parsing/coercion/renaming, and direct metadata field moves. |
| `without_transforms` | Routing filters and HTTP sinks, without parsing and field processing. |
| `without_transforms_and_rate_limiting` | Routing-only variant with the historical relaxed request limits. |

The former TOML variants have been migrated to YAML and current component APIs.
The Lua-free and per-pipeline variants include Vector unit tests for each stream.
The migrated TOML variants preserve newline-delimited JSON framing.

Run a selected variant with `just run -t real_world_1_performance -c <configuration> -s vector`.
Validate the Ansible-rendered config with Vector 0.61 before provisioning. The
per-pipeline configuration requires the distinct ports from its own variant.

These updated workloads establish new performance baselines. Historical results
from the earlier transform implementations are not directly comparable.

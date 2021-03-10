# Notes on `file_to_blackhole`

## 2021/03/10

Vector target `0327d46a9d213b5390e11d86b920224bdf91ed38` is set per following:

```toml
data_dir = "data"

[sources.internal]
type = "internal_metrics"

[sources.file]
type = "file"
max_line_bytes = 102400
fingerprint.strategy = "device_and_inode"
include = ["logs/*.log"]

[sinks.prometheus]
type = "prometheus_exporter"
inputs = ["internal"]
address = "0.0.0.0:9001"

[sinks.blackhole]
type = "blackhole"
inputs = ["file"]
```

With a `file_gen` -- target `9e13b40919109e1737ba5388ef9eeb07d0230608` -- config
like so:

```toml
random_seed = 20210309

# Set the number of background worker threads that this run will use.
worker_threads = 2

[targets.plain]
path_template = "logs/%NNN%-plain.log"
duplicates = 10
variant = "Ascii"
maximum_bytes_per_file = "4Gb"
bytes_per_second = "25 Kb"
maximum_line_size_bytes = "5 Kb"
```

Vector keeps up per included grafana dashboard. Production into files is a
relatively steady 250Kb / second with file source mirroring this. Blackhole sink
is at 267Kb/s; internal overhead added per message expected. If
`bytes_per_second` is adjusted to 250Kb per file `file_gen` generates at 2.5Mb/s
as expected, vector source/sink adjust similarly. Table follows with other
parameters and observed outcomes:

| `duplicates` | `bytes_per_second` | `maximum_line_size_bytes` | `file_gen` bytes/sec throughput | file source `sum(rate(vector_processed_bytes_total[1m]))` | blackhole sink `sum(rate(vector_processed_bytes_total[1m]))` | delta `file_gen`, `file` |
| ------------ | ------------------- | ------------------------ | ------------------------------- | --------------------------------------------------------- | ---------------------------------------- | ------------------ |
| 10 | 25 Kb/sec | 5 Kb | 250 Kb/sec | 250 Kb/sec | 267 Kb/sec | 0 Kb/sec |
| 10 | 250 Kb/sec | 5 Kb | 2.5 Mb/sec | 2.5 Mb/sec | 2.68 Mb/sec | 0 Mb/sec |
| 10 | 2.5 Mb/sec | 5 Kb | 25 Mb/sec | 25 Mb/sec | 26.8 Mb/sec | 0 Mb/sec |
| 10 | 2.5 Mb/sec | 10 Kb | 25 Mb/sec | 25 Mb/sec | 26.2 Mb/sec | 0 Mb/sec |
| 10 | 2.5 Mb/sec | 100 Kb | 25 Mb/sec | 25 Mb/sec | 25.7 Mb/sec | 0 Mb/sec |
| 10 | 2.5 Mb/sec | 100 Kb | 25 Mb/sec | 25 Mb/sec | 25.7 Mb/sec | 0 Mb/sec |
| 10 | 2.5 Mb/sec | 105 Kb | 25 Mb/sec | 23.9 Mb/sec | 24.6 Mb/sec | 1.1 Mb/sec |
| 10 | 2.5 Mb/sec | 110 Kb | 25 Mb/sec | 22 Mb/sec | 22.06 Mb/sec | 3 Mb/sec |
| 10 | 2.5 Mb/sec | 125 Kb | 25 Mb/sec | 17 Mb/sec | 17.04 Mb/sec | 9 Mb/sec |
| 10 | 2.5 Mb/sec | 150 Kb | 25 Mb/sec | 12.3 Mb/sec | 12.6 Mb/sec | 12.7 Mb/sec |
| 10 | 2.5 Mb/sec | 200 Kb | 25 Mb/sec | 6 Mb/sec | 6.15 Mb/sec | 19 Mb/sec |
| 10 | 2.5 Mb/sec | 300 Kb | 25 Mb/sec | 3 Mb/sec | 3.08 Mb/sec | 22 Mb/sec |
| 10 | 2.5 Mb/sec | 400 Kb | 25 Mb/sec | 1.53 Mb/sec | 1.57 Mb/sec | 23.47 Mb/sec |
| 10 | 2.5 Mb/sec | 500 Kb | 25 Mb/sec | 1.05 Mb/sec | 1.07 Mb/sec | 23.95 Mb/sec |
| 10 | 2.5 Mb/sec | 900 Kb | 25 Mb/sec | 231 Kb/sec | 237 Kb/sec | 24.77 Mb/sec |

Note that each `maximum_line_size_bytes` is chosen to avoid going over the file
source's maximum line size. Drop-off in the file source happens once the line
size goes beyond 100 Kb, notable even at 105Kb. No hypothesis at this time.

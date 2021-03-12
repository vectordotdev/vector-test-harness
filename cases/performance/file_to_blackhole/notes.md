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

### Source Exploration

The main entry point for the file source is `src/sources/file.rs` function
`file_source`. This wires up user config, data directory and out
`Pipeline`. Interior return is a boxed async move, `FileServer` from
`file_server` crate is setup as a
[`spawn_blocking`](https://docs.rs/tokio/1.3.0/tokio/task/fn.spawn_blocking.html). Possible
that `block_in_place` would be more ideal but realistically we'd be async all
the way down. `FileServer` is adapted from
[cernan](https://github.com/postmates/cernan/tree/master/src/source/file) code,
notably written before Rust async's story was ideal. That said, I do not find it
explicable that this would be the reason for fall-off once line lengths go
beyond a certain limit, absent other indications of blocking.

## 2021/03/12

As noted by Luke Steensen the drop-off above 100Kb is partially explicable owing
the a bug in the vector config. I had intended the `max_line_bytes` setting to
be 1MB but it was set to 100KB. Vector config is now adjusted to:

```toml
data_dir = "data"

[sources.internal]
type = "internal_metrics"

[sources.file]
type = "file"
max_line_bytes = 1024000
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

New results, `bytes_per_second` held to 2.5 Mb/sec:

| `duplicates` | `bytes_per_second ` |  `maximum_line_size_bytes` | `file_gen` bytes/sec throughput | file source `sum(rate(vector_processed_bytes_total[1m]))` | delta `file_gen`, `file` |
| ------------ | ------------------- | ------------------- | ------------------------------- | --------------------------------------------------------- | ------------------------ |
| 10           | 2.5 Mb/sec           | 10 Kb/sec           | 25 Mb/sec             | 25 Mb/sec            | 0 Kb/sec                |
| 10           | 25 Mb/sec            | 10 Kb/sec           | 246 Mb/sec            | 219 Mb/sec           | 27 Mb/sec               |
| 10           | 50 Mb/sec            | 10 Kb/sec           | 475 Mb/sec            | 221 Mb/sec           | 254 Mb/sec              |
| 10           | 2.5 Mb/sec           | 100 Kb/sec          | 25 Mb/sec             | 25 Mb/sec            | 0 Kb/sec                |
| 50           | 2.5 Mb/sec           | 10 Kb/sec           | 125 Mb/sec            | 125 Mb/sec           | 0 Mb/sec                |
| 75           | 2.5 Mb/sec           | 10 Kb/sec           | 187 Mb/sec            | 187 Mb/sec           | 0 Mb/sec                |
| 85           | 2.5 Mb/sec           | 10 Kb/sec           | 212 Mb/sec            | 212 Mb/sec           | 0 Mb/sec                |
| 90           | 2.5 Mb/sec           | 10 Kb/sec           | 225 Mb/sec            | 222 Mb/sec           | 3 Mb/sec                |
| 95           | 2.5 Mb/sec           | 10 Kb/sec           | 237 Mb/sec            | 216 Mb/sec           | 19 Mb/sec               |
| 100          | 2.5 Mb/sec           | 10 Kb/sec           | 250 Mb/sec            | 221 Mb/sec           | 29 Mb/sec               |
| 255          | 2.5 Mb/sec           | 10 Kb/sec           | 616 Mb/sec            | 222 Mb/sec           | 394 Mb/sec              |
| 510          | 100 Kb/sec           | 1 Kb/sec            | 48 Mb/sec             | 40 Mb/sec            | 8 Mb/sec                |

One thing stood out in the tests: if I allowed vector to develop a backlog and
then stopped `file_gen` the throughput would gradually catch up, before
inevitably falling to zero when all the bytes were processed. There are two
dimensions that cause the file source to fall behind, based on the above table:
total number of files and byte throughput of files. 510 files of relatively
meager throughput -- 48 Mb/sec -- forced file source to fall behind by 8 Mb/sec,
a relatively small number of files -- 10 -- producing at 50Mb/sec caused the
file source to fall behind by 254 Mb/sec. The progenitor code for `file-source`
was concerned with fairness and not throughput, which I believe shows today in
the data.

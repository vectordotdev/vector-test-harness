# tcp_to_tcp_performance

```sh
vth check tcp_to_tcp_performance --vector-bin /path/to/vector --seed 7
vth check tcp_to_tcp_performance --fluent-bit-bin /path/to/fluent-bit --seed 7
```

Both use the same finite publisher and byte-exact receiver. Vector decodes
newline-delimited bytes and forwards the original text. The Fluent Bit 5.2.0
adapter uses TCP `format: none` and
[`raw_message_key: '$log'`](https://docs.fluentbit.io/manual/data-pipeline/outputs/tcp-and-tls)
to forward that text with newline framing, without a JSON wrapper.
Matching seeds and event counts produce identical input; each run retains a
distinct run ID.

Uses the shared [finite workload contract](../../README.md#finite-workload-contract).
This verifies delivered records and records diagnostic finite completion times;
comparative rankings are not published.

The profiles use volatile memory and plain TCP without application
acknowledgements. Vector suppresses optional peer host/port fields. Its
1,048,576-event blocking sink queue and Fluent Bit's 512 MiB input limit are
sized to hold the full million-record fixture; they do not establish equivalent
queue capacity or process memory limits. Fluent Bit closes input connections
[when paused](https://github.com/fluent/fluent-bit/blob/v5.2.0/plugins/in_tcp/tcp.c),
so a pause, overflow or retry disqualifies a comparison run. Overload and crash
recovery are outside this workload.

Fluent Bit uses a 50 ms flush interval and two output workers; Vector uses two
runtime workers and streams output. Scheduling and batching remain part of the
measured completion time. Comparisons require equal whole-process CPU placement,
identical input hashes, matching received wire bytes and separate apparatus
headroom checks.
Other product profiles use their native framing and acknowledgements through
the same application-record verifier.

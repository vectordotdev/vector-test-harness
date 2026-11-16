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

The Vector and Fluent Bit configurations above use volatile memory queues
without application acknowledgements. Buffering and backpressure differ: Vector blocks at a
512-event sink queue; Fluent Bit uses `mem_buf_limit: 16M` with chunk-based
accounting and closes TCP input connections
[when paused](https://github.com/fluent/fluent-bit/blob/v5.2.0/plugins/in_tcp/tcp.c).
Queue capacities are not equivalent, and overload behavior is outside this check.
Other product profiles use their native framing and acknowledgements through
the same application-record verifier.

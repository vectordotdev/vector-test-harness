# tcp_to_http_performance

Run locally: `vth check tcp_to_http_performance --vector-bin /path/to/vector`.

The TCP source forwards records to an HTTP/1 receiver as uncompressed POST
batches of newline-delimited JSON containing only `message`.
Batching uses a 256-event limit, 100 ms flush timeout and one request at a time.

Uses the shared [finite workload contract](../../README.md#finite-workload-contract).

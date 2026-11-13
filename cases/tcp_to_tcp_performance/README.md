# tcp_to_tcp_performance

Run locally: `vth check tcp_to_tcp_performance --vector-bin /path/to/vector`.

The TCP source decodes newline-delimited bytes and the TCP sink forwards the
original text with newline framing.

Uses the shared [finite workload contract](../../README.md#finite-workload-contract).

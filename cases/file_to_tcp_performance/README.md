# file_to_tcp_performance

Run locally: `vth check file_to_tcp_performance --vector-bin /path/to/vector`.

The file source reads a fresh finite fixture from the beginning with fresh state
and delivers each unchanged message to the shared TCP verifier using native framing.
Files are recently written; the filesystem cache is not evicted.

Uses the shared [finite workload contract](../../README.md#finite-workload-contract).

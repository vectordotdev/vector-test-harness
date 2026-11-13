# regex_parsing_performance

Run locally: `vth check regex_parsing_performance --vector-bin /path/to/vector`.

The TCP source feeds a `parse_regex!` remap; the TCP sink emits newline-delimited
JSON. The receiver checks exact objects containing eight string fields:
`host`, `user`, `bytes_in`, `timestamp`, `method`, `path`, `status`, `bytes_out`.

Uses the shared [finite workload contract](../../README.md#finite-workload-contract).

# regex_parsing_performance

Run locally: `vth check regex_parsing_performance --vector-bin /path/to/vector`.

Each profile parses the same TCP input using native processing. The shared
receiver decodes native output framing and checks exact objects with eight string fields:
`host`, `user`, `bytes_in`, `timestamp`, `method`, `path`, `status`, `bytes_out`.

Uses the shared [finite workload contract](../../README.md#finite-workload-contract).

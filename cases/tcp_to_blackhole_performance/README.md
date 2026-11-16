# tcp_to_blackhole_performance

Run locally: `vth check tcp_to_blackhole_performance --vector-bin /path/to/vector`.
Supported profiles send the finite TCP fixture to a native discard output and
require its exact terminal count. This verifies no identities, payloads or
delivery, and receives no completion timing. Artifacts retain the configuration,
native counter evidence, health checks and shutdown result.

Remote runs remain retired. No capacity or comparative-throughput result is
published; see the [comparison requirements](../../docs/comparisons.md).

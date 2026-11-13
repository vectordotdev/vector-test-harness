# tcp_to_blackhole_performance

Run locally: `vth check tcp_to_blackhole_performance --vector-bin /path/to/vector`.
The check sends a finite fixture through Vector's newline-delimited TCP source
to the blackhole sink and checks its terminal cumulative event count.
This verifies the count only; discarded records cannot verify identities or delivery.
Run artifacts include the rendered config, Vector log and verification report.
Remote runs remain retired. Capacity measurement, competitor validation and
publishable throughput are pending; see the [comparison audit](../../docs/comparisons.md).

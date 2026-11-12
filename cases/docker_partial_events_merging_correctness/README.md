# Docker partial-event merging diagnostic

This is a Vector-only regression diagnostic, with no cross-product scorecard.
A container emits one JSON record larger than Docker's splitting threshold. The
fixture verifies that `docker logs` contains one complete input, including its
final `bzz` field. The receiver must obtain exactly one JSON record with the
expected message, all 1,352 padding fields and the final field intact.

The [retained AWS observation](../../results/2026-11-12/README.md) failed: the
receiver obtained two identical complete records. The count assertion remains;
the observation does not establish whether the defect is in Vector or the fixture.

```sh
vth run -t docker_partial_events_merging_correctness -s vector -v 0.61.0
```

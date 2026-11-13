# Docker partial-event merging diagnostic

This is a functional diagnostic, with no timing or cross-product scorecard.
A real container emits one JSON record larger than Docker's splitting threshold. The
fixture verifies that `docker logs` contains one complete input, including its
final `bzz` field. The receiver must obtain exactly one JSON record with the
expected message and all 1,352 padding fields, including `bzz`, intact.

The local runner verifies the actual Docker fragments and the producer's natural
exit. Vector's native container die and balanced watcher-completion events must
arrive before subject shutdown. Products ingest through their native Docker API
or JSON-file input; reassembly happens inside the product. Linux host access to
Docker and its log files is required. Prepare an immutable fixture image with
`/bin/sh`, `cat` and `sleep`, then run:

```sh
VTH_DOCKER_IMAGE=sha256:<prepared-image-id> vth check docker_partial_events_merging_correctness \
  --subject vector --subject-bin /path/to/vector
```

The [legacy AWS observation](../../results/2026-11-12/README.md) failed: the
receiver obtained two identical complete records. The count assertion remains;
the observation does not establish whether the defect is in Vector or the fixture.

```sh
vth run -t docker_partial_events_merging_correctness -s vector -v 0.61.0
```

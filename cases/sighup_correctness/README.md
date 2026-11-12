# sighup_correctness

This remote case has been retired.

A fixed one-second delay does not establish comparable reload completion across products. The Vector-only local check remains supported.

See the [comparison audit](../../docs/comparisons.md) for the publication criteria.
Historical scorecards and new numerical comparisons have been removed; raw run
artifacts remain in private storage and the case history remains in Git.

Run the Vector-only check with:

```sh
vth check sighup_correctness --vector-bin /path/to/vector
```

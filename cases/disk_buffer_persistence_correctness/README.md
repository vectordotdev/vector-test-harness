# Disk Buffering Persistence Correctness Test

This test is designed to ensure data is persisted to disk properly when disk buffers are
configured. This is an important test to reduce data loss and prevent disruption when
downstream services fail.

## Design

1. Configure all subjects to use disk-buffers output to a downstream service.
2. Start the subject.
3. Shutdown the downstream service.
4. Send data to the subject.
5. Restart the subject.
6. Bring the downstream service back online.
7. Assert that data was evetually received.

### Caveats

This test is not designed to verify at least once delivery, some data loss is acceptable.
This test verifies that data is reasonably persisted to disk and made available between restarts.

Logstash enables [`pipeline.unsafe_shutdown`](https://www.elastic.co/docs/reference/logstash/shutdown)
for this case so it can stop while the receiver is offline, then replay its
persistent queue after restarting. Its default graceful shutdown waits for the
unavailable output indefinitely.

## AWS baseline — 2026-11-12

See the [current AWS report](../../results/2026-11-12/README.md) for this case’s
configurations, pinned versions, hardware, measured outcomes, and sampling limits.
Performance figures use one run per subject/configuration. Correctness results
retain the existing assertions, including any observed failures.

## Historical results

These tables archive earlier Vector versions and workload definitions. Use the
dated AWS baseline above for current measurements; software, configurations, and
measurement methods differ from the historical experiments.

|     Subject | Result  |
|------------:|:-------:|
|    Filebeat |    ✅    |
|   FluentBit |    ❌    |
|     FluentD |    ❌    |
|    Logstash |  ⚠️[1]  |
|   Splunk HF |    ✅    |
|   Splunk UF |    ✅    |
|  **Vector** |    ✅    |

1. After many hours of trying, were unable to get Logstash to work at all for this test. It seems
   unlikely that their persistent queues feature does not work, and it's more likely that we do not
   understand something. The documentation failed to help us.

## Try It

```bash
vth run -t disk_buffer_persistence_correctness
```

## Resources

* [Setup][setup]
* [Development][development]
* [How it works][how_it_works]
* [Vector docs][docs]
* [Vector repo][repo]
* [Vector website][website]


[development]: /README.md#development
[docs]: https://vector.dev/docs/
[how_it_works]: /README.md#how-it-works
[repo]: https://github.com/vectordotdev/vector
[setup]: /README.md#setup
[website]: https://vector.dev

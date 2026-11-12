# SIGHUP Correctness Test

This test verifies that the subject correctly responds to the `SIGHUP` signal, gracefully
reloading configuration changes.

## Design

1. Two consumers are set up on separate hosts (HTTP for Fluentd, TCP for other subjects).
2. Each subject is initially configured to send data to the first consumer.
3. A test line is sent to the subject.
4. The subject's configuration file is updated to send data to the second consumer.
5. The `SIGHUP` signal is sent to the subject's process.
6. A second test line is sent to the subject.
7. Assert that the first server received the first line and the second server
   received the second line.

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
|    Filebeat |    ❌    |
|   FluentBit |    ❌    |
|     FluentD |    ❌    |
|    Logstash |  ⚠️[1]  |
|   Splunk HF |    ✅    |
|   Splunk UF |    ✅    |
|  **Vector** |    ✅    |

1. Logstash failed the test because it does not gracefully reload change. Instead of performs
   a full restart, making Logstash unavaible for a period of time. Because of this it failed to
   receive the second line. In order for this test to pass we had to wait 30 seconds to send
   the second line.

## Try It

Run the local Vector check without cloud infrastructure:

```sh
vth check sighup_correctness --vector-bin /path/to/vector
```

For the remote case:

```sh
vth run -t sighup_correctness -s vector
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

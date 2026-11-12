# File Rotate Create Correctness Test

This test verifies that a file is correctly followed when rotated in the manner of
`logrotate`'s `create` directive. The subject should continue to read data sent to the
rotated file and recognize the new file in it's place. Failure to do so results in data
loss as well as duplicate data.

## Design

1. Each subject is configured to tail a test log file and send it to a downstream service.
2. The subject is started.
3. The test log file is renamed in a way that it matches the configured patterns.
4. A new empty file is created in it's place.
5. A unique new line is written to both files.
6. Assert that the downstream service received both new unique lines _and_ did not receive
   duplicate data.

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
|   FluentBit |    ✅    |
|     FluentD |    ✅    |
|    Logstash |    ✅    |
|   Splunk HF |    ✅    |
|   Splunk UF |    ✅    |
|  **Vector** |    ✅    |

## Try it

You can run this test via:

```
vth run -t file_rotate_create_correctness -s vector
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
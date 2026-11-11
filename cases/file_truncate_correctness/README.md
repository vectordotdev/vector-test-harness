# File Truncate Correctness Test

This test verifies that the read position of a file is reset in the event of
truncation. Failing to do so typically results loss of any new data written to the file.

## Design

1. Each subject is configured to tail a test log file and send it to a downstream service.
2. The subject is started.
3. The test log file is truncated.
4. A unique new line is added to the test log file.
5. Assert that the downstream service received the unique new line.

## Historical results

These results describe earlier Vector versions and workload definitions. They
have not been re-measured with the Vector 0.61 configurations; establish a new
baseline before comparing performance or claiming current correctness.

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
vth run -t file_truncate_correctness -s vector
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
# File Rotate Truncate Correctness Test

This test verifies that a file is correctly followed when rotated in the manner of
`logrotate`'s `copytruncate` directive. The subject should continue to read new data sent
to the rotated file and correct reset it's position on the newly truncated file.
Failure to do so results in data loss as well as duplicate data.

## Design

1. Each subject is configured to tail a test log file and send it to a downstream service.
2. The subject is started.
3. The test log file is renamed in a way that it matches the configured patterns.
4. Data is copied to a new rotated file.
5. The target file is truncated.
6. A unique new line is written to both files.
7. Assert that the downstream service received both new unique lines _and_ did not receive
   duplicate data.

## Results

|     Subject | Result  |
|------------:|:-------:|
|    Filebeat |    ❌    |
|   FluentBit |    ❌    |
|     FluentD |    ❌    |
|    Logstash |    ❌    |
|   Splunk HF |    ✅    |
|   Splunk UF |    ✅    |
|  **Vector** |    ✅    |

## Try it

You can run this test via:

```
test -t file_rotate_truncate_correctness
```

## Resources

* [Setup][setup]
* [Development][development]
* [How it works][how_it_works]
* [Vector docs][docs]
* [Vector repo][repo]
* [Vector website][website]


[development]: /README.md#development
[docs]: https://docs.vectorproject.io
[how_it_works]: /README.md#how-it-works
[repo]: https://github.com/timberio/vector
[setup]: /README.md#setup
[website]: https://vectorproject.io
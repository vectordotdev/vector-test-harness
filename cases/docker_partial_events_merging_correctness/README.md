# Docker Partial Events Merging Correctness Test

This test verifies that vector properly merges partial events when used with Docker.

## Results

|     Subject | Result  |
|------------:|:-------:|
|    Filebeat |          |
|   FluentBit |          |
|     FluentD |          |
|    Logstash |          |
|   Splunk HF |          |
|   Splunk UF |          |
|  **Vector** |    ✅    |

## Design

Subject is configured to read logs that an actual Docker container prints to stdout.

1. Container prints a really long (> 16 Kb) message to stdout.
2. Docker splits the event into multiple parts and emits them separately.
3. Source reads the logs and merges the partial messages together, and outputs the merged event.

## Try It

You can run this test via:

```
test -t docker_partial_events_merging_correctness
```

## Resources

* [Setup][setup]
* [Development][development]
* [How it works][how_it_works]
* [Vector docs][docs]
* [Vector repo][repo]
* [Vector website][website]


[development]: /README.md#development
[docs]: https://vector.dev/docs
[how_it_works]: /README.md#how-it-works
[repo]: https://github.com/timberio/vector
[setup]: /README.md#setup
[website]: https://vector.dev

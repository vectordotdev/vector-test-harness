# Docker Partial Events Merging Correctness Test

This test verifies that vector properly merges partial events when used with Docker.

## Historical results

These results describe earlier Vector versions and workload definitions. They
have not been re-measured with the Vector 0.61 configurations; establish a new
baseline before comparing performance or claiming current correctness.

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
3. The Docker source collects only the named test container, merges the partial
   messages, and sends the raw record directly to the TCP consumer.
4. The test driver decodes the received JSON and checks that exactly one complete
   record arrived, including all 1,352 padding fields and the final `bzz` field.
   Application JSON parsing is outside the Vector pipeline being tested.

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

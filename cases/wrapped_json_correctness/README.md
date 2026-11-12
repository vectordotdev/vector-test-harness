# Wrapped JSON Correctness Test

This test verifies that wrapped JSON documents are parsed correctly:

```json
{"log":"{\"message\":\"Hello world\"}"}
```

This is a _very_ important for environments that wrap log output in a JSON document, such
as Docker and Kubernetes.

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
|     FluentD |    ✅    |
|    Logstash |    ✅    |
|   Splunk HF |    ✅    |
|   Splunk UF |    ✅    |
|  **Vector** |    ✅    |

## Design

The subject is configured to:

1. Receive the following data:

   ```json
   {"log":"{\"message\":\"Hello world\"}"}
   ```

2. Decode the outer JSON envelope with the socket source's JSON codec.
3. Decode the inner `log` string with one remap and remove that envelope field.
4. Output the decoded application fields, re-encoded as JSON. Other outer fields
   are retained.

## Try It

Run the local Vector check without cloud infrastructure:

```sh
vth check wrapped_json_correctness --vector-bin /path/to/vector
```

For the remote case:

```sh
vth run -t wrapped_json_correctness -s vector
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

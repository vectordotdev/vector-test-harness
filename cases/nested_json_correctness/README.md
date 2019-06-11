# Nested JSON Correctness Test

This test verifies that nested JSON documents are parsed correctly:

```json
{"log":"{\"message\":\"Hello world\"}"}
```

This is a _very_ important for environments that warp log output in a JSON document, such
as Docker and Kubernetes.

## Results

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

2. Parse the data as JSON
3. Output the data, re-encoded as JSON

## Try It

You can run this test via:

```
test -t nested_json_correctness
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
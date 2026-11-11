# Wrapped JSON Correctness Test

This test verifies that wrapped JSON documents are parsed correctly:

```json
{"log":"{\"message\":\"Hello world\"}"}
```

This is a _very_ important for environments that wrap log output in a JSON document, such
as Docker and Kubernetes.

## Historical results

These results describe earlier Vector versions and workload definitions. They
have not been re-measured with the Vector 0.61 configurations; establish a new
baseline before comparing performance or claiming current correctness.

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
just check-case wrapped_json_correctness --vector-bin /path/to/vector
```

For the remote case:

```sh
just run -t wrapped_json_correctness -s vector
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

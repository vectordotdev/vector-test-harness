# Wrapped JSON correctness

Every supported subject receives the same input:

```json
{"log":"{\"message\":\"Hello world\"}"}
```

After outer and inner JSON decoding, the local check requires exactly one
application record: `{"message":"Hello world"}`. Native transport envelopes are
decoded before strict equality is checked. This check does not compare throughput
or general product quality.

Local profiles cover all eight products. Remote profiles cover Filebeat, Fluent
Bit, Fluentd, Logstash and Vector. The
[retained AWS results](../../results/2026-11-12/README.md) record their pinned
versions, actual received counts and observed payload checks.

```sh
vth run -t wrapped_json_correctness -s vector -v 0.61.0
vth check wrapped_json_correctness --vector-bin /path/to/vector
```

The local check rejects a retained `log` envelope or extra application fields.
See the [comparison requirements](../../docs/comparisons.md).

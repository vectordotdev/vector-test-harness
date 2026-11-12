# Wrapped JSON correctness

Every supported subject receives the same input:

```json
{"log":"{\"message\":\"Hello world\"}"}
```

After outer and inner JSON decoding, exactly one application record must contain
`message: Hello world`. Transport adapters decode Beats or JSON output before the
same count and payload assertion. Additional product metadata is outside this
contract. This check does not compare throughput or general product quality.

Supported subjects are Filebeat, Fluent Bit, Fluentd, Logstash and Vector. The
[retained AWS results](../../results/2026-11-12/README.md) record their pinned
versions, actual received counts and observed payload checks.

```sh
vth run -t wrapped_json_correctness -s vector -v 0.61.0
vth check wrapped_json_correctness --vector-bin /path/to/vector
```

The local Vector check additionally verifies that the envelope field is removed.
See the [comparison requirements](../../docs/comparisons.md).

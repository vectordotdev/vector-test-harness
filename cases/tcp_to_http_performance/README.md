# tcp_to_http_performance

Run locally: `vth check tcp_to_http_performance --vector-bin /path/to/vector`.

Profiles forward the same TCP messages to an HTTP/1 receiver with native encoding
and batching. The receiver verifies each application record inside a complete
request before returning success. Filebeat uses Bulk acknowledgement emulation;
there is no Elasticsearch indexing or durability proof.

Vector's profile uses newline-delimited JSON, a 256-event limit, a 100 ms flush
timeout and one request at a time. Other products retain their native batching.

Splunk HF/UF HTTPOUT carries an [S2S payload](https://help.splunk.com/en/splunk-enterprise/get-started/splunk-validated-architectures/getting-data-in-forwarding-and-preprocessing/intermediate-data-routing-using-universal-and-heavy-forwarders) and needs the corresponding receiver, so its protocol verification remains pending.

Uses the shared [finite workload contract](../../README.md#finite-workload-contract).

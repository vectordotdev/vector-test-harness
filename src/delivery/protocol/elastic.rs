//! Elasticsearch Bulk envelope decoding for the Filebeat output under test.

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

const MAX_BODY: usize = 4 * 1024 * 1024;

/// The native client's discovery requests are protocol stubs. This receiver
/// verifies records in memory; it does not index them or promise durability.
pub(in crate::delivery) fn handshake(path: &str) -> Option<Value> {
    match path {
        "/" => Some(json!({
            "name":"vth-memory-verifier",
            "cluster_name":"vth-memory-verifier",
            "version":{
                "number":"9.5.6",
                "build_flavor":"default",
                "build_type":"tar",
                "build_snapshot":false,
            },
        })),
        "/_license" => Some(json!({"license":{
            "uid":"00000000-0000-0000-0000-000000000001",
            "type":"basic",
            "status":"active",
            "issue_date_in_millis":0,
            "expiry_date_in_millis":4_102_444_800_000_u64,
            "max_nodes":1,
            "issued_to":"vth-memory-verifier",
            "issuer":"vth-memory-verifier",
            "start_date_in_millis":0,
        }})),
        _ => None,
    }
}

/// Return application records and matching Bulk response items. The caller must
/// verify every record before sending these success responses.
pub(in crate::delivery) fn decode_bulk(input: &[u8]) -> Result<(Vec<Value>, Vec<Value>)> {
    ensure!(
        !input.is_empty() && input.len() <= MAX_BODY,
        "invalid Bulk body length"
    );
    ensure!(input.ends_with(b"\n"), "Bulk body must end with a newline");
    let lines: Vec<_> = input[..input.len() - 1]
        .split(|byte| *byte == b'\n')
        .collect();
    ensure!(
        lines.len().is_multiple_of(2),
        "Bulk action/document pairs are incomplete"
    );
    let mut records = Vec::with_capacity(lines.len() / 2);
    let mut responses = Vec::with_capacity(lines.len() / 2);
    for pair in lines.chunks_exact(2) {
        let action: Value = serde_json::from_slice(pair[0]).context("invalid Bulk action JSON")?;
        let action = action
            .as_object()
            .context("Bulk action must be an object")?;
        ensure!(
            action.len() == 1,
            "Bulk action must contain exactly one operation"
        );
        let (operation, metadata) = action.iter().next().unwrap();
        ensure!(
            matches!(operation.as_str(), "create" | "index"),
            "only Bulk create/index operations are supported"
        );
        let metadata = metadata
            .as_object()
            .context("Bulk operation metadata must be an object")?;
        let index = metadata
            .get("_index")
            .and_then(Value::as_str)
            .context("Bulk index is missing")?;
        ensure!(!index.is_empty(), "Bulk index must not be empty");
        let id = match metadata.get("_id") {
            Some(id) => id
                .as_str()
                .context("Bulk document ID must be a string")?
                .to_owned(),
            None => format!("vth-{}", records.len()),
        };
        let mut record: Value =
            serde_json::from_slice(pair[1]).context("invalid Bulk document JSON")?;
        let fields = record
            .as_object_mut()
            .context("Bulk document must be an object")?;
        fields.remove("@timestamp");
        fields.remove("@metadata");
        records.push(record);
        responses.push(json!({(operation):{
            "_index":index,"_id":id,"status":if operation == "create" { 201 } else { 200 },
        }}));
    }
    Ok((records, responses))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_pairs_keep_application_fields_and_acknowledge_the_native_operation() {
        let body = concat!(
            "{\"create\":{\"_index\":\"vth\"}}\n",
            "{\"@timestamp\":\"now\",\"message\":\"first\",\"unexpected\":true}\n",
            "{\"index\":{\"_index\":\"vth\",\"_id\":\"second-id\"}}\n",
            "{\"@metadata\":{},\"message\":\"second\"}\n"
        );
        let (records, responses) = decode_bulk(body.as_bytes()).unwrap();
        assert_eq!(
            records,
            [
                json!({"message":"first","unexpected":true}),
                json!({"message":"second"})
            ]
        );
        assert_eq!(responses[0]["create"]["status"], 201);
        assert_eq!(responses[1]["index"]["_id"], "second-id");
        assert_eq!(responses[1]["index"]["status"], 200);
        for invalid in [
            "",
            "{\"create\":{\"_index\":\"vth\"}}\n",
            "{\"create\":{\"_index\":\"vth\"}}\n{}",
            "{\"delete\":{\"_index\":\"vth\"}}\n{}\n",
            "{\"create\":{}}\n{}\n",
            "{\"create\":{\"_index\":\"vth\"}}\n[]\n",
        ] {
            assert!(decode_bulk(invalid.as_bytes()).is_err(), "{invalid:?}");
        }
    }
}

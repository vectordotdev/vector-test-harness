//! The uncompressed Lumberjack v2 JSON path selected by the Filebeat configs.

use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;

use super::Frame;

const MAX_PAYLOAD: usize = 4 * 1024 * 1024;
const MAX_WINDOW: u32 = 65_536;

#[derive(Default)]
pub(in crate::delivery) struct Decoder {
    // Remaining records and next sequence within the current batch window.
    window: Option<(u32, u32)>,
}

impl Decoder {
    pub(in crate::delivery) fn decode(&mut self, input: &[u8]) -> Result<Option<Frame>> {
        if input.len() < 2 {
            return Ok(None);
        }
        ensure!(input[0] == b'2', "expected Lumberjack protocol version 2");
        match input[1] {
            b'W' => {
                ensure!(self.window.is_none(), "incomplete Lumberjack batch window");
                if input.len() < 6 {
                    return Ok(None);
                }
                let size = u32::from_be_bytes(input[2..6].try_into().unwrap());
                ensure!(
                    (1..=MAX_WINDOW).contains(&size),
                    "invalid Lumberjack window size"
                );
                self.window = Some((size, 1));
                Ok(Some(Frame {
                    consumed: 6,
                    records: Vec::new(),
                    reply: Vec::new(),
                }))
            }
            b'J' => {
                let (remaining, expected) =
                    self.window.context("Lumberjack JSON outside a window")?;
                if input.len() < 10 {
                    return Ok(None);
                }
                let sequence = u32::from_be_bytes(input[2..6].try_into().unwrap());
                ensure!(
                    sequence == expected,
                    "unexpected Lumberjack sequence number"
                );
                let length = u32::from_be_bytes(input[6..10].try_into().unwrap()) as usize;
                ensure!(
                    (1..=MAX_PAYLOAD).contains(&length),
                    "Lumberjack JSON payload exceeds limit or is empty"
                );
                let consumed = 10 + length;
                if input.len() < consumed {
                    return Ok(None);
                }
                let mut record: Value = serde_json::from_slice(&input[10..consumed])
                    .context("invalid Lumberjack JSON event")?;
                let fields = record
                    .as_object_mut()
                    .context("Lumberjack event must be an object")?;
                // Product-generated envelope fields are outside the application contract.
                // Everything else remains visible to the shared exact verifier.
                fields.remove("@timestamp");
                fields.remove("@metadata");
                let reply = if remaining == 1 {
                    self.window = None;
                    let mut reply = vec![b'2', b'A'];
                    reply.extend_from_slice(&sequence.to_be_bytes());
                    reply
                } else {
                    self.window = Some((remaining - 1, expected + 1));
                    Vec::new()
                };
                Ok(Some(Frame {
                    consumed,
                    records: vec![record],
                    reply,
                }))
            }
            b'C' => bail!("compressed Lumberjack is disabled; set Filebeat compression_level to 0"),
            _ => bail!("unsupported Lumberjack frame type"),
        }
    }

    pub(in crate::delivery) fn complete(&self) -> bool {
        self.window.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fragments_preserve_window_state_and_only_complete_batches_acknowledge() {
        let window = [b'2', b'W', 0, 0, 0, 2];
        let event = |sequence: u32, value: Value| {
            let bytes = serde_json::to_vec(&value).unwrap();
            let mut frame = vec![b'2', b'J'];
            frame.extend_from_slice(&sequence.to_be_bytes());
            frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            frame.extend_from_slice(&bytes);
            frame
        };
        let mut decoder = Decoder::default();
        for end in 0..window.len() {
            assert!(decoder.decode(&window[..end]).unwrap().is_none());
            assert!(decoder.complete());
        }
        assert_eq!(decoder.decode(&window).unwrap().unwrap().consumed, 6);
        let first = event(
            1,
            json!({"@timestamp":"now","@metadata":{},"message":"one"}),
        );
        for end in 0..first.len() {
            assert!(decoder.decode(&first[..end]).unwrap().is_none());
            assert!(!decoder.complete());
        }
        let frame = decoder.decode(&first).unwrap().unwrap();
        assert_eq!(frame.records, [json!({"message":"one"})]);
        assert!(frame.reply.is_empty());
        assert!(!decoder.complete());
        let frame = decoder
            .decode(&event(2, json!({"message":"two","unexpected":"retained"})))
            .unwrap()
            .unwrap();
        assert_eq!(
            frame.records,
            [json!({"message":"two","unexpected":"retained"})]
        );
        assert_eq!(frame.reply, [b'2', b'A', 0, 0, 0, 2]);
        assert!(decoder.complete());

        assert!(decoder.decode(b"2C").is_err());
        assert!(decoder.decode(b"1J").is_err());
        assert!(decoder.decode(&event(1, json!({}))).is_err());
        decoder.decode(&window).unwrap();
        assert!(decoder.decode(&window).is_err());
        assert!(decoder.decode(&event(2, json!({}))).is_err());
        let mut oversized = vec![b'2', b'J', 0, 0, 0, 1];
        oversized.extend_from_slice(&((MAX_PAYLOAD + 1) as u32).to_be_bytes());
        assert!(decoder.decode(&oversized).is_err());
        assert!(!decoder.complete());
    }
}

//! Fluentd PackedForward with integer timestamps, no compression and no acks.

use std::io::Cursor;

use anyhow::{bail, ensure, Context, Result};
use rmpv::Value;
use serde_json::{Map, Number, Value as Json};

use super::Frame;

const MAX_FRAME: usize = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 16;
const MAX_ITEMS: usize = 4096;
const MAX_RECORDS: usize = 65_536;

fn bytes(input: &[u8], start: usize, length: usize) -> Result<Option<usize>> {
    let end = start
        .checked_add(length)
        .context("Forward length overflow")?;
    ensure!(end <= MAX_FRAME, "Forward value exceeds 4 MiB");
    Ok((input.len() >= end).then_some(end))
}

fn length(input: &[u8], start: usize, count: usize) -> Option<usize> {
    let data = input.get(start..start.checked_add(count)?)?;
    Some(
        data.iter()
            .fold(0_usize, |value, byte| value * 256 + usize::from(*byte)),
    )
}

// Validate declared sizes and nesting before generic deserialization. In
// particular, an array/map/string header cannot request an unbounded allocation.
fn scan(input: &[u8], start: usize, depth: usize, nodes: &mut usize) -> Result<Option<usize>> {
    ensure!(depth <= MAX_DEPTH, "Forward nesting exceeds limit");
    *nodes += 1;
    ensure!(*nodes <= MAX_RECORDS, "Forward value has too many nodes");
    let Some(&marker) = input.get(start) else {
        return Ok(None);
    };
    let mut position = start + 1;
    let mut children = 0;
    let payload = match marker {
        0x00..=0x7f | 0xc0 | 0xc2 | 0xc3 | 0xe0..=0xff => 0,
        0x80..=0x8f => {
            children = usize::from(marker & 0x0f) * 2;
            0
        }
        0x90..=0x9f => {
            children = usize::from(marker & 0x0f);
            0
        }
        0xa0..=0xbf => usize::from(marker & 0x1f),
        0xc4 | 0xc5 | 0xc6 | 0xc7 | 0xc8 | 0xc9 | 0xd9 | 0xda | 0xdb => {
            let count = match marker {
                0xc4 | 0xc7 | 0xd9 => 1,
                0xc5 | 0xc8 | 0xda => 2,
                _ => 4,
            };
            let Some(size) = length(input, position, count) else {
                return Ok(None);
            };
            position += count;
            size.checked_add(usize::from(matches!(marker, 0xc7..=0xc9)))
                .context("Forward size overflow")?
        }
        0xca | 0xce | 0xd2 => 4,
        0xcb | 0xcf | 0xd3 => 8,
        0xcc | 0xd0 => 1,
        0xcd | 0xd1 => 2,
        0xd4 => 2,
        0xd5 => 3,
        0xd6 => 5,
        0xd7 => 9,
        0xd8 => 17,
        0xdc..=0xdf => {
            let count = if matches!(marker, 0xdc | 0xde) { 2 } else { 4 };
            let Some(size) = length(input, position, count) else {
                return Ok(None);
            };
            ensure!(size <= MAX_ITEMS, "Forward container exceeds item limit");
            position += count;
            children = size * if matches!(marker, 0xde | 0xdf) { 2 } else { 1 };
            0
        }
        _ => bail!("invalid MessagePack marker"),
    };
    let Some(end) = bytes(input, position, payload)? else {
        return Ok(None);
    };
    position = end;
    for _ in 0..children {
        let Some(end) = scan(input, position, depth + 1, nodes)? else {
            return Ok(None);
        };
        position = end;
    }
    Ok(Some(position))
}

fn json(value: Value) -> Result<Json> {
    Ok(match value {
        Value::Nil => Json::Null,
        Value::Boolean(value) => Json::Bool(value),
        Value::Integer(value) => match value.as_i64() {
            Some(value) => Json::Number(value.into()),
            None => Json::Number(value.as_u64().context("invalid integer")?.into()),
        },
        Value::F32(value) => {
            Json::Number(Number::from_f64(f64::from(value)).context("nonfinite float")?)
        }
        Value::F64(value) => Json::Number(Number::from_f64(value).context("nonfinite float")?),
        Value::String(value) => Json::String(value.into_str().context("invalid UTF-8 record")?),
        Value::Array(values) => Json::Array(values.into_iter().map(json).collect::<Result<_>>()?),
        Value::Map(values) => {
            let mut object = Map::new();
            for (key, value) in values {
                let key = key.as_str().context("non-string record key")?.to_owned();
                ensure!(
                    object.insert(key, json(value)?).is_none(),
                    "duplicate record key"
                );
            }
            Json::Object(object)
        }
        _ => bail!("non-JSON application record value"),
    })
}

pub(in crate::delivery) fn decode(input: &[u8]) -> Result<Option<Frame>> {
    let Some(consumed) = scan(input, 0, 0, &mut 0)? else {
        return Ok(None);
    };
    let value =
        rmpv::decode::read_value_with_max_depth(&mut Cursor::new(&input[..consumed]), MAX_DEPTH)?;
    let outer = value.as_array().context("expected PackedForward array")?;
    ensure!(outer.len() == 3, "expected three PackedForward fields");
    let tag = outer[0].as_str().context("invalid Forward tag")?;
    ensure!(
        !tag.is_empty() && tag.len() <= 1024,
        "invalid Forward tag length"
    );
    let packed = match &outer[1] {
        Value::Binary(bytes) => bytes.as_slice(),
        Value::String(value) => value.as_bytes(),
        _ => bail!("expected packed Forward entries"),
    };
    let options = outer[2].as_map().context("expected Forward options")?;
    let mut expected = None;
    let mut compressed = None;
    for (key, value) in options {
        match key.as_str() {
            Some("size") => {
                ensure!(expected.is_none(), "duplicate Forward size");
                expected = Some(value.as_u64().context("invalid Forward size")?);
            }
            Some("compressed") => {
                ensure!(compressed.is_none(), "duplicate Forward compression");
                compressed = Some(value.as_str().context("invalid compression value")?);
            }
            _ => bail!("unsupported Forward option (acks and authentication are disabled)"),
        }
    }
    ensure!(
        compressed == Some("text"),
        "compressed Forward is unsupported"
    );
    let expected = expected.context("missing Forward size")?;
    ensure!(
        expected > 0 && expected <= MAX_RECORDS as u64,
        "Forward record count exceeds limit"
    );
    let mut records = Vec::new();
    let mut position = 0;
    while position < packed.len() {
        ensure!(
            records.len() < expected as usize,
            "Forward count exceeds advertised size"
        );
        let end = scan(packed, position, 0, &mut 0)?.context("truncated packed Forward entry")?;
        let entry = rmpv::decode::read_value_with_max_depth(
            &mut Cursor::new(&packed[position..end]),
            MAX_DEPTH,
        )?;
        let mut entry = match entry {
            Value::Array(values) if values.len() == 2 => values,
            _ => bail!("expected [integer timestamp, record]"),
        };
        ensure!(
            entry[0].is_i64() || entry[0].is_u64(),
            "Forward timestamp must be an integer"
        );
        let record = json(entry.pop().context("missing Forward record")?)?;
        ensure!(record.is_object(), "Forward record must be an object");
        records.push(record);
        position = end;
    }
    ensure!(
        records.len() as u64 == expected,
        "Forward size does not match records"
    );
    Ok(Some(Frame {
        consumed,
        records,
        reply: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_fragmented_forward_and_rejects_invalid_or_unbounded_frames() {
        let mut packed = Vec::new();
        rmpv::encode::write_value(
            &mut packed,
            &Value::Array(vec![
                1.into(),
                Value::Map(vec![("message".into(), "record".into())]),
            ]),
        )
        .unwrap();
        let packet = |size: u64, compression: &str| {
            let mut bytes = Vec::new();
            let value = Value::Array(vec![
                "records".into(),
                Value::Binary(packed.clone()),
                Value::Map(vec![
                    ("size".into(), size.into()),
                    ("compressed".into(), compression.into()),
                ]),
            ]);
            rmpv::encode::write_value(&mut bytes, &value).unwrap();
            bytes
        };
        let bytes = packet(1, "text");
        for length in 0..bytes.len() {
            assert!(decode(&bytes[..length]).unwrap().is_none());
        }
        let frame = decode(&bytes).unwrap().unwrap();
        assert_eq!(frame.consumed, bytes.len());
        assert_eq!(frame.records, [serde_json::json!({"message":"record"})]);
        assert!(frame.reply.is_empty());
        assert!(decode(&packet(2, "text")).is_err());
        assert!(decode(&packet(1, "gzip")).is_err());
        assert!(decode(&[0xdd, 0xff, 0xff, 0xff, 0xff]).is_err());
        assert!(decode(&[0xdb, 0xff, 0xff, 0xff, 0xff]).is_err());
        let mut deep = vec![0x91; MAX_DEPTH + 2];
        deep.push(0xc0);
        assert!(decode(&deep).is_err());
    }
}

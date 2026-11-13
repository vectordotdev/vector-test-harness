use anyhow::{Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufWriter, Write},
    ops::Range,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

struct TextCache {
    bytes: Vec<u8>,
    stride: usize,
    user: Range<usize>,
    path: Range<usize>,
}

#[derive(Clone)]
pub(super) struct Fixture {
    pub events: usize,
    pub warmup: usize,
    pub token: String,
    text_cache: Option<Arc<TextCache>>,
}

impl Fixture {
    pub fn new(events: usize, warmup: usize, token: String) -> Self {
        Self {
            events,
            warmup,
            token,
            text_cache: None,
        }
    }

    pub fn total(&self) -> usize {
        1 + self.warmup + self.events
    }

    fn path(&self, id: usize) -> String {
        let payload = Sha256::digest(format!("{}:{id:010}", self.token).as_bytes());
        format!("/vth/{}/{id:010}/{payload:x}", self.token)
    }

    pub fn record(&self, id: usize) -> Vec<u8> {
        format!(
            "192.0.2.1 - vth{id:010} 123 [12/Nov/2026:00:00:00 +0000] \"GET {}\" 200 456\n",
            self.path(id)
        )
        .into_bytes()
    }

    /// Prepare exact expected text before timing, sharing one allocation across clones.
    pub fn prepare_text(&mut self, mut check: impl FnMut() -> Result<()>) -> Result<()> {
        check()?;
        if self.text_cache.is_some() {
            return Ok(());
        }
        let total = self
            .events
            .checked_add(self.warmup)
            .and_then(|count| count.checked_add(1))
            .context("fixture record count overflow")?;
        let first = self.record(0);
        let stride = first.len();
        let field_range = |expected: &[u8]| -> Result<Range<usize>> {
            anyhow::ensure!(!expected.is_empty(), "expected field must not be empty");
            let start = first
                .windows(expected.len())
                .position(|bytes| bytes == expected)
                .context("expected field not found in canonical text")?;
            let end = start
                .checked_add(expected.len())
                .context("expected field range overflow")?;
            anyhow::ensure!(end < stride, "expected field exceeds canonical text");
            Ok(start..end)
        };
        let user = field_range(format!("vth{:010}", 0).as_bytes())?;
        let path = field_range(self.path(0).as_bytes())?;
        anyhow::ensure!(user.end <= path.start, "expected field ranges overlap");
        let capacity = total
            .checked_mul(stride)
            .context("expected text cache size overflow")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .context("allocate expected text cache")?;
        for id in 0..total {
            if id % 1024 == 0 {
                check()?;
            }
            let record = self.record(id);
            anyhow::ensure!(
                record.len() == stride && record.ends_with(b"\n"),
                "expected text records must have a constant length and newline delimiter"
            );
            bytes.extend_from_slice(&record);
        }
        check()?;
        self.text_cache = Some(Arc::new(TextCache {
            bytes,
            stride,
            user,
            path,
        }));
        Ok(())
    }

    fn cached_record(&self, id: usize) -> Option<&[u8]> {
        let cache = self.text_cache.as_ref()?;
        let start = id.checked_mul(cache.stride)?;
        let end = start.checked_add(cache.stride)?;
        cache.bytes.get(start..end)
    }

    pub fn expected_text(&self, id: usize) -> Option<&[u8]> {
        self.cached_record(id)?.strip_suffix(b"\n")
    }

    pub fn cache_bytes(&self) -> usize {
        self.text_cache
            .as_ref()
            .map_or(0, |cache| cache.bytes.len())
    }

    pub fn parsed(&self, id: usize) -> Value {
        json!({
            "host": "192.0.2.1",
            "user": format!("vth{id:010}"),
            "bytes_in": "123",
            "timestamp": "12/Nov/2026:00:00:00 +0000",
            "method": "GET",
            "path": self.path(id),
            "status": "200",
            "bytes_out": "456",
        })
    }

    pub fn matches_parsed(&self, id: usize, value: &Value) -> bool {
        let Some(cache) = &self.text_cache else {
            return *value == self.parsed(id);
        };
        let Some(record) = self.cached_record(id) else {
            return false;
        };
        let Some(fields) = value.as_object() else {
            return false;
        };
        fields.len() == 8
            && [
                ("host", "192.0.2.1"),
                ("bytes_in", "123"),
                ("timestamp", "12/Nov/2026:00:00:00 +0000"),
                ("method", "GET"),
                ("status", "200"),
                ("bytes_out", "456"),
            ]
            .iter()
            .all(|(key, expected)| fields.get(*key).and_then(Value::as_str) == Some(*expected))
            && fields
                .get("user")
                .and_then(Value::as_str)
                .map(str::as_bytes)
                == Some(&record[cache.user.clone()])
            && fields
                .get("path")
                .and_then(Value::as_str)
                .map(str::as_bytes)
                == Some(&record[cache.path.clone()])
    }

    pub fn write(
        &self,
        path: &Path,
        range: Range<usize>,
        mut check: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        anyhow::ensure!(
            range.start <= range.end && range.end <= self.total(),
            "fixture range is out of bounds"
        );
        let mut output = BufWriter::new(
            File::create(path).with_context(|| format!("create fixture {}", path.display()))?,
        );
        for id in range {
            if id % 1024 == 0 {
                check()?;
            }
            match self.cached_record(id) {
                Some(record) => output.write_all(record)?,
                None => output.write_all(&self.record(id))?,
            }
        }
        output.flush().context("flush fixture")
    }
}

#[derive(Clone, Copy)]
pub(super) enum Format {
    Text,
    ParsedJson,
    MessageJson,
}

pub(super) struct Verifier {
    fixture: Fixture,
    format: Format,
    seen: Vec<bool>,
    unique: usize,
    duplicates: usize,
    unknown: usize,
    corrupt: usize,
    malformed: usize,
    examples: Vec<String>,
    measurement_start: Option<Instant>,
    measurement_elapsed: Option<Duration>,
}

impl Verifier {
    pub fn new(fixture: Fixture, format: Format) -> Self {
        Self {
            seen: vec![false; fixture.total()],
            fixture,
            format,
            unique: 0,
            duplicates: 0,
            unknown: 0,
            corrupt: 0,
            malformed: 0,
            examples: Vec::new(),
            measurement_start: None,
            measurement_elapsed: None,
        }
    }

    pub(super) fn start_measurement(&mut self) -> Result<()> {
        anyhow::ensure!(
            self.measurement_start.is_none(),
            "measurement already started"
        );
        anyhow::ensure!(self.fixture.events != 0, "measurement requires events");
        anyhow::ensure!(
            !self.has_errors(),
            "verification has errors before measurement"
        );
        let prefix = 1 + self.fixture.warmup;
        anyhow::ensure!(
            self.unique == prefix && self.seen[..prefix].iter().all(|seen| *seen),
            "measurement requires exactly the verified canary and warmup prefix"
        );
        self.measurement_start = Some(Instant::now());
        Ok(())
    }

    pub(super) fn measurement_seconds(&self) -> Option<f64> {
        if self.measurement_start.is_none() || !self.passed() {
            return None;
        }
        self.measurement_elapsed
            .map(|elapsed| elapsed.as_secs_f64())
    }

    pub fn record_line(&mut self, bytes: &[u8]) {
        match self.format {
            // A text frame may include its single protocol newline delimiter.
            Format::Text => self.record_text(bytes.strip_suffix(b"\n").unwrap_or(bytes)),
            Format::ParsedJson | Format::MessageJson => {
                match serde_json::from_slice::<Value>(bytes) {
                    Ok(value) => self.record_json(&value),
                    Err(error) => self.malformed(&format!("invalid JSON: {error}")),
                }
            }
        }
    }

    pub fn record_json(&mut self, value: &Value) {
        match self.format {
            Format::ParsedJson => {
                let Some(user) = value.get("user").and_then(Value::as_str) else {
                    self.malformed("parsed event has no string user field");
                    return;
                };
                let Some(id) = self.identify(user.as_bytes()) else {
                    return;
                };
                self.verify(id, self.fixture.matches_parsed(id, value));
            }
            Format::MessageJson => {
                if value.as_object().is_none_or(|fields| fields.len() != 1) {
                    self.malformed("HTTP event must contain only the message field");
                    return;
                }
                match value.get("message").and_then(Value::as_str) {
                    Some(message) => self.record_text(message.as_bytes()),
                    None => self.malformed("event has no string message field"),
                }
            }
            Format::Text => self.malformed("received JSON for a text fixture"),
        }
    }

    fn record_text(&mut self, bytes: &[u8]) {
        let Some(user) = bytes
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty())
            .nth(2)
        else {
            self.malformed("text event has no user field");
            return;
        };
        let Some(id) = self.identify(user) else {
            return;
        };
        let matches = match self.fixture.expected_text(id) {
            Some(expected) => bytes == expected,
            None => {
                let expected = self.fixture.record(id);
                bytes == &expected[..expected.len() - 1]
            }
        };
        self.verify(id, matches);
    }

    fn identify(&mut self, user: &[u8]) -> Option<usize> {
        let id = user
            .strip_prefix(b"vth")
            .filter(|digits| digits.len() == 10 && digits.iter().all(u8::is_ascii_digit))
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| digits.parse::<usize>().ok());
        match id {
            Some(id) if id < self.seen.len() => Some(id),
            _ => {
                self.unknown += 1;
                self.example("unknown or out-of-range event ID");
                None
            }
        }
    }

    fn verify(&mut self, id: usize, matches: bool) {
        if !matches {
            self.corrupt += 1;
            self.example(&format!("event {id:010} differs from its fixture"));
        } else if self.seen[id] {
            self.duplicates += 1;
            self.example(&format!("duplicate event {id:010}"));
        } else {
            self.seen[id] = true;
            self.unique += 1;
            if self.unique == self.fixture.total() && self.measurement_elapsed.is_none() {
                if let Some(start) = self.measurement_start {
                    self.measurement_elapsed = Some(Instant::now().duration_since(start));
                }
            }
        }
    }

    pub fn malformed(&mut self, reason: &str) {
        self.malformed += 1;
        self.example(reason);
    }

    fn example(&mut self, reason: &str) {
        if self.examples.len() < 8 {
            self.examples.push(reason.chars().take(160).collect());
        }
    }

    pub fn unique(&self) -> usize {
        self.unique
    }

    pub fn has_errors(&self) -> bool {
        self.duplicates != 0 || self.unknown != 0 || self.corrupt != 0 || self.malformed != 0
    }

    pub fn passed(&self) -> bool {
        self.unique == self.fixture.total() && !self.has_errors()
    }

    pub fn report(&self) -> Value {
        json!({
            "passed": self.passed(),
            "expected": self.fixture.total(),
            "unique": self.unique,
            "missing": self.fixture.total() - self.unique,
            "duplicates": self.duplicates,
            "unknown": self.unknown,
            "corrupt": self.corrupt,
            "malformed": self.malformed,
            "examples": self.examples,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_complete_payloads_and_accounts_for_delivery_anomalies() {
        fn deliver(verifier: &mut Verifier, id: usize, corrupt: bool) {
            let mut record = verifier.fixture.record(id);
            if corrupt {
                let payload = record.iter().rposition(|byte| *byte == b'/').unwrap() + 1;
                record[payload] = if record[payload] == b'a' { b'b' } else { b'a' };
            }
            match verifier.format {
                Format::Text => verifier.record_line(&record),
                Format::ParsedJson => {
                    let mut parsed = verifier.fixture.parsed(id);
                    if corrupt {
                        parsed["path"] = json!(format!("{}x", parsed["path"].as_str().unwrap()));
                    }
                    verifier.record_line(&serde_json::to_vec(&parsed).unwrap());
                }
                Format::MessageJson => verifier.record_json(&json!({
                    "message": std::str::from_utf8(&record[..record.len() - 1]).unwrap(),
                })),
            }
        }

        let mut canceled = Fixture::new(1024, 0, "canceled-cache".to_owned());
        let mut checks = 0;
        assert!(canceled
            .prepare_text(|| {
                checks += 1;
                anyhow::ensure!(checks < 3, "cancel during cache preparation");
                Ok(())
            })
            .is_err());
        assert_eq!(canceled.cache_bytes(), 0);

        for (format, cached) in [
            (Format::Text, false),
            (Format::Text, true),
            (Format::ParsedJson, false),
            (Format::ParsedJson, true),
            (Format::MessageJson, false),
            (Format::MessageJson, true),
        ] {
            let mut fixture = Fixture::new(2, 1, "test-run".to_owned());
            if cached {
                fixture.prepare_text(|| Ok(())).unwrap();
                assert_eq!(
                    fixture.cache_bytes(),
                    fixture.total() * fixture.record(0).len()
                );
                let cloned = fixture.clone();
                assert!(Arc::ptr_eq(
                    fixture.text_cache.as_ref().unwrap(),
                    cloned.text_cache.as_ref().unwrap()
                ));
                for id in 0..fixture.total() {
                    let expected = fixture.record(id);
                    assert_eq!(
                        fixture.expected_text(id),
                        Some(&expected[..expected.len() - 1])
                    );
                    assert!(fixture.matches_parsed(id, &fixture.parsed(id)));
                }
                assert_eq!(fixture.expected_text(fixture.total()), None);
            }
            let mut verifier = Verifier::new(fixture.clone(), format);
            for id in (0..fixture.total() - 1).rev() {
                deliver(&mut verifier, id, false);
            }
            assert!(!verifier.passed());
            assert_eq!(verifier.report()["missing"], 1);
            deliver(&mut verifier, fixture.total() - 1, false);
            assert!(verifier.passed());
            deliver(&mut verifier, 0, false);
            assert!(!verifier.passed());
            assert_eq!(verifier.report()["duplicates"], 1);

            let mut verifier = Verifier::new(fixture.clone(), format);
            deliver(&mut verifier, 0, true);
            assert_eq!(verifier.unique(), 0);
            assert_eq!(verifier.report()["corrupt"], 1);
            deliver(&mut verifier, 0, false);
            assert_eq!(verifier.unique(), 1);
            assert_eq!(verifier.report()["duplicates"], 0);
            deliver(&mut verifier, fixture.total(), false);
            assert_eq!(verifier.report()["unknown"], 1);
            assert_eq!(verifier.unique(), 1);
            assert!(verifier.has_errors());

            if matches!(format, Format::ParsedJson) {
                for field in [
                    "host",
                    "user",
                    "bytes_in",
                    "timestamp",
                    "method",
                    "path",
                    "status",
                    "bytes_out",
                ] {
                    let mut wrong_type = fixture.parsed(0);
                    wrong_type[field] = json!(123);
                    assert!(!fixture.matches_parsed(0, &wrong_type), "{field}");
                    let mut wrong_value = fixture.parsed(0);
                    wrong_value[field] = json!("unexpected");
                    assert!(!fixture.matches_parsed(0, &wrong_value), "{field}");
                }
                let mut extra = fixture.parsed(0);
                extra["extra"] = json!("unexpected");
                let mut verifier = Verifier::new(fixture.clone(), format);
                verifier.record_json(&extra);
                assert_eq!(verifier.report()["corrupt"], 1);
                assert_eq!(verifier.unique(), 0);
            }

            if matches!(format, Format::MessageJson) {
                let mut verifier = Verifier::new(fixture.clone(), format);
                verifier.record_json(&json!({
                    "message": String::from_utf8(fixture.record(0)).unwrap(),
                }));
                assert_eq!(verifier.report()["corrupt"], 1);
                assert_eq!(verifier.unique(), 0);
                let mut verifier = Verifier::new(fixture.clone(), format);
                verifier.record_json(&json!({
                    "message": String::from_utf8(fixture.record(0)).unwrap().trim_end(),
                    "extra": "unexpected",
                }));
                assert_eq!(verifier.report()["malformed"], 1);
                assert_eq!(verifier.unique(), 0);
            }
        }
    }

    #[test]
    fn measurement_requires_exact_warmup_and_freezes_valid_completion() {
        let fixture = Fixture::new(2, 1, "timed-run".to_owned());
        let mut verifier = Verifier::new(fixture.clone(), Format::Text);
        assert!(verifier.start_measurement().is_err());
        for id in [0, 2] {
            verifier.record_line(&fixture.record(id));
        }
        assert!(verifier.start_measurement().is_err());
        verifier.record_line(&fixture.record(1));
        assert!(verifier.start_measurement().is_err());

        let mut verifier = Verifier::new(fixture.clone(), Format::Text);
        for id in 0..=fixture.warmup {
            verifier.record_line(&fixture.record(id));
        }
        verifier.start_measurement().unwrap();
        let start = verifier.measurement_start;
        assert!(verifier.start_measurement().is_err());
        assert_eq!(verifier.measurement_start, start);
        assert_eq!(verifier.measurement_seconds(), None);
        verifier.record_line(&fixture.record(2));
        assert_eq!(verifier.measurement_seconds(), None);
        verifier.record_line(&fixture.record(3));
        let elapsed = verifier.measurement_seconds().unwrap();
        let completed = verifier.measurement_elapsed;
        for _ in 0..3 {
            assert_eq!(verifier.report()["passed"], true);
            assert_eq!(verifier.measurement_seconds(), Some(elapsed));
            assert_eq!(verifier.measurement_elapsed, completed);
        }
        verifier.record_line(&fixture.record(3));
        assert_eq!(verifier.measurement_seconds(), None);
        assert_eq!(verifier.measurement_elapsed, completed);

        let mut verifier = Verifier::new(fixture.clone(), Format::Text);
        for id in 0..=fixture.warmup {
            verifier.record_line(&fixture.record(id));
        }
        verifier.malformed("pre-measurement error");
        assert!(verifier.start_measurement().is_err());
        let fixture = Fixture::new(0, 0, "no-events".to_owned());
        let mut verifier = Verifier::new(fixture.clone(), Format::Text);
        verifier.record_line(&fixture.record(0));
        assert!(verifier.start_measurement().is_err());
    }
}

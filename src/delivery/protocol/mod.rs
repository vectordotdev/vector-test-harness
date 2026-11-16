//! Native wire decoding. Application-record verification remains in `fixture`.

pub(super) mod beats;
pub(super) mod elastic;
pub(super) mod forward;

pub(super) struct Frame {
    pub consumed: usize,
    pub records: Vec<serde_json::Value>,
    pub reply: Vec<u8>,
}

use std::io::Write;

use anyhow::Context;
use bincode::{deserialize, serialize, Result};
use serde::{Deserialize, Serialize};
use tokio_util::{
    bytes::BufMut,
    codec::{Decoder, Encoder},
};

/// The structure of the response is as follows:
/// - The first byte represents the version flag.
/// - The next 4 bytes represent the request ID.
/// - The next 8 bytes represent the timestamp.
/// - The next 2 bytes represent the response code.
/// - The remaining bytes represent the message, ending with a `\r\n` terminator.
///
/// # Fields
///
/// - `version`: A `u8` representing the version of the request protocol.
/// - `request_id`: A `u32` representing the request to which we are responding.
/// - `timestamp`: A `u64` representing the Unix timestamp when the response was generated.
/// - `code`: A `u16` representing the response code.
/// - `message`: A `String` containing the message.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Response {
    pub version: u8,
    pub request_id: u32,
    pub timestamp: u64,
    pub code: u16,
    pub origin_length: u8,
    pub origin: String,

    // @FEATURE: Should message take a format which can be parsed
    // into a different AST node? So that it can be displayed differently
    // in the client - i.e. nick change messages could be in grey
    pub message: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub enum ResponseMessage {
    #[default]
    Pong,
    Welcome {
        network: String,
        nick: String,
    },
}

impl Response {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = serialize(self)?;
        bytes.extend(b"\r\n");

        Ok(bytes)
    }

    pub fn decode(encoded: &[u8]) -> Result<Response> {
        deserialize(encoded)
    }

    pub fn write_to(&self, stream: &mut impl Write) -> anyhow::Result<()> {
        stream
            .write_all(&self.encode()?[..])
            .context("ERROR: Failed to write to stream")
    }
}

impl Decoder for Response {
    type Item = Response;
    type Error = anyhow::Error;

    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> anyhow::Result<Option<Response>> {
        if let Some(pos) = src.windows(2).position(|w| w == b"\r\n") {
            let mut buf = src.split_to(pos + 2).freeze();
            buf.truncate(pos);

            let response = Response::decode(&buf[..])?;

            return Ok(Some(response));
        }

        Ok(None)
    }
}

impl Encoder<Response> for Response {
    type Error = anyhow::Error;

    fn encode(
        &mut self,
        item: Response,
        dst: &mut tokio_util::bytes::BytesMut,
    ) -> anyhow::Result<()> {
        let bytes = item.encode()?;
        dst.reserve(bytes.len());
        dst.put(&bytes[..]);

        Ok(())
    }
}

#[derive(Default)]
pub struct ResponseBuilder {
    request_id: u32,
    code: u16,
    origin: String,
    message: String,
}

impl ResponseBuilder {
    pub fn new(code: u16, message: String) -> Self {
        Self {
            code,
            message,
            ..Self::default()
        }
    }

    pub fn with_request_id(mut self, request_id: u32) -> Self {
        self.request_id = request_id;

        self
    }

    pub fn with_origin(mut self, origin: String) -> Self {
        self.origin = origin;

        self
    }

    pub fn build(self) -> Response {
        Response {
            version: 1,
            request_id: self.request_id,
            timestamp: u64::try_from(chrono::Utc::now().timestamp())
                .expect("ERROR: Timestamp exceeds u64::MAX"),
            code: self.code,
            origin_length: u8::try_from(self.origin.len()).expect("ERROR: Origin too long"),
            origin: self.origin,
            message: self.message,
        }
    }
}

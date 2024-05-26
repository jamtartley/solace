use std::io::Write;

use anyhow::Context;
use bincode::{deserialize, serialize, Result};
use serde::{Deserialize, Serialize};
use tokio_util::{
    bytes::BufMut,
    codec::{Decoder, Encoder},
};

/// The structure of the request is as follows:
/// - The first byte represents the version flag.
/// - The next 4 bytes represent the request ID.
/// - The remaining bytes represent the message, ending with a `\r\n` terminator.
///
/// # Fields
///
/// - `version`: A `u8` representing the version of the request protocol.
/// - `id`: A `u32` representing a unique identifier for the request.
/// - `message`: A `ReqeustMessage` containing the message.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Request {
    pub version: u8,
    pub id: u32,
    pub message: RequestMessage,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub enum RequestMessage {
    #[default]
    Ping,
    Message(String),
    NewTopic(String),
    NewNick(String),
    WhoIs(String),
    Disconnect,
}

impl Request {
    pub fn new(id: u32, message: RequestMessage) -> Self {
        Self {
            version: 1,
            id,
            message: message.clone(),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = serialize(self)?;
        bytes.extend(b"\r\n");

        Ok(bytes)
    }

    pub fn decode(encoded: &[u8]) -> Result<Request> {
        deserialize(encoded)
    }

    pub fn write_to(&self, stream: &mut impl Write) -> anyhow::Result<()> {
        stream
            .write_all(&self.encode()?[..])
            .context("ERROR: Failed to write to stream")
    }
}

impl Decoder for Request {
    type Item = Request;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut tokio_util::bytes::BytesMut) -> anyhow::Result<Option<Request>> {
        if let Some(pos) = src.windows(2).position(|w| w == b"\r\n") {
            let mut buf = src.split_to(pos + 2).freeze();
            buf.truncate(pos);

            if buf.len() < 5 {
                return Err(anyhow::anyhow!("ERROR: Request is too short"));
            }

            let request = Request::decode(&buf[..])?;

            return Ok(Some(request));
        }

        Ok(None)
    }
}

impl Encoder<Request> for Request {
    type Error = anyhow::Error;

    fn encode(
        &mut self,
        item: Request,
        dst: &mut tokio_util::bytes::BytesMut,
    ) -> anyhow::Result<()> {
        let bytes = item.encode()?;
        dst.reserve(bytes.len());
        dst.put(&bytes[..]);

        Ok(())
    }
}

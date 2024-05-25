use std::{io::Write, net::TcpStream};

use anyhow::Context;
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
/// - `message`: A `String` containing the message.
#[derive(Debug, Default)]
pub struct Request {
    pub version: u8,
    pub id: u32,
    pub message: String,
}

impl Request {
    pub fn new(message: String) -> Self {
        Self {
            version: 1,
            id: rand::random(),
            message: message.clone(),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.version];
        bytes.extend(self.id.to_be_bytes());
        bytes.extend(self.message.as_bytes());
        bytes.extend(b"\r\n");

        bytes
    }

    pub fn write_to(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
        stream
            .write_all(&self.as_bytes())
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

            let version = buf[0];
            let id = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
            let message = String::from_utf8(buf[5..].to_vec())?;

            return Ok(Some(Request {
                version,
                id,
                message,
            }));
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
        let bytes = item.as_bytes();
        dst.reserve(bytes.len());
        dst.put(&bytes[..]);

        Ok(())
    }
}

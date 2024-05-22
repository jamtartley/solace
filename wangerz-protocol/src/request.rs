use std::{io::Write, net::TcpStream};

use anyhow::Context;

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

impl TryFrom<&mut Vec<u8>> for Request {
    type Error = anyhow::Error;

    fn try_from(buf: &mut Vec<u8>) -> Result<Self, anyhow::Error> {
        if let Some(pos) = buf.windows(2).position(|w| w == b"\r\n") {
            let parseable = buf.drain(..pos).collect::<Vec<u8>>();
            buf.drain(0..2); // pop out the \r\n

            if parseable.len() < 5 {
                return Err(anyhow::anyhow!("ERROR: Request is too short"));
            }

            let version = parseable[0];
            let id = u32::from_be_bytes([parseable[1], parseable[2], parseable[3], parseable[4]]);
            let message = String::from_utf8(parseable[5..].to_owned())?;

            return Ok(Self {
                id,
                message,
                version,
            });
        }

        Err(anyhow::anyhow!("ERROR: Invalid reqeust"))
    }
}

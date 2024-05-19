use std::{io::Write, net::TcpStream, sync::Arc};

use anyhow::Context;

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
#[derive(Debug, Default)]
pub struct Response {
    pub version: u8,
    pub request_id: u32,
    pub timestamp: u64,
    pub code: u16,
    pub origin_length: u8,
    pub origin: String,
    pub message: String,
}

impl Response {
    pub fn new(request_id: u32, code: u16, message: String) -> Self {
        let timestamp = chrono::Utc::now().timestamp() as u64;
        let origin = "".to_owned();

        Self {
            version: 1,
            request_id,
            timestamp,
            code,
            origin_length: origin.len() as u8,
            origin,
            message: message.clone(),
        }
    }

    pub fn with_origin(mut self, origin: String) -> Self {
        self.origin_length = origin.len() as u8;
        self.origin = origin;

        self
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.version];
        bytes.extend(&self.request_id.to_be_bytes());
        bytes.extend(&self.timestamp.to_be_bytes());
        bytes.extend(&self.code.to_be_bytes());
        bytes.extend(&self.origin_length.to_be_bytes());
        bytes.extend(self.origin.as_bytes());
        bytes.extend(self.message.as_bytes());
        bytes.extend(b"\r\n");

        bytes
    }

    // @CLEANUP: Align signature with Request::write_to()
    pub fn write_to(&self, stream: &Arc<TcpStream>) -> anyhow::Result<()> {
        stream
            .clone()
            .as_ref()
            .write_all(&self.as_bytes())
            .context("ERROR: Failed to write to stream")
    }
}

impl TryFrom<Vec<u8>> for Response {
    type Error = anyhow::Error;

    fn try_from(mut buf: Vec<u8>) -> Result<Self, anyhow::Error> {
        if let Some(pos) = buf.windows(2).position(|w| w == b"\r\n") {
            let parseable = buf.drain(..pos).collect::<Vec<u8>>();

            if parseable.len() < 13 {
                return Err(anyhow::anyhow!("Invalid response: too short"));
            }

            let version = parseable[0];
            // @CLEANUP: Assumption of big-endian byte order
            let request_id =
                u32::from_be_bytes([parseable[1], parseable[2], parseable[3], parseable[4]]);
            let timestamp = u64::from_be_bytes([
                parseable[5],
                parseable[6],
                parseable[7],
                parseable[8],
                parseable[9],
                parseable[10],
                parseable[11],
                parseable[12],
            ]);
            let code = u16::from_be_bytes([parseable[13], parseable[14]]);
            let origin_length = u8::from_be_bytes([parseable[15]]);
            let origin_end = 16 + origin_length;
            let origin = String::from_utf8(parseable[16..origin_end as usize].to_vec())?;
            let message = String::from_utf8(parseable[origin_end as usize..].to_vec())?;

            return Ok(Self {
                version,
                request_id,
                timestamp,
                code,
                origin,
                origin_length,
                message,
            });
        }

        Err(anyhow::anyhow!("Invalid response"))
    }
}

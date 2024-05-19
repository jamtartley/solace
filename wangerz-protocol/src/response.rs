use std::{
    io::Write,
    net::TcpStream,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

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
    pub message: String,
}

impl Response {
    pub fn new(request_id: u32, code: u16, message: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            version: 1,
            request_id,
            timestamp,
            code,
            message: message.clone(),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.version];
        bytes.extend(&self.request_id.to_be_bytes());
        bytes.extend(&self.timestamp.to_be_bytes());
        bytes.extend(&self.code.to_be_bytes());
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
            let message = String::from_utf8(parseable[15..].to_owned())?;

            return Ok(Self {
                version,
                request_id,
                timestamp,
                code,
                message,
            });
        }

        Err(anyhow::anyhow!("Invalid response"))
    }
}

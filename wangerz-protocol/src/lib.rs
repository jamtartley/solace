#![allow(dead_code)]

/// The structure of the request is as follows:
/// - The first byte represents the version flag.
/// - The remaining bytes represent the message, ending with a `\r\n` terminator.
///
/// # Fields
///
/// - `version`: A `u8` representing the version of the request protocol.
/// - `message`: A `String` containing the message.
#[derive(Debug, Default)]
pub struct Request {
    pub version: u8,
    pub message: String,
}

pub struct RequestBuilder {
    message: String,
}

impl RequestBuilder {
    pub fn new() -> Self {
        Self {
            message: String::default(),
        }
    }

    pub fn with_message(&mut self, message: String) -> &Self {
        self.message = message;

        self
    }

    pub fn build(&self) -> Request {
        Request {
            version: 1,
            message: self.message.clone(),
        }
    }
}

impl Request {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.version];
        bytes.extend(self.message.as_bytes());
        bytes.extend(b"\r\n");

        bytes
    }
}

impl TryFrom<Vec<u8>> for Request {
    type Error = anyhow::Error;

    fn try_from(mut buf: Vec<u8>) -> Result<Self, anyhow::Error> {
        if let Some(pos) = buf.windows(2).position(|w| w == b"\r\n") {
            let parseable = buf.drain(..pos).collect::<Vec<u8>>();

            if parseable.is_empty() {
                // @FIXME: What should happen with empty request?
                return Ok(Request::default());
            }

            let version = parseable[0];
            let message = String::from_utf8(parseable[1..].to_owned())?;

            return Ok(Self { version, message });
        }

        Err(anyhow::anyhow!("Invalid request"))
    }
}

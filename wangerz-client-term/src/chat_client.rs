use std::{io::Write, net::TcpStream};

#[derive(Default)]
pub struct ChatClientBuilder<'a> {
    ip: &'a str,
    port: usize,
}

impl<'a> ChatClientBuilder<'a> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_ip(mut self, ip: &'a str) -> Self {
        self.ip = ip;

        self
    }

    pub(crate) fn with_port(mut self, port: usize) -> Self {
        self.port = port;

        self
    }

    pub(crate) fn connect(self) -> anyhow::Result<ChatClient> {
        let conn = format!("{}:{}", self.ip, self.port);
        let stream = TcpStream::connect(conn).and_then(|stream| {
            stream.set_nonblocking(true)?;

            Ok(Some(stream))
        })?;

        Ok(ChatClient {
            stream,
            should_quit: false,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChatClient {
    stream: Option<TcpStream>,
    pub(crate) should_quit: bool,
}

impl ChatClient {
    pub(crate) fn write(&mut self, value: String) -> anyhow::Result<()> {
        if let Some(tcp_stream) = self.stream.as_mut() {
            tcp_stream.write(value.as_bytes())?;
        }

        Ok(())
    }
}

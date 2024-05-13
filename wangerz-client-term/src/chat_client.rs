use std::{
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    str,
};

use crossterm::style;

use crate::Renderable;

#[derive(Debug, Default)]
pub(crate) struct ChatHistory {
    pub(crate) entries: Vec<String>,
}

impl Renderable for ChatHistory {
    fn render_into(&self, buf: &mut crate::RenderBuffer, rect: &crate::Rect) {
        let height = rect.height as usize;

        for (i, entry) in self.entries.iter().rev().take(height).enumerate() {
            for (j, ch) in entry.chars().enumerate() {
                let x = rect.x + j as u16;
                let y = rect.y + rect.height - 1 - i as u16;

                buf.put_at(
                    x,
                    y,
                    ch,
                    style::Color::White,
                    style::Color::Reset,
                    crate::CellStyle::Normal,
                );
            }
        }
    }
}

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
            history: ChatHistory::default(),
            stream,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChatClient {
    stream: Option<TcpStream>,
    pub(crate) history: ChatHistory,
}

impl ChatClient {
    pub(crate) fn write(&mut self, value: String) -> anyhow::Result<()> {
        if let Some(tcp_stream) = self.stream.as_mut() {
            tcp_stream.write_all(value.as_bytes())?;
        }

        Ok(())
    }

    pub(crate) fn read(&mut self) -> anyhow::Result<()> {
        let mut buf = vec![0; 1024];

        if let Some(tcp_stream) = &mut self.stream {
            match tcp_stream.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let bytes = buf[0..n]
                        .iter()
                        .filter(|&x| *x >= 32)
                        .cloned()
                        .collect::<Vec<u8>>();

                    if let Ok(message) = str::from_utf8(&bytes) {
                        if !message.is_empty() {
                            self.history.entries.push(message.to_string());
                        }
                    }
                }
                Err(e) if e.kind() != ErrorKind::WouldBlock => {
                    self.stream = None;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

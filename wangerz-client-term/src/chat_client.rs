use std::{
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    str,
};

use crossterm::style;

use crate::Renderable;

#[derive(Debug, Default)]
pub(crate) struct ChatHistory {
    pub(crate) entries: Vec<(String, style::Color)>,
}

impl Renderable for ChatHistory {
    fn render_into(&self, buf: &mut crate::RenderBuffer, rect: &crate::Rect) {
        let height = rect.height as usize;

        for (i, entry) in self.entries.iter().rev().take(height).enumerate() {
            for (j, ch) in entry.0.chars().enumerate() {
                let x = rect.x + j as u16;
                let y = rect.y + rect.height - 1 - i as u16;

                buf.put_at(
                    x,
                    y,
                    ch,
                    entry.1,
                    style::Color::Reset,
                    crate::CellStyle::Normal,
                );
            }
        }
    }
}

impl ChatHistory {
    pub(crate) fn message(&mut self, msg: impl Into<String>) {
        self.entries.push((msg.into(), style::Color::White));
    }

    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        self.entries.push((msg.into(), style::Color::Red));
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChatClient {
    pub(crate) history: ChatHistory,
    pub(crate) should_quit: bool,
    pub(crate) stream: Option<TcpStream>,
    buf_message: Vec<u8>,
}

impl ChatClient {
    pub(crate) fn new() -> Self {
        let stream = TcpStream::connect("0.0.0.0:7878")
            .and_then(|s| {
                s.set_nonblocking(true)?;
                Ok(s)
            })
            .ok();

        Self {
            buf_message: Vec::new(),
            history: ChatHistory::default(),
            should_quit: false,
            stream,
        }
    }

    pub(crate) fn write(&mut self, to_send: String) -> anyhow::Result<()> {
        if let Some(tcp_stream) = self.stream.as_mut() {
            let with_newlines = format!("{to_send}\r\n");

            tcp_stream.write_all(with_newlines.as_bytes())?;
        }

        Ok(())
    }

    pub(crate) fn read(&mut self) -> anyhow::Result<()> {
        let mut buf_tmp = vec![0; 512];

        if let Some(tcp_stream) = &mut self.stream {
            match tcp_stream.read(&mut buf_tmp) {
                Ok(n) if n > 0 => {
                    self.buf_message.extend_from_slice(&buf_tmp[..n]);

                    if let Some(pos) = self
                        .buf_message
                        .windows(2)
                        .position(|window| window == b"\r\n")
                    {
                        let raw_message = self.buf_message.drain(..pos).collect::<Vec<u8>>();

                        if let Ok(message) = str::from_utf8(&raw_message) {
                            if !message.is_empty() {
                                // @CLEANUP: Immediately push to chat log and color differently
                                // until confirmed? As opposed to waiting for the server to return
                                // the same message back.
                                self.history.message(message);
                                self.buf_message.clear();
                            }
                        }
                    }
                }
                Ok(0) => {
                    self.stream = None;
                    self.history.error("Get out of it");
                }
                Err(e) if e.kind() != ErrorKind::WouldBlock => {
                    self.stream = None;
                    self.history.error("Get out of it");
                }
                _ => {}
            }
        }

        Ok(())
    }
}

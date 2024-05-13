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

#[derive(Debug, Default)]
pub(crate) struct ChatClient {
    pub(crate) history: ChatHistory,
    pub(crate) should_quit: bool,
    pub(crate) stream: Option<TcpStream>,
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

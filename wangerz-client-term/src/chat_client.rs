use std::{
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    str,
};

use crossterm::style;

use crate::Renderable;

#[derive(Debug, Default)]
pub(crate) struct ChatHistory {
    pub(crate) entries: Vec<Vec<(String, style::Color)>>,
}

impl Renderable for ChatHistory {
    fn render_into(&self, buf: &mut crate::RenderBuffer, rect: &crate::Rect) {
        let height = rect.height as usize;

        for (i, entry) in self.entries.iter().rev().take(height).enumerate() {
            let mut x = rect.x;

            for part in entry.iter() {
                for ch in part.0.chars() {
                    let y = rect.y + rect.height - 1 - i as u16;

                    buf.put_at(
                        x,
                        y,
                        ch,
                        part.1,
                        style::Color::Reset,
                        crate::CellStyle::Normal,
                    );

                    x += 1;
                }
            }
        }
    }
}

impl ChatHistory {
    pub(crate) fn info(&mut self, msg: impl Into<String>) {
        self.entries.push(vec![(msg.into(), style::Color::White)]);
    }

    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        self.entries.push(vec![(msg.into(), style::Color::Red)]);
    }

    pub(crate) fn message(&mut self, msg: &str) {
        use wangerz_message_parser;

        fn parts_for_node(node: wangerz_message_parser::AstNode) -> Vec<(String, style::Color)> {
            match node {
                wangerz_message_parser::AstNode::Text { value, .. } => {
                    vec![(value, style::Color::White)]
                }
                wangerz_message_parser::AstNode::ChannelMention {
                    raw_channel_name: channel_name,
                    ..
                } => {
                    vec![(channel_name, style::Color::Blue)]
                }
                wangerz_message_parser::AstNode::UserMention {
                    raw_user_name: user_name,
                    ..
                } => {
                    vec![(user_name, style::Color::Cyan)]
                }
                wangerz_message_parser::AstNode::Command {
                    raw_name: name,
                    args,
                    ..
                } => {
                    let args_parts: Vec<(String, style::Color)> =
                        args.into_iter().flat_map(parts_for_node).collect();

                    let mut parts = vec![(name, style::Color::Grey)];
                    parts.extend(args_parts);
                    parts
                }
            }
        }

        let parsed = wangerz_message_parser::parse(msg);
        let entry = parsed
            .nodes
            .into_iter()
            .flat_map(parts_for_node)
            .collect::<Vec<(String, style::Color)>>();

        self.entries.push(entry);
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
        let mut buf_tmp = vec![0; 1504];

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

use std::{
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    str,
};

use crossterm::style;

use crate::Renderable;

#[derive(Debug)]
struct ChatHistoryPartStyle {
    fg: style::Color,
    bg: style::Color,
    attr: crate::CellStyle, // @REFACTOR: make into a separate type
}

impl ChatHistoryPartStyle {
    fn new(fg: style::Color, bg: style::Color, attr: crate::CellStyle) -> Self {
        Self { fg, bg, attr }
    }
}

#[derive(Debug)]
pub(crate) struct ChatHistory {
    entries: Vec<Vec<(String, ChatHistoryPartStyle)>>,
}

impl Renderable for ChatHistory {
    fn render_into(&self, buf: &mut crate::RenderBuffer, rect: &crate::Rect) {
        let height = rect.height as usize;

        for (i, entry) in self.entries.iter().rev().take(height).enumerate() {
            let mut x = rect.x;

            for part in entry.iter() {
                for ch in part.0.chars() {
                    let y = rect.y + rect.height - 1 - i as u16;

                    buf.put_at(x, y, ch, part.1.fg, part.1.bg, part.1.attr);

                    x += 1;
                }
            }
        }
    }
}

impl ChatHistory {
    fn new() -> Self {
        Self { entries: vec![] }
    }

    pub(crate) fn info(&mut self, msg: impl Into<String>) {
        self.entries.push(vec![(
            msg.into(),
            ChatHistoryPartStyle::new(
                style::Color::White,
                style::Color::Reset,
                crate::CellStyle::Normal,
            ),
        )]);
    }

    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        self.entries.push(vec![(
            msg.into(),
            ChatHistoryPartStyle::new(
                style::Color::Red,
                style::Color::Reset,
                crate::CellStyle::Bold,
            ),
        )]);
    }

    pub(crate) fn message(&mut self, msg: &str) {
        fn parts_for_node(
            node: wangerz_message_parser::AstNode,
        ) -> Vec<(String, ChatHistoryPartStyle)> {
            match node {
                wangerz_message_parser::AstNode::Text { value, .. } => {
                    vec![(
                        value,
                        ChatHistoryPartStyle::new(
                            style::Color::White,
                            style::Color::Reset,
                            crate::CellStyle::Normal,
                        ),
                    )]
                }
                wangerz_message_parser::AstNode::ChannelMention {
                    raw_channel_name, ..
                } => {
                    vec![(
                        raw_channel_name,
                        ChatHistoryPartStyle::new(
                            style::Color::Black,
                            style::Color::Cyan,
                            crate::CellStyle::Bold,
                        ),
                    )]
                }
                wangerz_message_parser::AstNode::UserMention { raw_user_name, .. } => {
                    vec![(
                        raw_user_name,
                        ChatHistoryPartStyle::new(
                            style::Color::Black,
                            style::Color::Magenta,
                            crate::CellStyle::Bold,
                        ),
                    )]
                }
                wangerz_message_parser::AstNode::Command {
                    raw_name: name,
                    args,
                    ..
                } => {
                    let args_parts = args
                        .into_iter()
                        .flat_map(parts_for_node)
                        .collect::<Vec<_>>();
                    let mut parts = vec![(
                        name,
                        ChatHistoryPartStyle::new(
                            style::Color::DarkGreen,
                            style::Color::Reset,
                            crate::CellStyle::Bold,
                        ),
                    )];
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
            .collect::<Vec<_>>();

        self.entries.push(entry);
    }
}

#[derive(Debug)]
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
            history: ChatHistory::new(),
            should_quit: false,
            stream,
        }
    }

    pub(crate) fn write(&mut self, to_send: String) -> anyhow::Result<()> {
        if let Some(tcp_stream) = self.stream.as_mut() {
            let request = wangerz_protocol::RequestBuilder::new()
                .with_message(to_send)
                .build();

            tcp_stream.write_all(&request.as_bytes())?;
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

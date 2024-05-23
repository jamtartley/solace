use std::{
    io::{ErrorKind, Read},
    net::TcpStream,
};

use crossterm::style;
use wangerz_protocol::{code::RES_TOPIC_CHANGE, request::Request, response::Response};

use crate::{color::hex_to_rgb, CellStyle, Rect, Renderable};

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
struct ChatHistoryPart(String, ChatHistoryPartStyle);

impl ChatHistoryPart {
    fn new(text: String, style: ChatHistoryPartStyle) -> Self {
        Self(text, style)
    }
}

#[derive(Debug)]
struct ChatHistory {
    entries: Vec<Vec<ChatHistoryPart>>,
}

impl Renderable for ChatHistory {
    fn render_into(&self, buf: &mut crate::RenderBuffer, rect: &crate::Rect) {
        let height = rect.height as usize;

        for (i, entry) in self.entries.iter().rev().take(height).enumerate() {
            let mut x = rect.x;

            for part in entry.iter() {
                for ch in part.0.chars() {
                    if x >= rect.width {
                        break;
                    }

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

    fn error(&mut self, msg: impl Into<String>) {
        self.entries.push(vec![ChatHistoryPart::new(
            msg.into(),
            ChatHistoryPartStyle::new(
                style::Color::Red,
                style::Color::Reset,
                crate::CellStyle::Bold,
            ),
        )]);
    }

    fn message(&mut self, msg: &str, timestamp: &str, origin: &str) {
        fn parts_for_node(
            node: wangerz_message_parser::AstNode,
            has_origin: bool,
        ) -> Vec<ChatHistoryPart> {
            match node {
                wangerz_message_parser::AstNode::Text { value, .. } => {
                    vec![ChatHistoryPart::new(
                        value,
                        ChatHistoryPartStyle::new(
                            if has_origin {
                                hex_to_rgb(crate::config!(colors.message))
                            } else {
                                hex_to_rgb(crate::config!(colors.server_message))
                            },
                            hex_to_rgb(crate::config!(colors.background)),
                            crate::CellStyle::Normal,
                        ),
                    )]
                }
                wangerz_message_parser::AstNode::ChannelMention {
                    raw_channel_name, ..
                } => {
                    vec![ChatHistoryPart::new(
                        raw_channel_name,
                        ChatHistoryPartStyle::new(
                            style::Color::Black,
                            hex_to_rgb(crate::config!(colors.channel_mention)),
                            crate::CellStyle::Bold,
                        ),
                    )]
                }
                wangerz_message_parser::AstNode::UserMention { raw_user_name, .. } => {
                    vec![ChatHistoryPart::new(
                        raw_user_name,
                        ChatHistoryPartStyle::new(
                            style::Color::Black,
                            hex_to_rgb(crate::config!(colors.user_mention)),
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
                        .flat_map(|part| parts_for_node(part, has_origin))
                        .collect::<Vec<_>>();
                    let mut parts = vec![ChatHistoryPart::new(
                        name,
                        ChatHistoryPartStyle::new(
                            hex_to_rgb(crate::config!(colors.command)),
                            hex_to_rgb(crate::config!(colors.background)),
                            crate::CellStyle::Bold,
                        ),
                    )];
                    parts.extend(args_parts);
                    parts
                }
            }
        }

        let has_origin = !origin.is_empty();
        let parsed = wangerz_message_parser::parse(msg);
        let mut entry = parsed
            .nodes
            .into_iter()
            .flat_map(|part| parts_for_node(part, has_origin))
            .collect::<Vec<_>>();

        // @REFACTOR: Come up with a better way to prepend metadata to chat log entry
        entry.insert(
            0,
            ChatHistoryPart::new(
                format!(" {timestamp} "),
                ChatHistoryPartStyle::new(
                    style::Color::Grey,
                    style::Color::DarkGrey,
                    crate::CellStyle::Bold,
                ),
            ),
        );
        entry.insert(
            1,
            ChatHistoryPart::new(
                format!(
                    " {origin:16}",
                    origin = format!(
                        "{marker}{origin:15}",
                        marker = if has_origin { "@" } else { "--" }
                    )
                ),
                ChatHistoryPartStyle::new(
                    if has_origin {
                        style::Color::Cyan
                    } else {
                        style::Color::Grey
                    },
                    style::Color::Reset,
                    crate::CellStyle::Bold,
                ),
            ),
        );
        entry.insert(
            2,
            ChatHistoryPart::new(
                " ".to_owned(),
                ChatHistoryPartStyle::new(
                    style::Color::Reset,
                    style::Color::Reset,
                    crate::CellStyle::Normal,
                ),
            ),
        );

        self.entries.push(entry);
    }
}

#[derive(Debug, Default)]
struct ChatTopic(String);

impl Renderable for ChatTopic {
    fn render_into(&self, buf: &mut crate::RenderBuffer, rect: &Rect) {
        for i in 0..rect.width {
            if let Some(ch) = self.0.chars().nth(i.into()) {
                buf.put_at(
                    i,
                    0,
                    ch,
                    style::Color::Black,
                    style::Color::Cyan,
                    CellStyle::Italic,
                );
            } else {
                buf.put_at(
                    i,
                    0,
                    ' ',
                    style::Color::Black,
                    style::Color::Cyan,
                    CellStyle::Italic,
                );
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct ChatWindow {
    buf_message: Vec<u8>,
    stream: Option<TcpStream>,
    topic: ChatTopic,
    history: ChatHistory,
}

impl ChatWindow {
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
            stream,
            topic: ChatTopic::default(),
        }
    }

    pub(crate) fn write(&mut self, to_send: String) -> anyhow::Result<()> {
        if let Some(tcp_stream) = self.stream.as_mut() {
            Request::new(to_send).write_to(tcp_stream)?;
        }

        Ok(())
    }

    pub(crate) fn read(&mut self) -> anyhow::Result<()> {
        let mut buf_tmp = vec![0; 1504];

        if let Some(tcp_stream) = &mut self.stream {
            match tcp_stream.read(&mut buf_tmp) {
                Ok(n) if n > 0 => {
                    self.buf_message.extend_from_slice(&buf_tmp[..n]);

                    while let Ok(res) = Response::try_from(&mut self.buf_message) {
                        let Response {
                            message,
                            origin,
                            timestamp,
                            code,
                            ..
                        } = res;
                        let timestamp = self.to_local_time(timestamp);

                        match code {
                            RES_TOPIC_CHANGE => {
                                self.topic.0 = message;
                            }
                            _ => self.history.message(&message, &timestamp, &origin),
                        }
                    }
                }
                Ok(0) => {
                    self.stream = None;
                    self.history.error("Server closed the connection");
                }
                Err(e) if e.kind() != ErrorKind::WouldBlock => {
                    self.stream = None;
                    self.history.error("Server closed the connection");
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn to_local_time(&self, timestamp: u64) -> String {
        use chrono::{Local, TimeZone, Utc};

        let local = Utc
            .timestamp_opt(timestamp as i64, 0)
            .unwrap()
            .with_timezone(&Local);

        local.format("%H:%M:%S").to_string()
    }
}

impl Renderable for ChatWindow {
    fn render_into(&self, buf: &mut crate::RenderBuffer, rect: &crate::Rect) {
        self.topic.render_into(
            buf,
            &Rect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: 1,
            },
        );
        self.history.render_into(
            buf,
            &Rect {
                x: rect.x,
                y: rect.y + 1,
                width: rect.width,
                height: rect.height,
            },
        );
    }
}

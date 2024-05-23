use std::{
    io::{ErrorKind, Read},
    net::TcpStream,
};

use crossterm::style;
use wangerz_message_parser::{Ast, AstNode};
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
struct ChatHistoryEntry {
    author: Option<String>,
    parts: Vec<ChatHistoryPart>,
    timestamp: String,
}

impl ChatHistoryEntry {
    fn new(ast: Ast, author: Option<String>, timestamp: String) -> Self {
        let mut parts = vec![
            Self::part_from_timestamp(timestamp.clone()),
            Self::part_from_author(author.clone()),
        ];
        parts.extend(
            ast.nodes
                .into_iter()
                .flat_map(|arg| Self::parts_for_node(arg, author.clone()))
                .collect::<Vec<ChatHistoryPart>>(),
        );

        Self {
            author,
            timestamp,
            parts,
        }
    }

    fn part_from_timestamp(timestamp: String) -> ChatHistoryPart {
        ChatHistoryPart::new(
            format!(" {timestamp} "),
            ChatHistoryPartStyle::new(
                hex_to_rgb(crate::config!(colors.timestamp_fg)),
                hex_to_rgb(crate::config!(colors.timestamp_bg)),
                crate::CellStyle::Bold,
            ),
        )
    }

    fn part_from_author(author: Option<String>) -> ChatHistoryPart {
        const MAX_AUTHOR_LENGTH: usize = 16;

        let formatted_part = if let Some(author) = author.clone() {
            let truncated_author = if author.len() > MAX_AUTHOR_LENGTH {
                &author[..MAX_AUTHOR_LENGTH]
            } else {
                author.as_str()
            };
            format!(" {:>17} ", format!("@{truncated_author}"))
        } else {
            format!(" {:>17} ", "--")
        };

        ChatHistoryPart::new(
            formatted_part,
            ChatHistoryPartStyle::new(
                if author.is_some() {
                    hex_to_rgb(crate::config!(colors.user_name))
                } else {
                    hex_to_rgb(crate::config!(colors.server_message))
                },
                style::Color::Reset,
                if author.is_some() {
                    crate::CellStyle::Bold
                } else {
                    crate::CellStyle::Normal
                },
            ),
        )
    }

    fn parts_for_node(node: AstNode, author: Option<String>) -> Vec<ChatHistoryPart> {
        match node {
            wangerz_message_parser::AstNode::Text { value, .. } => {
                vec![ChatHistoryPart::new(
                    value,
                    ChatHistoryPartStyle::new(
                        if author.is_some() {
                            hex_to_rgb(crate::config!(colors.message))
                        } else {
                            hex_to_rgb(crate::config!(colors.server_message))
                        },
                        style::Color::Reset,
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
                        hex_to_rgb(crate::config!(colors.channel_mention)),
                        style::Color::Reset,
                        crate::CellStyle::Bold,
                    ),
                )]
            }
            wangerz_message_parser::AstNode::UserMention { raw_user_name, .. } => {
                vec![ChatHistoryPart::new(
                    raw_user_name,
                    ChatHistoryPartStyle::new(
                        hex_to_rgb(crate::config!(colors.user_mention)),
                        style::Color::Reset,
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
                    .flat_map(|arg| Self::parts_for_node(arg, author.clone()))
                    .collect::<Vec<ChatHistoryPart>>();
                let mut parts = vec![ChatHistoryPart::new(
                    name,
                    ChatHistoryPartStyle::new(
                        hex_to_rgb(crate::config!(colors.command)),
                        style::Color::Reset,
                        crate::CellStyle::Bold,
                    ),
                )];
                parts.extend(args_parts);
                parts
            }
        }
    }
}

#[derive(Debug)]
struct ChatHistory {
    entries: Vec<ChatHistoryEntry>,
}

impl Renderable for ChatHistory {
    fn render_into(&self, buf: &mut crate::RenderBuffer, rect: &crate::Rect) {
        let height = rect.height as usize;

        for (i, entry) in self.entries.iter().rev().take(height).enumerate() {
            let mut x = rect.x;

            for part in entry.parts.iter() {
                for ch in part.0.chars() {
                    if x >= rect.width {
                        break;
                    }

                    let y = rect.y + rect.height - 1 - i as u16;

                    buf.put_at(x, y, ch, part.1.bg, part.1.fg, part.1.attr);

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

    fn error(&mut self, _msg: impl Into<String>) {}

    fn message(&mut self, msg: &str, timestamp: &str, origin: &str) {
        let parsed = wangerz_message_parser::parse(msg);
        let entry = ChatHistoryEntry::new(
            parsed,
            if origin.is_empty() {
                None
            } else {
                Some(origin.to_owned())
            },
            timestamp.to_owned(),
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
                    hex_to_rgb(crate::config!(colors.topic_bg)),
                    hex_to_rgb(crate::config!(colors.topic_fg)),
                    CellStyle::Bold,
                );
            } else {
                buf.put_at(
                    i,
                    0,
                    ' ',
                    hex_to_rgb(crate::config!(colors.topic_bg)),
                    hex_to_rgb(crate::config!(colors.topic_fg)),
                    CellStyle::Bold,
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

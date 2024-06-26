use crossterm::style;
use futures::sink::SinkExt;
use solace_message_parser::{parse, AstMessage, AstNode};
use solace_protocol::code::{
    RES_ACK_MESSAGE, RES_COMMAND_LIST, RES_NICK_LIST, RES_TOPIC_CHANGE, RES_YOUR_NICK,
};
use solace_protocol::request::RequestMessage;
use solace_protocol::{request::Request, response::Response};
use tokio::io::{split, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::{config_hex_color, prompt::Prompt, CellStyle, Rect, Renderable};

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

/// # Fields
///
/// - `id`: An optional `u32` representing a unique identifier for the message.
///    Will only be set on outbound messages and is used to reconcile with acks
///    from the server to show in the UI that the message is pending/sent.
///    `is_confirmed`: Has the server acked the message sent with this `id`?
#[derive(Debug)]
struct ChatHistoryEntry {
    author: Option<String>,
    id: Option<u32>,
    is_confirmed: bool,
    parts: Vec<ChatHistoryPart>,
    timestamp: String,
}

impl ChatHistoryEntry {
    fn new(ast: AstMessage, author: Option<String>, timestamp: String, id: Option<u32>) -> Self {
        let mut parts = Self::prefix(timestamp.clone(), author.clone());
        parts.extend(Self::parts_for_ast(&ast, &author));

        Self {
            author,
            id,
            is_confirmed: false,
            timestamp,
            parts,
        }
    }

    fn error(msg: &str) -> Self {
        let timestamp = chrono::Utc::now().format("%H:%M:%S").to_string();
        let mut parts = Self::prefix(timestamp.clone(), None);
        parts.push(ChatHistoryPart::new(
            msg.to_owned(),
            ChatHistoryPartStyle {
                fg: config_hex_color!(colors.error_fg),
                bg: config_hex_color!(colors.error_bg),
                attr: crate::CellStyle::Bold,
            },
        ));

        Self {
            author: None,
            id: None,
            is_confirmed: true,
            parts,
            timestamp,
        }
    }

    fn prefix(timestamp: String, author: Option<String>) -> Vec<ChatHistoryPart> {
        vec![
            Self::part_from_timestamp(timestamp.clone()),
            Self::part_from_author(author.clone()),
        ]
    }

    fn part_from_timestamp(timestamp: String) -> ChatHistoryPart {
        ChatHistoryPart::new(
            format!(" {timestamp} "),
            ChatHistoryPartStyle::new(
                config_hex_color!(colors.timestamp_fg),
                config_hex_color!(colors.timestamp_bg),
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
                    config_hex_color!(colors.user_name)
                } else {
                    config_hex_color!(colors.server_message)
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

    fn parts_for_ast(ast: &AstMessage, author: &Option<String>) -> Vec<ChatHistoryPart> {
        match ast {
            AstMessage::Command(command) => match command {
                AstNode::Command { args, .. } => {
                    let mut parts = vec![Self::part_for_node(command.clone(), author)];
                    parts.extend(
                        args.iter()
                            .map(|n| Self::part_for_node(n.clone(), author))
                            .collect::<Vec<ChatHistoryPart>>(),
                    );

                    parts
                }
                _ => unreachable!(),
            },
            AstMessage::Normal(nodes) => nodes
                .iter()
                .map(|n| Self::part_for_node(n.clone(), author))
                .collect::<Vec<ChatHistoryPart>>(),
        }
    }

    fn part_for_node(node: AstNode, author: &Option<String>) -> ChatHistoryPart {
        match node {
            AstNode::Command { raw_name, .. } => ChatHistoryPart::new(
                raw_name,
                ChatHistoryPartStyle::new(
                    config_hex_color!(colors.command),
                    style::Color::Reset,
                    crate::CellStyle::Bold,
                ),
            ),
            AstNode::UserMention { raw_user_name, .. } => ChatHistoryPart::new(
                raw_user_name,
                ChatHistoryPartStyle::new(
                    config_hex_color!(colors.user_mention),
                    style::Color::Reset,
                    crate::CellStyle::Bold,
                ),
            ),
            AstNode::ChannelMention {
                raw_channel_name, ..
            } => ChatHistoryPart::new(
                raw_channel_name,
                ChatHistoryPartStyle::new(
                    config_hex_color!(colors.channel_mention),
                    style::Color::Reset,
                    crate::CellStyle::Bold,
                ),
            ),
            AstNode::Text { value, .. } => ChatHistoryPart::new(
                value,
                ChatHistoryPartStyle::new(
                    if author.is_some() {
                        config_hex_color!(colors.message)
                    } else {
                        config_hex_color!(colors.server_message)
                    },
                    style::Color::Reset,
                    crate::CellStyle::Normal,
                ),
            ),
            AstNode::Whitespace { span } => ChatHistoryPart::new(
                " ".repeat(span.len()),
                ChatHistoryPartStyle::new(
                    style::Color::Reset,
                    style::Color::Reset,
                    crate::CellStyle::Normal,
                ),
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ChatHistory {
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
                    let bg = if !entry.is_confirmed && entry.id.is_some() {
                        // @TODO: Generate unconfirmed colors
                        style::Color::Reset
                    } else {
                        part.1.bg
                    };
                    let fg = if !entry.is_confirmed && entry.id.is_some() {
                        style::Color::Black
                    } else {
                        part.1.fg
                    };

                    buf.put_at(x, y, ch, bg, fg, part.1.attr);

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

    pub(crate) fn error(&mut self, msg: &str) {
        self.entries.push(ChatHistoryEntry::error(msg));
    }

    fn message(&mut self, msg: &str, timestamp: &str, origin: &str, id: Option<u32>) {
        let parsed = solace_message_parser::parse(msg);
        let entry = ChatHistoryEntry::new(
            parsed,
            if origin.is_empty() {
                None
            } else {
                Some(origin.to_owned())
            },
            timestamp.to_owned(),
            id,
        );

        self.entries.push(entry);
    }

    fn ack(&mut self, id: u32) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == Some(id)) {
            entry.is_confirmed = true;
        }
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
                    config_hex_color!(colors.topic_bg),
                    config_hex_color!(colors.topic_fg),
                    CellStyle::Bold,
                );
            } else {
                buf.put_at(
                    i,
                    0,
                    ' ',
                    config_hex_color!(colors.topic_bg),
                    config_hex_color!(colors.topic_fg),
                    CellStyle::Bold,
                );
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct ChatWindow {
    buf_message: Vec<u8>,
    topic: ChatTopic,
    req: FramedWrite<WriteHalf<TcpStream>, Request>,
    res: FramedRead<ReadHalf<TcpStream>, Response>,
    pub(crate) history: ChatHistory,
    pub(crate) prompt: Prompt,
}

impl ChatWindow {
    pub(crate) async fn new() -> anyhow::Result<Self> {
        let stream = TcpStream::connect("0.0.0.0:7878").await?;

        let (reader, writer) = split(stream);
        let req = FramedWrite::new(writer, Request::default());
        let res = FramedRead::new(reader, Response::default());

        let local_commands = vec!["exit".to_owned(), "connect".to_owned()];
        let mut prompt = Prompt::new();
        prompt.register_local_commands(local_commands);

        Ok(Self {
            buf_message: Vec::new(),
            history: ChatHistory::new(),
            prompt,
            req,
            res,
            topic: ChatTopic::default(),
        })
    }

    pub(crate) async fn write(&mut self, to_send: String) -> anyhow::Result<()> {
        let ast = parse(&to_send);

        if self.handle_local_command(&ast) {
            return Ok(());
        }

        let message = match ast {
            AstMessage::Command(AstNode::Command {
                parsed_name, args, ..
            }) => match parsed_name.as_str() {
                "ping" => Some(RequestMessage::Ping),
                "disconnect" => Some(RequestMessage::Disconnect),
                "nick" => Some(RequestMessage::NewNick(
                    match args
                        .iter()
                        .skip_while(|arg| matches!(*arg, AstNode::Whitespace { .. }))
                        .cloned()
                        .collect::<Vec<AstNode>>()
                        .first()
                    {
                        Some(AstNode::Text { value, .. }) => value.to_owned(),
                        _ => todo!(),
                    },
                )),
                "topic" => Some(RequestMessage::NewTopic(
                    match args
                        .iter()
                        .skip_while(|arg| matches!(*arg, AstNode::Whitespace { .. }))
                        .cloned()
                        .collect::<Vec<AstNode>>()
                        .first()
                    {
                        Some(AstNode::Text { value, .. }) => value.to_owned(),
                        _ => todo!(),
                    },
                )),
                "whois" => Some(RequestMessage::WhoIs(
                    match args
                        .iter()
                        .skip_while(|arg| matches!(*arg, AstNode::Whitespace { .. }))
                        .cloned()
                        .collect::<Vec<AstNode>>()
                        .first()
                    {
                        Some(AstNode::Text { value, .. }) => value.to_owned(),
                        _ => todo!(),
                    },
                )),
                _ => unimplemented!(),
            },
            AstMessage::Normal(_) => Some(RequestMessage::Message(to_send.to_owned())),
            _ => unreachable!(),
        };

        if let Some(message) = message {
            let id = rand::random::<u32>();
            let timestamp = chrono::Utc::now().format("%H:%M:%S").to_string();
            let request = Request::new(id, message);
            self.req.send(request).await?;

            self.history
                .message(&to_send, &timestamp, &self.prompt.nick, Some(id));
        }

        Ok(())
    }

    pub(crate) async fn read(&mut self) -> anyhow::Result<()> {
        match self.res.next().await {
            Some(Ok(res)) => {
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
                    RES_YOUR_NICK => {
                        self.prompt.nick = message;
                    }
                    RES_COMMAND_LIST => {
                        let mut commands = message
                            .split(' ')
                            .map(|x| x.to_owned())
                            .collect::<Vec<String>>();

                        commands.sort_by_key(|a| a.to_lowercase());

                        self.prompt.commands = commands;
                    }
                    RES_NICK_LIST => {
                        let mut nicks = message
                            .split(' ')
                            .map(|x| x.to_owned())
                            .collect::<Vec<String>>();

                        nicks.sort_by_key(|a| a.to_lowercase());

                        self.prompt.nicks = nicks;
                    }
                    RES_ACK_MESSAGE => {
                        self.history.ack(message.parse::<u32>()?);
                    }
                    _ => self.history.message(&message, &timestamp, &origin, None),
                }
            }
            None => {
                self.history.error("Server closed the connection");
            }
            _ => (),
        }

        Ok(())
    }

    fn handle_local_command(&mut self, ast: &AstMessage) -> bool {
        match ast {
            AstMessage::Command(AstNode::Command { parsed_name, .. }) => {
                match parsed_name.as_str() {
                    "exit" => {
                        // @TODO: Just leave channel. not program
                        crossterm::terminal::disable_raw_mode().unwrap();
                        crossterm::execute!(
                            std::io::stdout(),
                            crossterm::terminal::LeaveAlternateScreen
                        )
                        .unwrap();
                        std::process::exit(0);
                    }
                    /* "connect" => match args.first() {
                        Some(AstNode::Text { value, .. }) => {
                            if self.stream.is_some() {
                                self.history.error("Already connected!");
                            } else {
                                let stream = TcpStream::connect(value.trim())
                                    .and_then(|s| {
                                        s.set_nonblocking(true)?;
                                        Ok(s)
                                    })
                                    .ok();

                                if stream.is_none() {
                                    self.history
                                        .error(format!("Could not connect to {value}").as_str());
                                }

                                self.stream = stream;
                            }

                            true
                        }
                        Some(_) | None => {
                            self.history.error("Usage: connect <addr>");
                            false
                        }
                    }, */
                    _ => false,
                }
            }
            _ => false,
        }
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

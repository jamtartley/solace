use anyhow::Context;
use std::sync::mpsc::Sender;
use std::{net::TcpStream, sync::Arc};
use wangerz_message_parser::AstNode;
use wangerz_protocol::code::{ERR_INVALID_ARGUMENT, RES_DISCONNECTED, RES_PONG};
use wangerz_protocol::response::ResponseBuilder;

use crate::Message;

type Execute = fn(&Arc<TcpStream>, &Sender<Message>, &Vec<AstNode>) -> anyhow::Result<()>;

#[derive(Clone)]
pub(crate) struct Command {
    name: &'static str,
    description: &'static str,
    usage: &'static str,
    pub(crate) execute: Execute,
}

const COMMANDS: &[Command] = &[
    Command {
        name: "ping",
        description: "Ping the server",
        usage: "/ping",
        execute: |stream, _, _| {
            ResponseBuilder::new(RES_PONG, "pong".to_owned())
                .build()
                .write_to(stream)?;

            Ok(())
        },
    },
    Command {
        name: "disconnect",
        description: "Disconnect from the server",
        usage: "/disconnect",
        execute: |stream, messages, _| {
            let addr = stream
                .peer_addr()
                .context("ERROR: Failed to get client socket address")?;

            ResponseBuilder::new(
                RES_DISCONNECTED,
                "You have disconnected from wangerz".to_owned(),
            )
            .build()
            .write_to(stream)?;

            messages
                .send(Message::ClientDisconnected { addr })
                .context("ERROR: Could not send disconnected message to client: {addr}")?;
            Ok(())
        },
    },
    Command {
        name: "nick",
        description: "Set your nickname",
        usage: "/nick <nickname>",
        execute: |stream, messages, args| {
            match &args.first() {
                Some(AstNode::Text { value, .. }) => {
                    let trimmed = value.trim();
                    let nickname = trimmed[0..16.min(trimmed.len())].to_owned();

                    messages
                        .send(Message::NickChanged {
                            stream: stream.clone(),
                            nickname,
                        })
                        .context("ERROR: Could not send nickname message to client")?;
                }
                _ => {
                    ResponseBuilder::new(
                        ERR_INVALID_ARGUMENT,
                        "Usage: /nick <nickname>".to_owned(),
                    )
                    .build()
                    .write_to(stream)?;
                }
            }

            Ok(())
        },
    },
];

pub(crate) fn parse_command(
    command_node: &wangerz_message_parser::AstNode,
) -> anyhow::Result<Option<Command>> {
    match command_node {
        wangerz_message_parser::AstNode::Command { parsed_name, .. } => {
            for command in COMMANDS.iter() {
                if parsed_name == command.name {
                    return Ok(Some(command.clone()));
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

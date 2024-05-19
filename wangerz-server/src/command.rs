use anyhow::Context;
use std::sync::mpsc::Sender;
use std::{net::TcpStream, sync::Arc};
use wangerz_protocol::code::{RES_DISCONNECTED, RES_PONG};
use wangerz_protocol::response::Response;

use crate::Message;

#[derive(Clone)]
pub(crate) struct Command {
    name: &'static str,
    description: &'static str,
    usage: &'static str,
    pub(crate) execute: fn(&Arc<TcpStream>, &Sender<Message>) -> anyhow::Result<()>,
}

const COMMANDS: &[Command] = &[
    Command {
        name: "ping",
        description: "Ping the server",
        usage: "/ping",
        execute: |stream, _| {
            Response::new(0, RES_PONG, "pong".to_owned()).write_to(stream)?;

            Ok(())
        },
    },
    Command {
        name: "disconnect",
        description: "Disconnect from the server",
        usage: "/disconnect",
        execute: |stream, messages| {
            let addr = stream
                .peer_addr()
                .context("ERROR: Failed to get client socket address")?;

            Response::new(
                0,
                RES_DISCONNECTED,
                "You have disconnected from wangerz".to_owned(),
            )
            .write_to(stream)?;

            messages
                .send(Message::ClientDisconnected { addr })
                .context("ERROR: Could not send disconnected message to client: {addr}")?;
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

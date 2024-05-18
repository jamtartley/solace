use anyhow::Context;
use std::io::Write;
use std::sync::mpsc::Sender;
use std::{net::TcpStream, sync::Arc};

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
            writeln!(stream.as_ref(), "PONG\r\n").context("ERROR: Could not send PONG")?;
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
            // @FEATURE: Provide a reason for disconnecting
            writeln!(stream.as_ref(), "You have disconnected.\r\n")
                .context("ERROR: Could not write disconnect message to client")?;
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

use std::net::SocketAddr;
use wangerz_message_parser::{Ast, AstNode};
use wangerz_protocol::code::{
    ERR_INVALID_ARGUMENT, RES_DISCONNECTED, RES_NICK_CHANGE, RES_PONG, RES_TOPIC_CHANGE,
    RES_TOPIC_CHANGE_MESSAGE,
};
use wangerz_protocol::response::ResponseBuilder;

use crate::Server;

type Execute = fn(&mut Server, &SocketAddr, &Ast) -> anyhow::Result<()>;

#[derive(Clone)]
pub(crate) struct Command {
    name: &'static str,
    description: &'static str,
    usage: &'static str,
    pub(crate) execute: Execute,
}

fn ping(server: &mut Server, from: &SocketAddr, _ast: &Ast) -> anyhow::Result<()> {
    if let Some(client) = server.clients.get(from) {
        ResponseBuilder::new(RES_PONG, "pong".to_owned())
            .build()
            .write_to(&client.conn)?;
    }

    Ok(())
}

fn disconnect(server: &mut Server, from: &SocketAddr, _ast: &Ast) -> anyhow::Result<()> {
    if let Some(client) = server.clients.get(from) {
        ResponseBuilder::new(
            RES_DISCONNECTED,
            "You have disconnected from wangerz".to_owned(),
        )
        .build()
        .write_to(&client.conn)?;

        for (_, other) in server.clients.iter() {
            ResponseBuilder::new(
                RES_DISCONNECTED,
                format!("{} has disconnected.", client.nick.clone()),
            )
            .build()
            .write_to(&other.conn)?;
        }
    }
    Ok(())
}

fn nick(server: &mut Server, from: &SocketAddr, ast: &Ast) -> anyhow::Result<()> {
    // @CLEANUP: Cloning client list
    let all = server.clients.clone();

    if let Some(client) = server.clients.get_mut(from) {
        if let Some(AstNode::Command { args, .. }) = ast.nodes.first() {
            match args.first() {
                Some(AstNode::Text { value, .. }) => {
                    let trimmed = value.trim();
                    let nickname = trimmed[0..16.min(trimmed.len())].to_owned();
                    let old_nickname = client.nick.clone();

                    client.nick.clone_from(&nickname);

                    let nick_notification_user = format!("You are now known as @{}", nickname);
                    let nick_notification_other =
                        format!("@{} is now known as @{}", old_nickname, nickname);

                    for (_, other) in all.iter() {
                        let notification = if other.ip == client.ip {
                            nick_notification_user.clone()
                        } else {
                            nick_notification_other.clone()
                        };
                        ResponseBuilder::new(RES_NICK_CHANGE, notification)
                            .build()
                            .write_to(&other.conn)?;
                    }
                }
                _ => {
                    ResponseBuilder::new(
                        ERR_INVALID_ARGUMENT,
                        "Usage: /nick <nickname>".to_owned(),
                    )
                    .build()
                    .write_to(&client.conn)?;
                }
            }
        }
    }

    Ok(())
}

fn topic(server: &mut Server, from: &SocketAddr, ast: &Ast) -> anyhow::Result<()> {
    if let Some(client) = server.clients.get(from) {
        if let Some(AstNode::Command { args, .. }) = ast.nodes.first() {
            match args.first() {
                Some(AstNode::Text { value, .. }) => {
                    let new_topic = value.trim().to_owned();
                    server.topic.clone_from(&new_topic);

                    for (_, other) in server.clients.iter() {
                        ResponseBuilder::new(RES_TOPIC_CHANGE, server.topic.clone())
                            .build()
                            .write_to(&other.conn)?;
                        ResponseBuilder::new(
                            RES_TOPIC_CHANGE_MESSAGE,
                            format!("Topic was changed to {}", server.topic.clone()),
                        )
                        .build()
                        .write_to(&other.conn)?;
                    }

                    println!("INFO: Topic changed to {new_topic}");
                }
                _ => {
                    ResponseBuilder::new(ERR_INVALID_ARGUMENT, "Usage: /topic <topic>".to_owned())
                        .build()
                        .write_to(&client.conn)?;
                }
            }
        }
    }

    Ok(())
}

const COMMANDS: &[Command] = &[
    Command {
        name: "ping",
        description: "Ping the server",
        usage: "/ping",
        execute: ping,
    },
    Command {
        name: "disconnect",
        description: "Disconnect from the server",
        usage: "/disconnect",
        execute: disconnect,
    },
    Command {
        name: "nick",
        description: "Set your nickname",
        usage: "/nick <nickname>",
        execute: nick,
    },
    Command {
        name: "topic",
        description: "Set the chat topic",
        usage: "/topic <topic>",
        execute: topic,
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

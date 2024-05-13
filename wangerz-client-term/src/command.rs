use std::net::TcpStream;

use crate::chat_client::ChatClient;

pub(crate) struct Command {
    name: &'static str,
    description: &'static str,
    usage: &'static str,
    pub(crate) execute: fn(&mut ChatClient, &[&str]) -> anyhow::Result<()>,
}

const COMMANDS: &[Command] = &[
    Command {
        name: "quit",
        description: "Quit wangerz and get on with your life",
        usage: "quit",
        execute: |client, _args| -> anyhow::Result<()> {
            client.should_quit = true;

            Ok(())
        },
    },
    Command {
        name: "connect",
        description: "Connect to a chat server",
        usage: "connect <host:port>",
        execute: |client, args| {
            if client.stream.is_none() {
                if args.len() == 2 {
                    let ip = args[0];
                    let port = args[1].parse::<usize>().unwrap();
                    let conn = format!("{}:{}", ip, port);

                    client.stream = TcpStream::connect(conn)
                        .and_then(|stream| {
                            stream.set_nonblocking(true)?;
                            Ok(stream)
                        })
                        .ok();
                }
            }

            Ok(())
        },
    },
];

pub(crate) fn parse_command(raw: &str) -> anyhow::Result<Option<(&Command, Vec<&str>)>> {
    let mut parts = raw.trim().split_whitespace();

    if let Some(command) = parts.next() {
        let name = command
            .strip_prefix('/')
            .ok_or_else(|| anyhow::anyhow!("ERROR: Commands must start with a slash"))?;
        let args = parts.collect::<Vec<&str>>();

        for cmd in COMMANDS {
            if cmd.name == name {
                return Ok(Some((cmd, args)));
            }
        }
    }

    Ok(None)
}

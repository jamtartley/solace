#![allow(dead_code)]

mod command;

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread,
};

use anyhow::Context;
use command::parse_command;

use wangerz_message_parser::AstNode;
use wangerz_protocol::{request::Request, response::ResponseBuilder};

struct Client {
    conn: Arc<TcpStream>,
    ip: SocketAddr,
}

pub(crate) enum Message {
    ClientConnected { author: Arc<TcpStream> },
    ClientDisconnected { addr: SocketAddr },
    // @FEATURE: Extend to target specific clients
    Sent { from: SocketAddr, message: String },
}

fn client_worker(stream: Arc<TcpStream>, messages: Sender<Message>) -> anyhow::Result<()> {
    let addr = stream
        .peer_addr()
        .context("ERROR: Failed to get client socket address")?;
    messages
        .send(Message::ClientConnected {
            author: stream.clone(),
        })
        .context("ERROR: Could not send message from {addr}:")?;

    let mut buf_message = Vec::new();

    loop {
        let mut buf_tmp = vec![0; 1504];

        match stream.as_ref().read(&mut buf_tmp) {
            Ok(0) => {
                messages
                    .send(Message::ClientDisconnected { addr })
                    .context("ERROR: Could not send disconnected message to client {addr}")?;
                break;
            }
            Ok(n) => {
                buf_message.extend_from_slice(&buf_tmp[..n]);

                let req = Request::try_from(buf_message.clone())?;
                let ast = wangerz_message_parser::parse(&req.message);

                // @FEATURE: Handle pinging mentioned users
                match ast.nodes.first() {
                    Some(AstNode::Command { raw_name, .. }) => {
                        if let Ok(Some(command)) = parse_command(&ast.nodes[0]) {
                            (command.execute)(&stream, &messages)?
                        } else {
                            writeln!(
                                stream.as_ref(),
                                "ERROR: Command '{raw_name}' not found or invalid\r\n"
                            ).context("ERROR: Failed to write 'invalid command' message to client: {addr}")?;
                            eprintln!("ERROR: Command not found or invalid");
                        }
                    }
                    Some(_) => {
                        messages
                            .send(Message::Sent {
                                from: addr,
                                message: req.message.to_owned(),
                            })
                            .context("ERROR: Could not send message")?;
                    }
                    None => {
                        eprintln!("ERROR: Received empty message or parsing failed");
                    }
                }

                buf_message.clear();
            }
            Err(_) => {
                messages
                    .send(Message::ClientDisconnected { addr })
                    .context("ERROR: Could not read message and disconnected client")?;
                break;
            }
        }
    }

    Ok(())
}

fn server_worker(messages: Receiver<Message>) -> anyhow::Result<()> {
    let mut clients = HashMap::<SocketAddr, Client>::new();

    loop {
        let message = messages.recv().expect("Socket has not hung up");

        match message {
            Message::ClientConnected { author } => {
                let addr = author
                    .peer_addr()
                    .context("ERROR: Failed to get client socket address")?;
                clients.insert(
                    addr,
                    Client {
                        conn: author.clone(),
                        ip: addr,
                    },
                );

                let response = ResponseBuilder::new()
                    .with_message("Welcome to wangerz!".to_owned())
                    .with_code(1)
                    .build();

                author
                    .clone()
                    .as_ref()
                    .write_all(&response.as_bytes())
                    .context("ERROR: Could not send welcome message")?;

                println!("INFO: Client {addr} connected");
            }
            Message::ClientDisconnected { addr } => {
                clients.remove(&addr);

                println!("INFO: Client {addr} disconnected");
            }
            Message::Sent { from, message } => {
                if clients.contains_key(&from) {
                    println!("INFO: Client {from} sent message: {message:?}");

                    let response = ResponseBuilder::new()
                        .with_message(message)
                        .with_code(1)
                        .build();

                    for (_, client) in clients.iter_mut() {
                        let response_bytes = response.as_bytes();
                        client
                            .conn
                            .as_ref()
                            .write_all(&response_bytes)
                            .context(format!(
                                "ERROR: Failed to write response to client {}",
                                client.ip
                            ))?;
                    }
                }
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    const PORT: i32 = 7878;
    let addr = &format!("0.0.0.0:{PORT}");
    let listener = TcpListener::bind(addr).context("ERROR: Failed to bind to {PORT}")?;

    println!("INFO: Server listening on {addr}");

    let (tx, rx) = channel();
    thread::spawn(|| server_worker(rx));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let stream = Arc::new(stream);
                let sender = tx.clone();

                thread::spawn(|| {
                    client_worker(stream, sender).context("ERROR: Error spawning client thread")
                });
            }
            Err(e) => eprintln!("ERROR: could not accept connection: {e}"),
        }
    }

    Ok(())
}

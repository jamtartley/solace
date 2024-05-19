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

                let req = wangerz_protocol::Request::try_from(buf_message.clone())?;
                println!("{req:?}");
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

                writeln!(author.clone().as_ref(), "Welcome to wangerz!\r\n")
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

                    let timestamp = chrono::Utc::now().format("%H:%M:%S");

                    for (_, client) in clients.iter_mut() {
                        writeln!(client.conn.as_ref(), "{timestamp} -> {message}\r\n").context(
                            "ERROR: could not broadcast message to all the clients from {from}:",
                        )?;
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

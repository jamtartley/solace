#![allow(dead_code)]

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    str,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread,
};

use anyhow::Context;

struct Client {
    conn: Arc<TcpStream>,
}

enum Message {
    ClientConnected { author: Arc<TcpStream> },
    ClientDisconnected { addr: SocketAddr },
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

    let mut buf = vec![0; 64];

    loop {
        match stream.as_ref().read(&mut buf) {
            Ok(0) => {
                let _ = messages
                    .send(Message::ClientDisconnected { addr })
                    .context("ERROR: Could not send disconnected message to client {addr}");
                break;
            }
            Ok(n) => {
                let bytes = buf[0..n]
                    .iter()
                    .filter(|&x| *x >= 32)
                    .cloned()
                    .collect::<Vec<u8>>();

                match str::from_utf8(&bytes) {
                    Ok(message) => {
                        if message.is_empty() {
                            continue;
                        }

                        messages
                            .send(Message::Sent {
                                from: addr,
                                message: message.to_string(),
                            })
                            .context("ERROR: Could not send message")?;
                    }
                    Err(e) => eprintln!("ERROR: Could not decode message into UTF-8: {e}"),
                }
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
                    },
                );

                println!("INFO: Client {addr} connected");
            }
            Message::ClientDisconnected { addr } => {
                clients.remove(&addr);

                println!("INFO: Client {addr} disconnected");
            }
            Message::Sent { from, message } => {
                if clients.get(&from).is_some() {
                    println!("INFO: Client {from} sent message: {message:?}");

                    for (_, client) in clients.iter_mut() {
                        writeln!(client.conn.as_ref(), "{message}").context(
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

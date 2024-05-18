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
    time::SystemTime,
};

use anyhow::Context;

struct Client {
    conn: Arc<TcpStream>,
    ip: SocketAddr,
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

                if let Some(pos) = buf_message.windows(2).position(|window| window == b"\r\n") {
                    let message = buf_message.drain(..pos).collect::<Vec<u8>>();

                    match str::from_utf8(&message) {
                        Ok(message) => {
                            if message.is_empty() {
                                continue;
                            }

                            let ast = wangerz_message_parser::parse(message);
                            println!("{ast:?}");

                            messages
                                .send(Message::Sent {
                                    from: addr,
                                    message: message.to_owned(),
                                })
                                .context("ERROR: Could not send message")?;
                            buf_message.clear();
                        }
                        Err(e) => eprintln!("ERROR: Could not decode message into UTF-8: {e}"),
                    }
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

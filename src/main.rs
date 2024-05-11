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

type Result<T> = std::result::Result<T, ()>;

struct Client {
    conn: Arc<TcpStream>,
}

enum Message {
    ClientConnected { author: Arc<TcpStream> },
    ClientDisconnected { addr: SocketAddr },
    Sent { from: SocketAddr, bytes: Vec<u8> },
}

fn client(stream: Arc<TcpStream>, messages: Sender<Message>) -> Result<()> {
    let addr = stream.peer_addr().unwrap();
    messages
        .send(Message::ClientConnected {
            author: stream.clone(),
        })
        .map_err(|e| eprintln!("ERROR: Could not send message from {addr}: {e}"))?;

    let mut buf = vec![0; 64];

    loop {
        let n = stream.as_ref().read(&mut buf).map_err(|e| {
            eprintln!("ERROR: Could not read message from {addr}: {e}");

            let _ = messages
                .send(Message::ClientDisconnected { addr })
                .map_err(|e| eprintln!("ERROR: Could not send message: {e}"));
        })?;

        if n > 0 {
            let mut bytes = Vec::new();

            for x in &buf[0..n] {
                if *x >= 32 {
                    bytes.push(*x);
                }
            }

            let _ = messages
                .send(Message::Sent { from: addr, bytes })
                .map_err(|e| eprintln!("ERROR: Could not send message: {e}"));
        } else {
            let _ = messages
                .send(Message::ClientDisconnected { addr })
                .map_err(|e| eprintln!("ERROR: Could not send message: {e}"));

            break;
        }
    }

    Ok(())
}

fn server(messages: Receiver<Message>) -> Result<()> {
    let mut clients = HashMap::<SocketAddr, Client>::new();

    loop {
        let message = messages.recv().expect("Socket has not hung up");

        match message {
            Message::ClientConnected { author } => {
                let addr = author.peer_addr().unwrap();
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
            Message::Sent { from, bytes } => {
                if clients.get_mut(&from).is_some() {
                    if let Ok(message) = str::from_utf8(&bytes) {
                        println!("INFO: Client {from} sent message: {message:?}");

                        for (addr, client) in clients.iter() {
                            println!("{addr}");
                            if *addr != from {
                                let _ = writeln!(client.conn.as_ref(), "{message}").map_err(|e| {
                                        eprintln!("ERROR: could not broadcast message to all the clients from {from}: {e}");
                                    });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    const PORT: i32 = 7878;
    let addr = &format!("0.0.0.0:{PORT}");
    let listener = TcpListener::bind(addr)?;

    println!("INFO: Server listening on {addr}");

    let (sender, receiver) = channel();
    thread::spawn(|| server(receiver));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let stream = Arc::new(stream);
                let sender = sender.clone();

                thread::spawn(|| client(stream, sender));
            }
            Err(e) => eprintln!("ERROR: could not accept connection: {e}"),
        }
    }

    Ok(())
}

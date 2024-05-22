#![allow(dead_code)]

mod command;

use std::{
    collections::HashMap,
    io::Read,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread,
};

use anyhow::Context;
use command::parse_command;

use rand::Rng;
use wangerz_message_parser::AstNode;
use wangerz_protocol::{
    code::{
        ERR_COMMAND_NOT_FOUND, RES_CHAT_MESSAGE_OK, RES_GOODBYE, RES_HELLO, RES_NICK_CHANGE,
        RES_TOPIC_CHANGE, RES_WELCOME,
    },
    request::Request,
    response::ResponseBuilder,
};

struct Server {
    clients: HashMap<SocketAddr, Client>,
    topic: String,
}

impl Server {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
            topic: "Hello, world!".to_owned(),
        }
    }
}

struct Client {
    conn: Arc<TcpStream>,
    ip: SocketAddr,
    nick: String,
}

impl Client {
    fn new(conn: Arc<TcpStream>, ip: SocketAddr) -> Self {
        let nick = Client::generate_random_nick();

        Self { conn, ip, nick }
    }

    fn generate_random_nick() -> String {
        let len = 16;
        let mut bytes = vec![0; len];

        for byte in bytes.iter_mut().take(len) {
            *byte = rand::thread_rng().gen_range(65..91);
        }

        String::from_utf8(bytes).unwrap()
    }
}

pub(crate) enum Message {
    ClientConnected {
        author: Arc<TcpStream>,
    },
    ClientDisconnected {
        addr: SocketAddr,
    },
    // @FEATURE: Extend to target specific clients
    Sent {
        from: SocketAddr,
        message: String,
        request_id: u32,
    },
    NickChanged {
        stream: Arc<TcpStream>,
        nickname: String,
    },
    TopicChanged {
        new_topic: String,
    },
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
                    Some(AstNode::Command { raw_name, args, .. }) => {
                        if let Ok(Some(command)) = parse_command(&ast.nodes[0]) {
                            (command.execute)(&stream, &messages, args)?
                        } else {
                            ResponseBuilder::new(
                                ERR_COMMAND_NOT_FOUND,
                                format!("Command {} not found", raw_name),
                            )
                            .build()
                            .write_to(&stream)?;
                        }
                    }
                    Some(_) => {
                        messages
                            .send(Message::Sent {
                                from: addr,
                                message: req.message.to_owned(),
                                request_id: req.id,
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
    let mut server = Server::new();

    loop {
        let message = messages.recv().expect("Socket has not hung up");

        match message {
            Message::ClientConnected { author } => {
                let addr = author
                    .peer_addr()
                    .context("ERROR: Failed to get client socket address")?;
                let client = Client::new(author.clone(), addr);
                let nick = client.nick.clone();
                server.clients.insert(addr, client);

                // @FIXME: Forward request id
                ResponseBuilder::new(RES_WELCOME, "Welcome to wangerz!".to_owned())
                    .build()
                    .write_to(&author)?;

                for (_, other) in server.clients.iter_mut() {
                    if other.ip != author.peer_addr().unwrap() {
                        ResponseBuilder::new(RES_HELLO, format!("{} has joined the channel", nick))
                            .build()
                            .write_to(&other.conn)?;
                    }
                }

                println!("INFO: Client {addr} connected");
            }
            Message::ClientDisconnected { addr } => {
                if let Some(left) = server.clients.remove(&addr) {
                    let nick = left.nick.clone();

                    for (_, other) in server.clients.iter_mut() {
                        if other.ip != left.ip {
                            ResponseBuilder::new(
                                RES_GOODBYE,
                                format!("{} has left the channel", nick),
                            )
                            .build()
                            .write_to(&other.conn)?;
                        }
                    }

                    println!("INFO: Client {addr} disconnected");
                }
            }
            Message::Sent {
                from,
                message,
                request_id,
            } => {
                if let Some(client) = server.clients.get(&from) {
                    println!("INFO: Client {from} sent message: {message:?}");

                    // @FIXME: Forward request id
                    let response = ResponseBuilder::new(RES_CHAT_MESSAGE_OK, message)
                        .with_request_id(request_id)
                        .with_origin(client.nick.clone())
                        .build();

                    for (_, client) in server.clients.iter() {
                        response.write_to(&client.conn)?;
                    }
                }
            }
            Message::NickChanged { stream, nickname } => {
                let addr = &stream.clone().peer_addr().unwrap();
                if let Some(client) = server.clients.get_mut(addr) {
                    let old_nickname = client.nick.clone();
                    client.nick.clone_from(&nickname);

                    let nick_notification_user = format!("You are now known as @{}", nickname);
                    let nick_notification_other =
                        format!("@{} is now known as @{}", old_nickname, nickname);

                    for (_, client) in server.clients.iter() {
                        let notification = if client.ip == stream.peer_addr().unwrap() {
                            nick_notification_user.clone()
                        } else {
                            nick_notification_other.clone()
                        };
                        ResponseBuilder::new(RES_NICK_CHANGE, notification)
                            .build()
                            .write_to(&client.conn)?;
                    }
                }
            }
            Message::TopicChanged { new_topic } => {
                server.topic = new_topic.clone();

                for (_, client) in server.clients.iter() {
                    ResponseBuilder::new(RES_TOPIC_CHANGE, server.topic.clone())
                        .build()
                        .write_to(&client.conn)?;
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

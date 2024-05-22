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
        ERR_COMMAND_NOT_FOUND, RES_CHAT_MESSAGE_OK, RES_GOODBYE, RES_HELLO, RES_TOPIC_CHANGE,
        RES_WELCOME,
    },
    request::Request,
    response::ResponseBuilder,
};

#[derive(Clone)]
struct Server {
    clients: HashMap<SocketAddr, Client>,
    topic: String,
}

impl Server {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
            topic: String::default(),
        }
    }

    fn other_clients(&self, ip: SocketAddr) -> Vec<&Client> {
        self.clients
            .iter()
            .filter(|(_, c)| c.ip != ip)
            .map(|(_, c)| c)
            .collect::<Vec<&Client>>()
    }
}

#[derive(Clone)]
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

                while let Ok(req) = Request::try_from(&mut buf_message) {
                    messages
                        .send(Message::Sent {
                            from: addr,
                            message: req.message,
                            request_id: req.id,
                        })
                        .context("ERROR: Failed to send message to server")?;
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

                ResponseBuilder::new(RES_WELCOME, "Welcome to wangerz!".to_owned())
                    .build()
                    .write_to(&author)?;
                ResponseBuilder::new(RES_TOPIC_CHANGE, server.topic.clone())
                    .build()
                    .write_to(&author)?;

                for (_, client) in server.clients.iter() {
                    ResponseBuilder::new(RES_HELLO, format!("{} has joined the channel", nick))
                        .build()
                        .write_to(&client.conn)?;
                }

                println!("INFO: Client {addr} connected");
            }
            Message::ClientDisconnected { addr } => {
                if let Some(left) = server.clients.remove(&addr) {
                    let nick = left.nick.clone();

                    for other in server.other_clients(left.ip) {
                        ResponseBuilder::new(RES_GOODBYE, format!("{} has left the channel", nick))
                            .build()
                            .write_to(&other.conn)?;
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

                    let ast = wangerz_message_parser::parse(&message);

                    // @FEATURE: Handle pinging mentioned users
                    match ast.nodes.first() {
                        Some(AstNode::Command { raw_name, .. }) => {
                            if let Ok(Some(command)) = parse_command(&ast.nodes[0]) {
                                (command.execute)(&mut server, &from, &ast)?
                            } else {
                                ResponseBuilder::new(
                                    ERR_COMMAND_NOT_FOUND,
                                    format!("Command {} not found", raw_name),
                                )
                                .build()
                                .write_to(&client.conn)?;
                            }
                        }
                        Some(_) => {
                            // @FIXME: Forward request id
                            let response = ResponseBuilder::new(RES_CHAT_MESSAGE_OK, message)
                                .with_request_id(request_id)
                                .with_origin(client.nick.clone())
                                .build();

                            for (_, client) in server.clients.iter() {
                                response.write_to(&client.conn)?;
                            }
                        }
                        None => {
                            eprintln!("ERROR: Received empty message or parsing failed");
                        }
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

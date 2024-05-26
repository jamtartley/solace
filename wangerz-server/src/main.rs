#![allow(dead_code)]

use futures::sink::SinkExt;
use rand::Rng;
use tokio::io::{split, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use wangerz_protocol::code::{
    RES_CHAT_MESSAGE_OK, RES_GOODBYE, RES_HELLO, RES_NICK_CHANGE, RES_TOPIC_CHANGE,
    RES_TOPIC_CHANGE_MESSAGE, RES_WELCOME, RES_YOUR_NICK,
};
use wangerz_protocol::request::Request;
use wangerz_protocol::response::{Response, ResponseBuilder};

type Tx = mpsc::UnboundedSender<Message>;
type Rx = mpsc::UnboundedReceiver<Message>;

macro_rules! respond {
    ($client:ident, $code: ident, $msg: expr) => {
        $client
            .res
            .send(ResponseBuilder::new($code, $msg).build())
            .await?;
    };
    ($client:ident, $code: ident, $msg: expr, $origin :expr) => {
        $client
            .res
            .send(
                ResponseBuilder::new($code, $msg)
                    .with_origin($origin)
                    .build(),
            )
            .await?;
    };
}

#[derive(Clone, Debug)]
struct MessageClient {
    addr: SocketAddr,
    nick: String,
}

#[derive(Clone, Debug)]
enum Message {
    ClientConnected(String),
    ClientDisconnected(String),
    Sent {
        from: MessageClient,
        message: String,
    },
    TopicChanged {
        from: MessageClient,
        topic: String,
    },
    NickChanged {
        from: MessageClient,
        new_nick: String,
    },
}

struct Server {
    clients: HashMap<SocketAddr, Tx>,
    topic: String,
}

struct Client {
    addr: SocketAddr,
    nick: String,
    req: FramedRead<ReadHalf<TcpStream>, Request>,
    res: FramedWrite<WriteHalf<TcpStream>, Response>,
    rx: Rx,
    tx: Tx,
}

impl Server {
    fn new() -> Self {
        Server {
            clients: HashMap::new(),
            topic: "[No topic]".to_owned(),
        }
    }

    async fn broadcast_to(&mut self, message: Message, to: SocketAddr) {
        if let Some(tx) = self.clients.get(&to) {
            let _ = tx.send(message.clone());
        }
    }

    async fn broadcast_all(&mut self, message: Message) {
        for tx in self.clients.values_mut() {
            let _ = tx.send(message.clone());
        }
    }

    async fn broadcast_others(&mut self, message: Message, sender: SocketAddr) {
        for (addr, tx) in self.clients.iter_mut() {
            if *addr != sender {
                let _ = tx.send(message.clone());
            }
        }
    }
}

impl Client {
    async fn new(addr: SocketAddr, stream: TcpStream) -> anyhow::Result<Client> {
        let (tx, rx) = mpsc::unbounded_channel();

        let (reader, writer) = split(stream);

        let nick = Self::generate_nick();
        let req = FramedRead::new(reader, Request::default());
        let res = FramedWrite::new(writer, Response::default());

        Ok(Client {
            addr,
            nick,
            req,
            res,
            rx,
            tx,
        })
    }

    fn generate_nick() -> String {
        let len = 16;
        let mut bytes = vec![0; len];

        for byte in bytes.iter_mut().take(len) {
            *byte = rand::thread_rng().gen_range(65..91);
        }

        String::from_utf8(bytes).unwrap()
    }
}

async fn handle_client(
    server: Arc<Mutex<Server>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut client = Client::new(addr, stream).await?;

    respond!(client, RES_WELCOME, "Welcome to wangerz!".to_owned());
    respond!(client, RES_YOUR_NICK, client.nick.clone());

    {
        let mut server = server.lock().await;
        server.clients.insert(addr, client.tx);
        server
            .broadcast_others(Message::ClientConnected(client.nick.clone()), addr)
            .await;
        respond!(client, RES_TOPIC_CHANGE, server.topic.clone());
    }

    loop {
        #[rustfmt::skip]
        tokio::select! {
            result = client.req.next() => match result {
                Some(Ok(req)) => {
                    if req.message.starts_with("/topic")  {
                        let topic = req.message.replace("/topic", "");
                        let mut server = server.lock().await;

                        server.topic.clone_from(&topic);
                        server.broadcast_to(
                            Message::Sent {
                                from: MessageClient {
                                    addr,
                                    nick: client.nick.clone(),
                                },
                                message: req.message.clone()
                            }, addr)
                        .await;
                        server
                            .broadcast_all(
                                Message::TopicChanged {
                                    from: MessageClient {
                                        addr, nick: client.nick.clone(),
                                    },
                                    topic: topic.clone().trim().to_owned()
                                })
                            .await;
                    } else if req.message.starts_with("/nick") {
                        let nick = req.message.replace("/nick", "").trim().to_owned();
                        let mut server = server.lock().await;
                        let was = client.nick.clone();

                        client.nick.clone_from(&nick);

                        server.broadcast_to(
                            Message::Sent {
                                from: MessageClient {
                                    addr,
                                    nick: client.nick.clone(),
                                },
                                message: req.message.clone()
                            },
                            addr)
                        .await;
                        server.broadcast_all(
                            Message::NickChanged {
                                from: MessageClient {
                                    addr,
                                    nick: was,
                                },
                                new_nick: nick.clone(),
                            })
                        .await;
                    } else {
                        let mut server = server.lock().await;
                        server
                            .broadcast_all(Message::Sent {
                                from: MessageClient {
                                    addr,
                                    nick: client.nick.clone(),
                                },
                                message: req.message
                            })
                            .await;
                    }
                }
                None => break,
                _ => break
            },
            Some(msg) = client.rx.recv() => {
                match msg {
                    Message::ClientConnected(nick) => {
                        println!("INFO: Client {nick} disconnected");
                        respond!(client, RES_HELLO, format!("{nick} has joined"));
                    }
                    Message::ClientDisconnected(nick) => {
                        println!("INFO: Client {nick} disconnected");
                        respond!(client, RES_GOODBYE, format!("{nick} has left the channel"));
                    }
                    Message::Sent { message, from, .. } => {
                        println!("INFO: Client {} sent message: {message:?}", from.nick);
                        respond!(client, RES_CHAT_MESSAGE_OK, message, format!("{}", from.nick));
                    }
                    Message::TopicChanged{ from, topic } => {
                        println!("INFO: Topic was changed by {} to: {topic}", from.nick);
                        respond!(client, RES_TOPIC_CHANGE, topic.clone());

                        let message = if addr == from.addr {
                            format!("You changed the channel topic to: {topic}")
                        } else {
                            format!("{} changed the channel topic to: {}", from.nick, topic.clone())
                        };
                        respond!(client, RES_TOPIC_CHANGE_MESSAGE, message);
                    }
                    Message::NickChanged{from, new_nick} => {
                        let message = if addr == from.addr {
                            format!("You are now known as {new_nick}")
                        } else {
                            format!("{} is now known as {new_nick}", from.nick)
                        };

                        if addr == from.addr {
                            respond!(client, RES_YOUR_NICK, new_nick);
                        }

                        respond!(client, RES_NICK_CHANGE, message);
                    }
                }
            }
        }
    }

    {
        let mut server = server.lock().await;

        server.clients.remove(&addr);
        server
            .broadcast_others(Message::ClientDisconnected(client.nick.clone()), addr)
            .await;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    const HOST: &str = "0.0.0.0";
    const PORT: i32 = 7878;

    let addr = format!("{HOST}:{PORT}");
    let listener = TcpListener::bind(&addr).await?;
    let server = Arc::new(Mutex::new(Server::new()));

    println!("INFO: Server listening on {PORT}");

    loop {
        let (stream, addr) = listener.accept().await?;
        let server = Arc::clone(&server);

        tokio::spawn(async move {
            if let Err(e) = handle_client(server, stream, addr).await {
                eprintln!("ERROR: {e}")
            }
        });
    }
}

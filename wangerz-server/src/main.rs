#![allow(dead_code)]

use futures::sink::SinkExt;
use tokio::io::{split, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use wangerz_protocol::code::{
    RES_CHAT_MESSAGE_OK, RES_GOODBYE, RES_HELLO, RES_TOPIC_CHANGE, RES_WELCOME,
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
enum Message {
    ClientConnected(SocketAddr),
    ClientDisconnected(SocketAddr),
    Sent { from: SocketAddr, message: String },
}

struct Server {
    clients: HashMap<SocketAddr, Tx>,
    topic: String,
}

struct Client {
    addr: SocketAddr,
    req: FramedRead<ReadHalf<TcpStream>, Request>,
    res: FramedWrite<WriteHalf<TcpStream>, Response>,
    rx: Rx,
}

impl Server {
    fn new() -> Self {
        Server {
            clients: HashMap::new(),
            topic: "[No topic]".to_owned(),
        }
    }

    async fn broadcast_all(&mut self, message: Message) {
        for client in self.clients.iter_mut() {
            let _ = client.1.send(message.clone());
        }
    }

    async fn broadcast_others(&mut self, message: Message, sender: SocketAddr) {
        for client in self.clients.iter_mut() {
            if *client.0 != sender {
                let _ = client.1.send(message.clone());
            }
        }
    }
}

impl Client {
    async fn new(
        addr: SocketAddr,
        server: Arc<Mutex<Server>>,
        stream: TcpStream,
    ) -> anyhow::Result<Client> {
        let (tx, rx) = mpsc::unbounded_channel();

        server.lock().await.clients.insert(addr, tx);

        let (reader, writer) = split(stream);

        let req = FramedRead::new(reader, Request::default());
        let res = FramedWrite::new(writer, Response::default());

        Ok(Client { addr, req, res, rx })
    }
}

async fn handle_client(
    server: Arc<Mutex<Server>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut client = Client::new(addr, server.clone(), stream).await?;

    {
        respond!(client, RES_WELCOME, "Welcome to wangerz!".to_owned());

        let mut server = server.lock().await;
        respond!(client, RES_TOPIC_CHANGE, server.topic.clone());
        server
            .broadcast_others(Message::ClientConnected(addr), addr)
            .await;
    }

    loop {
        tokio::select! {
            result = client.req.next() => match result {
                Some(Ok(req)) => {
                    let mut server = server.lock().await;
                    server
                        .broadcast_all(Message::Sent{from: addr, message: req.message})
                        .await;
                }
                _ => break
            },
            Some(msg) = client.rx.recv() => {
                match msg {
                    Message::ClientConnected(addr) => {
                        client.res.send(ResponseBuilder::new(RES_HELLO, format!("{addr} has joined")).build()).await?;
                    }
                    Message::ClientDisconnected(addr) => {
                        println!("INFO: Client {addr} disconnected");
                        respond!(client, RES_GOODBYE, format!("{addr} has left the channel"));
                    }
                    Message::Sent { message, from, .. } => {
                        println!("INFO: Client {from} sent message: {message:?}");
                        respond!(client, RES_CHAT_MESSAGE_OK, message, format!("{from}"));
                    }
                }
            }
        }
    }

    {
        let mut server = server.lock().await;
        server.clients.remove(&addr);

        server
            .broadcast_others(Message::ClientDisconnected(addr), addr)
            .await;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = Arc::new(Mutex::new(Server::new()));
    const HOST: &str = "0.0.0.0";
    const PORT: i32 = 7878;

    let addr = format!("{HOST}:{PORT}");
    let listener = TcpListener::bind(&addr).await?;

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

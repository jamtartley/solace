#![allow(dead_code)]

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use futures::SinkExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

type Tx = mpsc::UnboundedSender<Message>;
type Rx = mpsc::UnboundedReceiver<Message>;

#[derive(Clone, Debug)]
enum Message {
    ClientConnected(SocketAddr),
    ClientDisconnected(SocketAddr),
    Sent { from: SocketAddr, message: String },
}

struct Server {
    clients: HashMap<SocketAddr, Tx>,
}

struct Client {
    addr: SocketAddr,
    lines: Framed<TcpStream, LinesCodec>,
    rx: Rx,
}

impl Server {
    fn new() -> Self {
        Server {
            clients: HashMap::new(),
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
        state: Arc<Mutex<Server>>,
        lines: Framed<TcpStream, LinesCodec>,
    ) -> anyhow::Result<Client> {
        let (tx, rx) = mpsc::unbounded_channel();

        state.lock().await.clients.insert(addr, tx);

        Ok(Client { addr, lines, rx })
    }
}

async fn process(
    state: Arc<Mutex<Server>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let lines = Framed::new(stream, LinesCodec::new());

    let mut client = Client::new(addr, state.clone(), lines).await?;

    {
        let mut state = state.lock().await;
        state
            .broadcast_others(Message::ClientConnected(addr), addr)
            .await;
    }

    loop {
        tokio::select! {
            Some(msg) = client.rx.recv() => {
                match msg {
                    Message::ClientConnected(addr) => {
                        client.lines.send(format!("{addr} has joined the chat.")).await?;
                    }
                    Message::ClientDisconnected(addr) => {
                        client.lines.send(format!("{addr} has left the chat.")).await?;
                    }
                    Message::Sent { message, .. } => {
                        client.lines.send(&message).await?
                    }
                }
            }
            result = client.lines.next() => match result {
                Some(Ok(msg)) => {
                    let mut state = state.lock().await;
                    let msg = format!("{}: {}", addr, msg);

                    state.broadcast_all(Message::Sent { from: addr, message: msg }).await;
                }
                Some(Err(e)) => {
                    eprintln!("ERROR: {e}");
                }
                None => break,
            },
        }
    }

    {
        let mut state = state.lock().await;
        state.clients.remove(&addr);

        state
            .broadcast_others(Message::ClientDisconnected(addr), addr)
            .await;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(Server::new()));
    const HOST: &str = "0.0.0.0";
    const PORT: i32 = 7878;

    let addr = format!("{HOST}:{PORT}");
    let listener = TcpListener::bind(&addr).await?;

    println!("INFO: Server listening on {PORT}");

    loop {
        let (stream, addr) = listener.accept().await?;

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            if let Err(e) = process(state, stream, addr).await {
                eprintln!("ERROR: {e}")
            }
        });
    }
}

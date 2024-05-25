#![allow(dead_code)]

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use futures::SinkExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

type Tx = mpsc::UnboundedSender<String>;
type Rx = mpsc::UnboundedReceiver<String>;

struct Server {
    peers: HashMap<SocketAddr, Tx>,
}

struct Peer {
    addr: SocketAddr,
    lines: Framed<TcpStream, LinesCodec>,
    rx: Rx,
}

impl Server {
    fn new() -> Self {
        Server {
            peers: HashMap::new(),
        }
    }

    async fn broadcast_all(&mut self, message: &str) {
        for peer in self.peers.iter_mut() {
            let _ = peer.1.send(message.into());
        }
    }

    async fn broadcast_others(&mut self, sender: SocketAddr, message: &str) {
        for peer in self.peers.iter_mut() {
            if *peer.0 != sender {
                let _ = peer.1.send(message.into());
            }
        }
    }
}

impl Peer {
    async fn new(
        addr: SocketAddr,
        state: Arc<Mutex<Server>>,
        lines: Framed<TcpStream, LinesCodec>,
    ) -> anyhow::Result<Peer> {
        let (tx, rx) = mpsc::unbounded_channel();

        state.lock().await.peers.insert(addr, tx);

        Ok(Peer { addr, lines, rx })
    }
}

async fn process(
    state: Arc<Mutex<Server>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let lines = Framed::new(stream, LinesCodec::new());

    let mut peer = Peer::new(addr, state.clone(), lines).await?;

    {
        let mut state = state.lock().await;
        let msg = format!("{addr} has joined the chat");
        state.broadcast_others(addr, &msg).await;
    }

    loop {
        tokio::select! {
            Some(msg) = peer.rx.recv() => {
                peer.lines.send(&msg).await?;
            }
            result = peer.lines.next() => match result {
                Some(Ok(msg)) => {
                    let mut state = state.lock().await;
                    let msg = format!("{}: {}", addr, msg);

                    state.broadcast_others(addr, &msg).await;
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
        state.peers.remove(&addr);

        let msg = format!("{addr} has left the chat");
        state.broadcast_others(addr, &msg).await;
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

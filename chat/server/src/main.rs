use futures_util::SinkExt;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Sender, channel};
use tokio_websockets::{Message, ServerBuilder, WebSocketStream};

type Clients = Arc<Mutex<HashMap<SocketAddr, String>>>;

async fn handle_connection(
    addr: SocketAddr,
    mut ws_stream: WebSocketStream<TcpStream>,
    bcast_tx: Sender<String>,
    clients: Clients,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut bcast_rx = bcast_tx.subscribe();
    let mut username: Option<String> = None;

    loop {
        tokio::select! {
            incoming = ws_stream.next() => {
                match incoming {
                    Some(Ok(msg)) => {
                        if let Some(text) = msg.as_text() {
                            // First message is "__join:Name"
                            if text.starts_with("__join:") {
                                let name = text.trim_start_matches("__join:").to_string();
                                username = Some(name.clone());
                                clients.lock().unwrap().insert(addr, name.clone());
                                let _ = bcast_tx.send(format!("🔵 {} has joined the chat", name));
                                continue;
                            }

                            let name = username.clone().unwrap_or_else(|| "Anonymous".to_string());
                            println!("{} ({addr}) says: {}", name, text);
                            bcast_tx.send(format!("{}: {}", name, text))?;
                        }
                    }
                    Some(Err(err)) => return Err(err.into()),
                    None => {
                        // Disconnected
                        if let Some(name) = clients.lock().unwrap().remove(&addr) {
                            let _ = bcast_tx.send(format!("🔴 {} has left the chat", name));
                        }
                        return Ok(());
                    }
                }
            }
            msg = bcast_rx.recv() => {
                ws_stream.send(Message::text(msg?)).await?;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let (bcast_tx, _) = channel(100);
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    let listener = TcpListener::bind("127.0.0.1:7878").await?;
    println!("✅ Server running at ws://127.0.0.1:7878");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from {addr:?}");
        let bcast_tx = bcast_tx.clone();
        let clients = Arc::clone(&clients);

        tokio::spawn(async move {
            let (_req, ws_stream) = ServerBuilder::new().accept(socket).await?;
            handle_connection(addr, ws_stream, bcast_tx, clients).await
        });
    }
}
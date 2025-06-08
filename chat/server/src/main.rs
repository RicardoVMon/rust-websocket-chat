use futures_util::SinkExt;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Sender, channel};
use tokio_websockets::{Message, ServerBuilder, WebSocketStream};

use hyper::{Body, Request, Response, Server, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

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
                                let _ = bcast_tx.send(format!("🔵 {} se ha unido al chat", name));
                                println!(">> {} ({}) se ha conectado.", name, addr);
                                continue;
                            }

                            let name = username.clone().unwrap_or_else(|| "Anonymous".to_string());
                            println!("{} ({addr}) dice: {}", name, text);
                            bcast_tx.send(format!("{}: {}", name, text))?;
                        }
                    }
                    Some(Err(err)) => return Err(err.into()),
                    None => {
                        if let Some(name) = clients.lock().unwrap().remove(&addr) {
                            let _ = bcast_tx.send(format!("🔴 {} ha salido del chat", name));
                            println!("<< {} ({}) se ha desconectado.", name, addr);
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

async fn serve_html(_req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let mut file = match File::open("../chat.html").await {
        Ok(f) => f,
        Err(_) => {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Archivo no encontrado"))
                .unwrap());
        }
    };
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).await.unwrap();
    Ok(Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(contents))
        .unwrap())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Servidor HTTP para el HTML
    let make_svc = make_service_fn(|_conn| async { Ok::<_, hyper::Error>(service_fn(serve_html)) });
    let http_server = Server::bind(&([0, 0, 0, 0], 5500).into()).serve(make_svc);

    // Servidor WebSocket (puerto 7878)
    let (bcast_tx, _) = channel(100);
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let ws_listener = TcpListener::bind("0.0.0.0:7878").await?;
    
    println!("Server corriendo en ws://127.0.0.1:7878");
    println!("HTML en http://localhost:5500");

    // Ejecutar ambos servidores en paralelo
    tokio::spawn(async move {
        loop {
            let (socket, addr) = ws_listener.accept().await.unwrap();
            let bcast_tx = bcast_tx.clone();
            let clients = Arc::clone(&clients);

            tokio::spawn(async move {
                let (_req, ws_stream) = ServerBuilder::new().accept(socket).await?;
                handle_connection(addr, ws_stream, bcast_tx, clients).await
            });
        }
    });

    http_server.await?;
    Ok(())
}
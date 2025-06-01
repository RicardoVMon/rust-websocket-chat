use futures_util::SinkExt;
use futures_util::stream::StreamExt;
use http::Uri;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_websockets::{ClientBuilder, Message};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), tokio_websockets::Error> {
    let stdin = tokio::io::stdin();
    let mut input = BufReader::new(stdin).lines();

    print!("Enter your name: ");
    io::stdout().flush().unwrap(); // <- solución aquí

    let name = input.next_line().await?.unwrap_or_else(|| "Anonymous".to_string());

    let (mut ws_stream, _) =
        ClientBuilder::from_uri(Uri::from_static("ws://127.0.0.1:7878"))
            .connect()
            .await?;

    // Send initial name
    ws_stream.send(Message::text(format!("__join:{}", name))).await?;

    println!("Welcome, {name}! You can now start chatting.");

    loop {
        tokio::select! {
            incoming = ws_stream.next() => {
                match incoming {
                    Some(Ok(msg)) => {
                        if let Some(text) = msg.as_text() {
                            println!("{}", text);
                        }
                    },
                    Some(Err(err)) => return Err(err.into()),
                    None => return Ok(()),
                }
            }
            res = input.next_line() => {
                match res {
                    Ok(None) => return Ok(()),
                    Ok(Some(line)) => ws_stream.send(Message::text(line.to_string())).await?,
                    Err(err) => return Err(err.into()),
                }
            }
        }
    }
}
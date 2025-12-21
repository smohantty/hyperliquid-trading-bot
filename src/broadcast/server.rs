use crate::broadcast::types::WSEvent;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone)]
pub struct StatusBroadcaster {
    sender: broadcast::Sender<WSEvent>,
    last_config: Arc<Mutex<Option<WSEvent>>>,
    last_summary: Arc<Mutex<Option<WSEvent>>>,
}

impl StatusBroadcaster {
    pub fn new(port: Option<u16>) -> Self {
        let (sender, _) = broadcast::channel(100);
        let last_config = Arc::new(Mutex::new(None));
        let last_summary = Arc::new(Mutex::new(None));

        if let Some(p) = port {
            let sender_clone = sender.clone();
            let config_clone = last_config.clone();
            let summary_clone = last_summary.clone();

            tokio::spawn(async move {
                if let Err(e) = run_server(p, sender_clone, config_clone, summary_clone).await {
                    error!("WebSocket Server failed: {}", e);
                }
            });
        }

        Self {
            sender,
            last_config,
            last_summary,
        }
    }

    pub fn send(&self, event: WSEvent) {
        // Cache stateful events
        match &event {
            WSEvent::Config(_) => {
                let mut lock = self.last_config.lock().unwrap();
                *lock = Some(event.clone());
            }
            WSEvent::Summary(_) => {
                let mut lock = self.last_summary.lock().unwrap();
                *lock = Some(event.clone());
            }
            _ => {}
        }

        // We ignore "channel closed" errors as we might not have any subscribers
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WSEvent> {
        self.sender.subscribe()
    }
}

async fn run_server(
    port: u16,
    sender: broadcast::Sender<WSEvent>,
    last_config: Arc<Mutex<Option<WSEvent>>>,
    last_summary: Arc<Mutex<Option<WSEvent>>>,
) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket Status Server listening on: ws://{}", addr);

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let sender_clone = sender.clone();
        let config_clone = last_config.clone();
        let summary_clone = last_summary.clone();

        tokio::spawn(async move {
            if let Err(e) =
                handle_connection(stream, peer_addr, sender_clone, config_clone, summary_clone)
                    .await
            {
                warn!("Error handling connection from {}: {}", peer_addr, e);
            }
        });
    }

    Ok(())
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    sender: broadcast::Sender<WSEvent>,
    last_config: Arc<Mutex<Option<WSEvent>>>,
    last_summary: Arc<Mutex<Option<WSEvent>>>,
) -> anyhow::Result<()> {
    info!("New WebSocket connection: {}", peer_addr);

    // Accept the websocket handshake
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Subscribe to broadcast channel
    let mut rx = sender.subscribe();

    // Send Initial State (Config & Latest Summary)
    {
        let config_opt = last_config.lock().unwrap().clone();
        if let Some(event) = config_opt {
            let json_str = serde_json::to_string(&event)?;
            ws_sender.send(Message::Text(json_str)).await?;
        }

        let summary_opt = last_summary.lock().unwrap().clone();
        if let Some(event) = summary_opt {
            let json_str = serde_json::to_string(&event)?;
            ws_sender.send(Message::Text(json_str)).await?;
        }
    }

    // Since we only BROADCAST to clients, we can ignore incoming messages mostly,
    // but we need to keep the connection alive. We can select! loop.

    loop {
        tokio::select! {
            // Receive Message from Channel
            msg_res = rx.recv() => {
                match msg_res {
                    Ok(event) => {
                         let json_str = serde_json::to_string(&event)?;
                         ws_sender.send(Message::Text(json_str)).await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        warn!("Client {} lagged by {} messages", peer_addr, count);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }

            // Receive Message from Client (Heartbeats usually)
            client_msg = ws_receiver.next() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) => {
                        info!("Client {} disconnected", peer_addr);
                        break;
                    }
                    Some(Ok(_)) => {
                        // Ignore other messages (ping/pong handled by library/browser)
                    }
                    Some(Err(e)) => {
                        warn!("WebSocket error from {}: {}", peer_addr, e);
                        break;
                    }
                    None => {
                        break; // Stream closed
                    }
                }
            }
        }
    }

    Ok(())
}

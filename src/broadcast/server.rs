use crate::broadcast::types::WSEvent;
use crate::config::broadcast::WebsocketConfig;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use std::collections::VecDeque;
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
    last_info: Arc<Mutex<Option<WSEvent>>>,
    last_summary: Arc<Mutex<Option<WSEvent>>>,
    last_grid_state: Arc<Mutex<Option<WSEvent>>>,
    last_market_update: Arc<Mutex<Option<WSEvent>>>,
    order_history: Arc<Mutex<VecDeque<WSEvent>>>,
}

impl StatusBroadcaster {
    pub fn new(config: Option<WebsocketConfig>) -> Self {
        let (sender, _) = broadcast::channel(100);
        let last_config = Arc::new(Mutex::new(None));
        let last_info = Arc::new(Mutex::new(None));
        let last_summary = Arc::new(Mutex::new(None));
        let last_grid_state = Arc::new(Mutex::new(None));
        let last_market_update: Arc<Mutex<Option<WSEvent>>> = Arc::new(Mutex::new(None));
        let order_history = Arc::new(Mutex::new(VecDeque::with_capacity(50)));

        if let Some(conf) = config {
            let sender_clone = sender.clone();
            let config_clone = last_config.clone();
            let info_clone = last_info.clone();
            let summary_clone = last_summary.clone();
            let grid_state_clone = last_grid_state.clone();
            let market_update_clone = last_market_update.clone();
            let history_clone = order_history.clone();

            tokio::spawn(async move {
                if let Err(e) = run_server(
                    conf.host,
                    conf.port,
                    sender_clone,
                    config_clone,
                    info_clone,
                    summary_clone,
                    grid_state_clone,
                    market_update_clone,
                    history_clone,
                )
                .await
                {
                    error!("WebSocket Server failed: {}", e);
                }
            });
        }

        Self {
            sender,
            last_config,
            last_info,
            last_summary,
            last_grid_state,
            last_market_update,
            order_history,
        }
    }

    pub fn send(&self, event: WSEvent) {
        // Cache stateful events
        match &event {
            WSEvent::Config(_) => {
                let mut lock = self.last_config.lock().unwrap();
                *lock = Some(event.clone());
            }
            WSEvent::Info(_) => {
                let mut lock = self.last_info.lock().unwrap();
                *lock = Some(event.clone());
            }
            // Cache strategy summaries (either spot or perp)
            WSEvent::SpotGridSummary(_) | WSEvent::PerpGridSummary(_) => {
                let mut lock = self.last_summary.lock().unwrap();
                *lock = Some(event.clone());
            }
            // Cache grid state for new connections
            WSEvent::GridState(_) => {
                let mut lock = self.last_grid_state.lock().unwrap();
                *lock = Some(event.clone());
            }
            // Cache recent market update for new connections (so UI has price immediately)
            WSEvent::MarketUpdate(_) => {
                let mut lock = self.last_market_update.lock().unwrap();
                *lock = Some(event.clone());
            }
            // Cache recent order updates
            WSEvent::OrderUpdate(_) => {
                let mut lock = self.order_history.lock().unwrap();
                if lock.len() >= 50 {
                    lock.pop_front();
                }
                lock.push_back(event.clone());
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
    host: String,
    port: u16,
    sender: broadcast::Sender<WSEvent>,
    last_config: Arc<Mutex<Option<WSEvent>>>,
    last_info: Arc<Mutex<Option<WSEvent>>>,
    last_summary: Arc<Mutex<Option<WSEvent>>>,
    last_grid_state: Arc<Mutex<Option<WSEvent>>>,
    last_market_update: Arc<Mutex<Option<WSEvent>>>,
    order_history: Arc<Mutex<VecDeque<WSEvent>>>,
) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket Status Server listening on: ws://{}", addr);

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let sender_clone = sender.clone();
        let config_clone = last_config.clone();
        let info_clone = last_info.clone();
        let summary_clone = last_summary.clone();
        let grid_state_clone = last_grid_state.clone();
        let market_update_clone = last_market_update.clone();
        let history_clone = order_history.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(
                stream,
                peer_addr,
                sender_clone,
                config_clone,
                info_clone,
                summary_clone,
                grid_state_clone,
                market_update_clone,
                history_clone,
            )
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
    last_info: Arc<Mutex<Option<WSEvent>>>,
    last_summary: Arc<Mutex<Option<WSEvent>>>,
    last_grid_state: Arc<Mutex<Option<WSEvent>>>,
    last_market_update: Arc<Mutex<Option<WSEvent>>>,
    order_history: Arc<Mutex<VecDeque<WSEvent>>>,
) -> anyhow::Result<()> {
    info!("New WebSocket connection: {}", peer_addr);

    // Accept the websocket handshake
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Subscribe to broadcast channel
    let mut rx = sender.subscribe();

    // Send Initial State (Config, Summary, and Grid State)
    {
        let config_opt = last_config.lock().unwrap().clone();
        if let Some(event) = config_opt {
            let json_str = serde_json::to_string(&event)?;
            ws_sender.send(Message::Text(json_str)).await?;
        }

        let info_opt = last_info.lock().unwrap().clone();
        if let Some(event) = info_opt {
            let json_str = serde_json::to_string(&event)?;
            ws_sender.send(Message::Text(json_str)).await?;
        }

        let summary_opt = last_summary.lock().unwrap().clone();
        if let Some(event) = summary_opt {
            let json_str = serde_json::to_string(&event)?;
            ws_sender.send(Message::Text(json_str)).await?;
        }

        let grid_state_opt = last_grid_state.lock().unwrap().clone();
        if let Some(event) = grid_state_opt {
            let json_str = serde_json::to_string(&event)?;
            ws_sender.send(Message::Text(json_str)).await?;
        }

        let market_update_opt = last_market_update.lock().unwrap().clone();
        if let Some(event) = market_update_opt {
            let json_str = serde_json::to_string(&event)?;
            ws_sender.send(Message::Text(json_str)).await?;
        }

        // Send cached order history
        let history_events: Vec<WSEvent> = {
            let history = order_history.lock().unwrap();
            history.iter().cloned().collect()
        };

        for event in history_events {
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

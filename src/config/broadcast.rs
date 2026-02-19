use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastConfig {
    pub websocket: WebsocketConfig,
}

pub fn load_broadcast_config(cli_ws_port: Option<u16>) -> Result<BroadcastConfig> {
    // WebSocket Config
    // Priority: CLI Arg > Env Var > Default (9000)
    let port = if let Some(p) = cli_ws_port {
        p
    } else if let Ok(p_str) = env::var("WS_PORT") {
        p_str.parse::<u16>().unwrap_or(9000)
    } else {
        9000
    };

    let host = env::var("WS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

    let websocket = WebsocketConfig { port, host };

    Ok(BroadcastConfig { websocket })
}

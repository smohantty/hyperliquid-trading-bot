use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastConfig {
    pub websocket: WebsocketConfig,
}

pub fn load_broadcast_config(websocket_port: u16) -> BroadcastConfig {
    BroadcastConfig {
        websocket: WebsocketConfig {
            port: websocket_port,
            host: "0.0.0.0".to_string(),
        },
    }
}

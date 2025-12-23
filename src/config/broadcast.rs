use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketConfig {
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastConfig {
    pub websocket: WebsocketConfig,
    pub telegram: Option<TelegramConfig>,
}

pub fn load_broadcast_config(cli_ws_port: Option<u16>) -> Result<BroadcastConfig> {
    // 1. WebSocket Config
    // Priority: CLI Arg > Env Var > Default (9000)
    let port = if let Some(p) = cli_ws_port {
        p
    } else if let Ok(p_str) = env::var("WS_PORT") {
        p_str.parse::<u16>().unwrap_or(9000)
    } else {
        9000
    };

    let websocket = WebsocketConfig { port };

    // 2. Telegram Config
    let tg_token = env::var("TELEGRAM_BOT_TOKEN").ok();
    let tg_chat = env::var("TELEGRAM_CHAT_ID").ok();

    let telegram = if let (Some(bot_token), Some(chat_id)) = (tg_token, tg_chat) {
        Some(TelegramConfig { bot_token, chat_id })
    } else {
        None
    };

    Ok(BroadcastConfig {
        websocket,
        telegram,
    })
}

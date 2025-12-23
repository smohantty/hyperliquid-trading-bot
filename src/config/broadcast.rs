use crate::config::read_env_or_file;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketConfig {
    pub port: u16,
    pub host: String,
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

    let host = env::var("WS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

    let websocket = WebsocketConfig { port, host };

    // 2. Telegram Config
    let telegram = if let Ok(raw_path) = env::var("TELEGRAM_CONFIG_FILE") {
        // Expand tilde if present
        let path = if raw_path.starts_with("~/") {
            if let Ok(home) = env::var("HOME") {
                raw_path.replacen("~", &home, 1)
            } else {
                raw_path
            }
        } else {
            raw_path
        };

        let content = fs::read_to_string(&path)
            .context(format!("Failed to read TELEGRAM_CONFIG_FILE at {}", path))?;
        Some(
            serde_json::from_str(&content)
                .context("Failed to parse TELEGRAM_CONFIG_FILE as JSON")?,
        )
    } else {
        let tg_token = read_env_or_file("TELEGRAM_BOT_TOKEN").ok();
        let tg_chat = read_env_or_file("TELEGRAM_CHAT_ID").ok();

        if let (Some(bot_token), Some(chat_id)) = (tg_token, tg_chat) {
            Some(TelegramConfig { bot_token, chat_id })
        } else {
            None
        }
    };

    Ok(BroadcastConfig {
        websocket,
        telegram,
    })
}

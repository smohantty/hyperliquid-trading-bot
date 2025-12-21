use crate::broadcast::types::{PerpGridSummary, SpotGridSummary, WSEvent};
use anyhow::Result;
use log::{error, info, warn};
use std::env;
use std::sync::Arc;
use teloxide::prelude::*;
use tokio::sync::{broadcast, Mutex};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// Cached summary for telegram status command
#[derive(Clone)]
enum CachedSummary {
    SpotGrid(SpotGridSummary),
    PerpGrid(PerpGridSummary),
}

impl CachedSummary {
    fn format_status(&self) -> String {
        match self {
            CachedSummary::SpotGrid(s) => {
                format!(
                    "🟢 <b>SpotGrid</b>\n\
                     Symbol: <code>{}</code>\n\
                     💰 PnL: <code>{:.2}</code> (Unrl: <code>{:.2}</code>)\n\
                     📉 Price: <code>{:.4}</code>\n\
                     📦 Position: <code>{:.4}</code> @ <code>{:.4}</code>\n\
                     🔄 Roundtrips: <code>{}</code>\n\
                     📊 Grid: {} zones [{:.2} - {:.2}]",
                    s.symbol,
                    s.realized_pnl,
                    s.unrealized_pnl,
                    s.price,
                    s.position_size,
                    s.avg_entry_price,
                    s.roundtrips,
                    s.grid_count,
                    s.range_low,
                    s.range_high
                )
            }
            CachedSummary::PerpGrid(s) => {
                let position_icon = match s.position_side.as_str() {
                    "Long" => "📈",
                    "Short" => "📉",
                    _ => "➖",
                };
                format!(
                    "🟢 <b>PerpGrid ({} {}x)</b>\n\
                     Symbol: <code>{}</code>\n\
                     💰 PnL: <code>{:.2}</code> (Unrl: <code>{:.2}</code>)\n\
                     📉 Price: <code>{:.4}</code>\n\
                     {} Position: <code>{:.4}</code> {} @ <code>{:.4}</code>\n\
                     🔄 Roundtrips: <code>{}</code>\n\
                     📊 Grid: {} zones [{:.2} - {:.2}]",
                    s.grid_bias,
                    s.leverage,
                    s.symbol,
                    s.realized_pnl,
                    s.unrealized_pnl,
                    s.price,
                    position_icon,
                    s.position_size.abs(),
                    s.position_side,
                    s.avg_entry_price,
                    s.roundtrips,
                    s.grid_count,
                    s.range_low,
                    s.range_high
                )
            }
        }
    }
}

pub struct TelegramReporter {
    bot: Bot,
    chat_id: ChatId,
    receiver: broadcast::Receiver<WSEvent>,
}

impl TelegramReporter {
    pub fn new(receiver: broadcast::Receiver<WSEvent>) -> Result<Option<Self>> {
        let token = env::var("TELEGRAM_BOT_TOKEN").ok();
        let chat_id_str = env::var("TELEGRAM_CHAT_ID").ok();

        if let (Some(token), Some(chat_id_val)) = (token, chat_id_str) {
            let bot = Bot::new(token);
            let chat_id = ChatId(chat_id_val.parse::<i64>()?);
            Ok(Some(Self {
                bot,
                chat_id,
                receiver,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn run(self) {
        info!("Telegram Reporter started.");
        let bot = self.bot.clone();
        let chat_id = self.chat_id;

        // Shared state for the Command Handler to access the latest Summary and Config
        let last_summary: Arc<Mutex<Option<CachedSummary>>> = Arc::new(Mutex::new(None));
        let last_config: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));

        let last_summary_evt = last_summary.clone();
        let last_config_evt = last_config.clone();

        // Spawn Command Handler (REPL)
        let bot_repl = bot.clone();
        tokio::spawn(async move {
            let handler = Update::filter_message().endpoint(move |bot: Bot, msg: Message| {
                let summary_lock = last_summary.clone();
                let config_lock = last_config.clone();
                async move {
                    if let Some(text) = msg.text() {
                        if text == "/status" {
                            let summary = summary_lock.lock().await;
                            let config = config_lock.lock().await;

                            if let Some(s) = &*summary {
                                let mut resp = s.format_status();

                                if let Some(c) = &*config {
                                    // Format config as pretty JSON
                                    if let Ok(config_str) = serde_json::to_string_pretty(c) {
                                        resp.push_str(&format!(
                                            "\n\n⚙️ <b>Config:</b>\n<pre>{}</pre>",
                                            config_str
                                        ));
                                    }
                                }

                                bot.send_message(msg.chat.id, resp)
                                    .parse_mode(teloxide::types::ParseMode::Html)
                                    .await?;
                            } else {
                                bot.send_message(msg.chat.id, "⚠️ No Status Available yet.")
                                    .await?;
                            }
                        }
                    }
                    respond(())
                }
            });

            Dispatcher::builder(bot_repl, handler)
                .enable_ctrlc_handler()
                .build()
                .dispatch()
                .await;
        });

        // Event Loop (Notifications)
        let mut stream = BroadcastStream::new(self.receiver);
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(event) => match event {
                    WSEvent::SpotGridSummary(s) => {
                        let mut lock = last_summary_evt.lock().await;
                        *lock = Some(CachedSummary::SpotGrid(s));
                    }
                    WSEvent::PerpGridSummary(s) => {
                        let mut lock = last_summary_evt.lock().await;
                        *lock = Some(CachedSummary::PerpGrid(s));
                    }
                    WSEvent::Config(c) => {
                        let mut lock = last_config_evt.lock().await;
                        *lock = Some(c);
                    }
                    WSEvent::OrderUpdate(o) => {
                        if o.status == "FILLED" {
                            let icon = if o.side == "Buy" { "🟢" } else { "🔴" };
                            let msg = format!(
                                "{} <b>Order Filled</b>\nSide: {}\nSize: <code>{}</code>\nPrice: <code>{}</code>",
                                icon, o.side, o.size, o.price
                            );
                            if let Err(e) = bot
                                .send_message(chat_id, msg)
                                .parse_mode(teloxide::types::ParseMode::Html)
                                .await
                            {
                                error!("Failed to send Telegram notification: {}", e);
                            }
                        }
                    }
                    WSEvent::Error(e_msg) => {
                        let msg = format!("🔴 <b>Bot Stopped (Error)</b>\nREASON: {}", e_msg);
                        if let Err(e) = bot
                            .send_message(chat_id, msg)
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .await
                        {
                            error!("Failed to send Telegram error notification: {}", e);
                        }
                        // 🔴 CRITICAL: Break the loop to signal we are done and allow main to join
                        break;
                    }
                    _ => {}
                },
                Err(e) => {
                    warn!("Telegram Broadcast Stream Lagged: {}", e);
                }
            }
        }
        info!("Telegram Reporter Shutdown.");
    }
}

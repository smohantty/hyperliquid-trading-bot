use crate::broadcast::types::{StatusSummary, WSEvent};
use anyhow::Result;
use log::{error, info, warn};
use std::env;
use std::sync::Arc;
use teloxide::prelude::*;
use tokio::sync::{broadcast, Mutex};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

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

        // Shared state for the Command Handler to access the latest Summary
        let last_summary: Arc<Mutex<Option<StatusSummary>>> = Arc::new(Mutex::new(None));
        let last_summary_evt = last_summary.clone();

        // Spawn Command Handler (REPL)
        let bot_repl = bot.clone();
        tokio::spawn(async move {
            let handler =
                Update::filter_message().endpoint(move |bot: Bot, msg: Message| {
                    let summary_lock = last_summary.clone();
                    async move {
                        if let Some(text) = msg.text() {
                            if text == "/status" {
                                let summary = summary_lock.lock().await;
                                if let Some(s) = &*summary {
                                    let resp = format!(
                                        "🟢 **{}**\nSymbol: `{}`\n💰 PnL: `{:.2}` (Unrl: `{:.2}`)\n📉 Price: `{:.4}`\n📦 Inv: `{:.4}` @ `{:.4}`",
                                        s.strategy_name, s.symbol, s.realized_pnl, s.unrealized_pnl, s.price, s.inventory.base_size, s.inventory.avg_entry_price
                                    );
                                    bot.send_message(msg.chat.id, resp)
                                        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
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
                Ok(event) => {
                    match event {
                        WSEvent::Summary(s) => {
                            // Update cache
                            let mut lock = last_summary_evt.lock().await;
                            *lock = Some(s);
                        }
                        WSEvent::OrderUpdate(o) => {
                            if o.status == "FILLED" {
                                let icon = if o.side == "Buy" { "🟢" } else { "🔴" };
                                let msg = format!(
                                    "{} **Order Filled**\nSide: {}\nSize: `{}`\nPrice: `{}`",
                                    icon, o.side, o.size, o.price
                                );
                                if let Err(e) = bot
                                    .send_message(chat_id, msg)
                                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                                    .await
                                {
                                    error!("Failed to send Telegram notification: {}", e);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    warn!("Telegram Broadcast Stream Lagged: {}", e);
                }
            }
        }
    }
}

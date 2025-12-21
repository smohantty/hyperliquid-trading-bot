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
                let spacing = format_spacing(s.grid_spacing_pct);
                let total_pnl = s.realized_pnl + s.unrealized_pnl;
                let pnl_emoji = if total_pnl >= 0.0 { "🟢" } else { "🔴" };
                let pnl_sign = if total_pnl >= 0.0 { "+" } else { "" };

                format!(
                    "┌─────────────────────────────┐\n\
                     │  <b>📊 SPOT GRID</b>                │\n\
                     │  <code>{:<6}</code>                      │\n\
                     │  ⏱️ Running for <code>{}</code>        │\n\
                     ├─────────────────────────────┤\n\
                     │  💵 <b>Price</b>     <code>${:>12}</code>  │\n\
                     │  {} <b>PnL</b>       <code>{}{:>11.2}</code>  │\n\
                     │     ├ Real    <code>{:>12.2}</code>  │\n\
                     │     └ Unreal  <code>{:>12.2}</code>  │\n\
                     ├─────────────────────────────┤\n\
                     │  📦 <b>Position</b>  <code>{:>12.4}</code>  │\n\
                     │  📍 <b>Entry</b>     <code>${:>11.2}</code>  │\n\
                     │  💰 <b>Fees</b>      <code>${:>11.2}</code>  │\n\
                     ├─────────────────────────────┤\n\
                     │  🔄 <b>Roundtrips</b>         <code>{:>5}</code>  │\n\
                     │  📐 <b>Grid</b>       <code>{:>3}</code> zones     │\n\
                     │     <code>${} - ${}</code>\n\
                     │     <code>{}</code> spacing\n\
                     └─────────────────────────────┘",
                    s.symbol,
                    s.uptime,
                    format_price(s.price),
                    pnl_emoji,
                    pnl_sign,
                    total_pnl,
                    s.realized_pnl,
                    s.unrealized_pnl,
                    s.position_size,
                    s.avg_entry_price,
                    s.total_fees,
                    s.roundtrips,
                    s.grid_count,
                    format_price(s.range_low),
                    format_price(s.range_high),
                    spacing
                )
            }
            CachedSummary::PerpGrid(s) => {
                let spacing = format_spacing(s.grid_spacing_pct);
                let total_pnl = s.realized_pnl + s.unrealized_pnl;
                let pnl_emoji = if total_pnl >= 0.0 { "🟢" } else { "🔴" };
                let pnl_sign = if total_pnl >= 0.0 { "+" } else { "" };
                let bias_emoji = match s.grid_bias.as_str() {
                    "Long" => "🟢",
                    "Short" => "🔴",
                    _ => "⚪",
                };
                let pos_emoji = match s.position_side.as_str() {
                    "Long" => "📈",
                    "Short" => "📉",
                    _ => "➖",
                };

                format!(
                    "┌─────────────────────────────┐\n\
                     │  <b>📊 PERP GRID</b>                │\n\
                     │  <code>{:<6}</code>  {} <b>{}</b> <code>{}x</code>        │\n\
                     │  ⏱️ Running for <code>{}</code>        │\n\
                     ├─────────────────────────────┤\n\
                     │  💵 <b>Price</b>     <code>${:>12}</code>  │\n\
                     │  {} <b>PnL</b>       <code>{}{:>11.2}</code>  │\n\
                     │     ├ Real    <code>{:>12.2}</code>  │\n\
                     │     └ Unreal  <code>{:>12.2}</code>  │\n\
                     ├─────────────────────────────┤\n\
                     │  {} <b>Position</b>  <code>{:>12.4}</code>  │\n\
                     │     <code>{}</code>\n\
                     │  📍 <b>Entry</b>     <code>${:>11.2}</code>  │\n\
                     │  💰 <b>Fees</b>      <code>${:>11.2}</code>  │\n\
                     │  💳 <b>Margin</b>    <code>${:>11.2}</code>  │\n\
                     ├─────────────────────────────┤\n\
                     │  🔄 <b>Roundtrips</b>         <code>{:>5}</code>  │\n\
                     │  📐 <b>Grid</b>       <code>{:>3}</code> zones     │\n\
                     │     <code>${} - ${}</code>\n\
                     │     <code>{}</code> spacing\n\
                     └─────────────────────────────┘",
                    s.symbol,
                    bias_emoji,
                    s.grid_bias,
                    s.leverage,
                    s.uptime,
                    format_price(s.price),
                    pnl_emoji,
                    pnl_sign,
                    total_pnl,
                    s.realized_pnl,
                    s.unrealized_pnl,
                    pos_emoji,
                    s.position_size.abs(),
                    s.position_side,
                    s.avg_entry_price,
                    s.total_fees,
                    s.margin_balance,
                    s.roundtrips,
                    s.grid_count,
                    format_price(s.range_low),
                    format_price(s.range_high),
                    spacing
                )
            }
        }
    }
}

/// Format price with thousands separator
fn format_price(price: f64) -> String {
    if price >= 1000.0 {
        let whole = price as u64;
        let frac = ((price - whole as f64) * 100.0).round() as u64;
        let formatted = whole
            .to_string()
            .as_bytes()
            .rchunks(3)
            .rev()
            .map(|chunk| std::str::from_utf8(chunk).unwrap())
            .collect::<Vec<_>>()
            .join(",");
        if frac > 0 {
            format!("{}.{:02}", formatted, frac)
        } else {
            formatted
        }
    } else {
        format!("{:.2}", price)
    }
}

/// Format grid spacing: "2.50%" for geometric, "1.80% - 3.20%" for arithmetic
fn format_spacing(spacing: (f64, f64)) -> String {
    let (min, max) = spacing;
    let decimals = if min < 1.0 { 3 } else { 2 };
    let relative_diff = (max - min).abs() / max.max(min);
    if relative_diff < 0.01 {
        format!("{:.decimals$}%", min, decimals = decimals)
    } else {
        format!(
            "{:.decimals$}% - {:.decimals$}%",
            min,
            max,
            decimals = decimals
        )
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

        // Shared state for the Command Handler to access the latest Summary
        let last_summary: Arc<Mutex<Option<CachedSummary>>> = Arc::new(Mutex::new(None));
        let last_summary_evt = last_summary.clone();

        // Spawn Command Handler (REPL)
        let bot_repl = bot.clone();
        tokio::spawn(async move {
            let handler = Update::filter_message().endpoint(move |bot: Bot, msg: Message| {
                let summary_lock = last_summary.clone();
                async move {
                    if let Some(text) = msg.text() {
                        if text == "/status" {
                            let summary = summary_lock.lock().await;

                            if let Some(s) = &*summary {
                                bot.send_message(msg.chat.id, s.format_status())
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

        // Event Loop - only cache summary updates, send error notifications
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

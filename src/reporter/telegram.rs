use crate::broadcast::types::{PerpGridSummary, SpotGridSummary, WSEvent};
use crate::config::broadcast::TelegramConfig;
use crate::config::strategy::StrategyConfig;
use anyhow::Result;
use log::{error, info, warn};
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
    fn format_status(&self, config: &StrategyConfig) -> String {
        match self {
            CachedSummary::SpotGrid(s) => {
                let spacing = format_spacing(s.grid_spacing_pct);
                let total_pnl = s.realized_pnl + s.unrealized_pnl - s.total_fees;
                let pnl_emoji = if total_pnl >= 0.0 { "🟢" } else { "🔴" };
                let pnl_sign = if total_pnl >= 0.0 { "+" } else { "" };

                // Get config specific details
                let (investment, trigger) = match config {
                    StrategyConfig::SpotGrid(c) => (c.total_investment, c.trigger_price),
                    _ => (0.0, None),
                };

                let trigger_str = if let Some(t) = trigger {
                    format!("${:.2}", t)
                } else {
                    "None".to_string()
                };

                let init_entry_str = if let Some(p) = s.initial_entry_price {
                    format!("${:.2}", p)
                } else {
                    "-".to_string()
                };

                format!(
                    "<b>📊 SPOT GRID: {}</b>\n\
                     ⏱️ Running for {}\n\
                     🔄 Matched Trades: <code>{}</code>\n\n\
                     <b>💰 PROFIT & LOSS</b>\n\
                     Total: {} <b>{}{:.2}</b>\n\
                     Realized: <b>{:.2}</b>\n\
                     Unrealized: <b>{:.2}</b>\n\
                     Fees: <code>${:.2}</code>\n\n\
                     <b>📦 POSITION</b>\n\
                     Size: <code>{:.4}</code>\n\
                     Init Entry: <code>{}</code>\n\
                     Avg Entry: <code>${:.2}</code>\n\
                     Quote Bal: <code>${:.2}</code>\n\n\
                     <b>📐 GRID CONFIG</b>\n\
                     Range: <code>${} - ${}</code>\n\
                     Zones: <code>{}</code> ({} spacing)\n\
                     Trigger: <code>{}</code>\n\
                     Invest: <code>${:.2}</code>",
                    s.symbol,
                    s.uptime,
                    s.roundtrips,
                    pnl_emoji,
                    pnl_sign,
                    total_pnl,
                    s.realized_pnl,
                    s.unrealized_pnl,
                    s.total_fees,
                    s.position_size,
                    init_entry_str,
                    s.avg_entry_price,
                    s.quote_balance,
                    format_price(s.range_low),
                    format_price(s.range_high),
                    s.grid_count,
                    spacing,
                    trigger_str,
                    investment
                )
            }
            CachedSummary::PerpGrid(s) => {
                let spacing = format_spacing(s.grid_spacing_pct);
                let total_pnl = s.realized_pnl + s.unrealized_pnl - s.total_fees;
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

                // Get config specific details
                let (investment, trigger, is_isolated) = match config {
                    StrategyConfig::PerpGrid(c) => {
                        (c.total_investment, c.trigger_price, c.is_isolated)
                    }
                    _ => (0.0, None, false),
                };

                let trigger_str = if let Some(t) = trigger {
                    format!("${:.2}", t)
                } else {
                    "None".to_string()
                };

                let init_entry_str = if let Some(p) = s.initial_entry_price {
                    format!("${:.2}", p)
                } else {
                    "-".to_string()
                };

                let margin_mode = if is_isolated { "Isolated" } else { "Cross" };

                format!(
                    "<b>📊 PERP GRID: {}</b>\n\
                     {} <b>{}</b> ({}x)\n\
                     ⏱️ Running for {}\n\
                     🔄 Matched Trades: <code>{}</code>\n\n\
                     <b>💰 PROFIT & LOSS</b>\n\
                     Total: {} <b>{}{:.2}</b>\n\
                     Realized: <b>{:.2}</b>\n\
                     Unrealized: <b>{:.2}</b>\n\
                     Fees: <code>${:.2}</code>\n\n\
                     <b>📦 POSITION</b>\n\
                     {} <b>{}</b>\n\
                     Size: <code>{:.4}</code>\n\
                     Init Entry: <code>{}</code>\n\
                     Avg Entry: <code>${:.2}</code>\n\
                     Margin: <code>${:.2}</code>\n\n\
                     <b>📐 GRID CONFIG</b>\n\
                     Range: <code>${} - ${}</code>\n\
                     Zones: <code>{}</code> ({} spacing)\n\
                     Trigger: <code>{}</code>\n\
                     Mode: <code>{}</code>\n\
                     Invest: <code>${:.2}</code>",
                    s.symbol,
                    bias_emoji,
                    s.grid_bias,
                    s.leverage,
                    s.uptime,
                    s.roundtrips,
                    pnl_emoji,
                    pnl_sign,
                    total_pnl,
                    s.realized_pnl,
                    s.unrealized_pnl,
                    s.total_fees,
                    pos_emoji,
                    s.position_side,
                    s.position_size.abs(),
                    init_entry_str,
                    s.avg_entry_price,
                    s.margin_balance,
                    format_price(s.range_low),
                    format_price(s.range_high),
                    s.grid_count,
                    spacing,
                    trigger_str,
                    margin_mode,
                    investment
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
    config: StrategyConfig,
}

impl TelegramReporter {
    pub fn new(
        receiver: broadcast::Receiver<WSEvent>,
        config: TelegramConfig,
        strategy_config: StrategyConfig,
    ) -> Result<Self> {
        let bot = Bot::new(config.bot_token);
        let chat_id = ChatId(config.chat_id.parse::<i64>()?);
        Ok(Self {
            bot,
            chat_id,
            receiver,
            config: strategy_config,
        })
    }

    pub async fn run(self) {
        info!("Telegram Reporter started.");
        let bot = self.bot.clone();
        let chat_id = self.chat_id;
        let strategy_config = Arc::new(self.config);

        // Shared state for the Command Handler to access the latest Summary
        let last_summary: Arc<Mutex<Option<CachedSummary>>> = Arc::new(Mutex::new(None));
        let last_summary_evt = last_summary.clone();

        // Spawn Command Handler (REPL)
        let bot_repl = bot.clone();
        let strategy_config_repl = strategy_config.clone();

        tokio::spawn(async move {
            let handler = Update::filter_message().endpoint(move |bot: Bot, msg: Message| {
                let summary_lock = last_summary.clone();
                let config = strategy_config_repl.clone();

                async move {
                    if let Some(text) = msg.text() {
                        if text == "/status" {
                            let summary = summary_lock.lock().await;

                            if let Some(s) = &*summary {
                                bot.send_message(msg.chat.id, s.format_status(&config))
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

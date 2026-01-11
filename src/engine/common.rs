//! Common engine utilities shared between live and simulation engines.

use crate::engine::context::{MarketInfo, StrategyContext};
use anyhow::{anyhow, Result};
use ethers::types::H160;
use hyperliquid_rust_sdk::{BaseUrl, InfoClient};
use std::collections::HashMap;
use tracing::{error, info};

/// Create an InfoClient based on network configuration.
pub async fn setup_info_client(network: &str) -> Result<InfoClient> {
    let base_url = if network == "mainnet" {
        BaseUrl::Mainnet
    } else {
        BaseUrl::Testnet
    };
    InfoClient::with_reconnect(None, Some(base_url))
        .await
        .map_err(|e| anyhow!("Failed to connect InfoClient: {}", e))
}

/// Load market metadata (spot and perp) from the exchange.
pub async fn load_metadata(
    info_client: &mut InfoClient,
    log_prefix: &str,
) -> Result<HashMap<String, MarketInfo>> {
    info!("{}Fetching market metadata...", log_prefix);
    let mut markets = HashMap::new();

    // --- Fetch Spot Metadata ---
    match info_client.spot_meta().await {
        Ok(spot_meta) => {
            let index_to_token: HashMap<_, _> =
                spot_meta.tokens.iter().map(|t| (t.index, t)).collect();
            for asset in spot_meta.universe {
                if asset.tokens.len() >= 2 {
                    if let (Some(base), Some(quote)) = (
                        index_to_token.get(&asset.tokens[0]),
                        index_to_token.get(&asset.tokens[1]),
                    ) {
                        let symbol = format!("{}/{}", base.name, quote.name);
                        let coin = asset.name.clone();
                        let asset_index = asset.index as u32;
                        let sz_decimals = base.sz_decimals as u32;
                        let price_decimals = 8u32.saturating_sub(sz_decimals);

                        let info = MarketInfo::new(
                            symbol.clone(),
                            coin,
                            asset_index,
                            sz_decimals,
                            price_decimals,
                        );
                        markets.insert(symbol, info);
                    }
                }
            }
        }
        Err(e) => error!("{}Failed to fetch spot metadata: {}", log_prefix, e),
    }

    // --- Fetch Perp Metadata ---
    match info_client.meta().await {
        Ok(meta) => {
            for (i, asset) in meta.universe.iter().enumerate() {
                let symbol = asset.name.clone();
                let coin = symbol.clone();
                let asset_index = i as u32;
                let sz_decimals = asset.sz_decimals;
                let price_decimals = 6u32.saturating_sub(sz_decimals);

                let info = MarketInfo::new(
                    symbol.clone(),
                    coin,
                    asset_index,
                    sz_decimals,
                    price_decimals,
                );
                markets.insert(symbol, info);
            }
        }
        Err(e) => error!("{}Failed to fetch perp metadata: {}", log_prefix, e),
    }

    Ok(markets)
}

/// Fetch spot and perp balances from the exchange.
pub async fn fetch_balances(
    info_client: &mut InfoClient,
    user_address: H160,
    ctx: &mut StrategyContext,
    log_prefix: &str,
) {
    // Fetch Spot Balances
    match info_client.user_token_balances(user_address).await {
        Ok(balances) => {
            for balance in balances.balances {
                let total: f64 = balance.total.parse().unwrap_or(0.0);
                let hold: f64 = balance.hold.parse().unwrap_or(0.0);
                let available = total - hold;
                ctx.update_spot_balance(balance.coin, total, available);
            }
        }
        Err(e) => error!("{}Failed to fetch spot balances: {}", log_prefix, e),
    }

    // Fetch Perp Balances
    match info_client.user_state(user_address).await {
        Ok(user_state) => {
            let available = user_state.withdrawable.parse().unwrap_or(0.0);
            let total = user_state
                .margin_summary
                .account_value
                .parse()
                .unwrap_or(0.0);
            ctx.update_perp_balance("USDC".to_string(), total, available);
        }
        Err(e) => error!("{}Failed to fetch perp balances: {}", log_prefix, e),
    }
}

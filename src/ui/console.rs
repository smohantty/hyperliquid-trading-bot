//! Console renderer for simulation dry-run output.

use crate::broadcast::types::{GridState, PerpGridSummary, SpotGridSummary, StrategySummary};
use crate::config::strategy::StrategyConfig;
use crate::model::OrderRequest;

/// Console renderer for simulation dry-run reports.
pub struct ConsoleRenderer;

impl ConsoleRenderer {
    /// Render a complete dry-run report to stdout.
    pub fn render(
        config: &StrategyConfig,
        summary: Option<&StrategySummary>,
        grid: Option<&GridState>,
        orders: &[OrderRequest],
        current_price: Option<f64>,
    ) {
        println!();
        println!("{}", "=".repeat(60));
        println!(" SIMULATION DRY RUN REPORT");
        println!("{}", "=".repeat(60));

        // Section 1: Grid State
        if let Some(g) = grid {
            println!();
            Self::render_grid(g);
        }

        // Section 2: Proposed Actions
        println!();
        println!("{}", "-".repeat(60));
        Self::render_action_plan(orders);

        // Section 3: Configuration & Summary
        println!();
        println!("{}", "=".repeat(60));
        let grid_len = grid.map(|g| g.zones.len());
        Self::render_config(config, grid_len);

        if let Some(price) = current_price {
            println!();
            println!("Current Price: {:.6}", price);
        }

        println!();
        println!("{}", "-".repeat(60));
        if let Some(s) = summary {
            Self::render_summary(s);
        }

        println!();
        println!("{}", "=".repeat(60));
        println!();
    }

    /// Render strategy configuration.
    fn render_config(config: &StrategyConfig, grid_len: Option<usize>) {
        println!("CONFIGURATION");

        match config {
            StrategyConfig::SpotGrid(c) => {
                println!("Symbol:      {}", c.symbol);
                println!("Type:        spot_grid");
                println!("Grid Type:   {:?}", c.grid_type);
                println!("Total Inv:   {:.3}", c.total_investment);

                if let Some(spread) = c.spread_bips {
                    println!("Spread:      {} bips", spread);
                }

                if let Some(count) = c.grid_count {
                    println!("Grid Count:  {}", count);
                } else if let Some(len) = grid_len {
                    println!("Grid Count:  {}", len);
                }

                println!("Range:       {:.6} - {:.6}", c.lower_price, c.upper_price);

                if let Some(trigger) = c.trigger_price {
                    println!("Trigger:     {:.6}", trigger);
                }
            }
            StrategyConfig::PerpGrid(c) => {
                println!("Symbol:      {}", c.symbol);
                println!("Type:        perp_grid");
                println!("Grid Type:   {:?}", c.grid_type);
                println!("Grid Bias:   {:?}", c.grid_bias);
                println!("Total Inv:   {:.3}", c.total_investment);
                println!("Leverage:    {}x", c.leverage);

                if let Some(spread) = c.spread_bips {
                    println!("Spread:      {} bips", spread);
                }

                // grid_count is u32 (not Option) for PerpGridConfig
                println!("Grid Count:  {}", c.grid_count);

                println!("Range:       {:.6} - {:.6}", c.lower_price, c.upper_price);

                if let Some(trigger) = c.trigger_price {
                    println!("Trigger:     {:.6}", trigger);
                }
            }
        }
    }

    /// Render strategy summary.
    fn render_summary(summary: &StrategySummary) {
        match summary {
            StrategySummary::SpotGrid(s) => Self::render_spot_summary(s),
            StrategySummary::PerpGrid(s) => Self::render_perp_summary(s),
        }
    }

    fn render_spot_summary(s: &SpotGridSummary) {
        println!("STRATEGY: {}", s.symbol);
        println!("State:    {}", s.state);
        println!("Type:     SPOT GRID");

        // Grid spacing
        let (min_pct, max_pct) = s.grid_spacing_pct;
        if (min_pct - max_pct).abs() < 0.001 {
            println!("Spacing:  {:.3}%", min_pct);
        } else {
            println!("Spacing:  {:.3}% - {:.3}%", min_pct, max_pct);
        }

        // Parse symbol for asset names
        let (base, _quote) = parse_symbol(&s.symbol);
        println!(
            "Balance:  {:.6} {} | {:.3} USDC",
            s.base_balance, base, s.quote_balance
        );

        println!("Matched Profit:  {:.4}", s.matched_profit);
        println!("Net PnL:  {:.4}", s.total_profit);
    }

    fn render_perp_summary(s: &PerpGridSummary) {
        println!("STRATEGY: {}", s.symbol);
        println!("State:    {}", s.state);
        println!("Type:     PERP GRID ({})", s.grid_bias);

        // Grid spacing
        let (min_pct, max_pct) = s.grid_spacing_pct;
        if (min_pct - max_pct).abs() < 0.001 {
            println!("Spacing:  {:.3}%", min_pct);
        } else {
            println!("Spacing:  {:.3}% - {:.3}%", min_pct, max_pct);
        }

        println!("Margin:   {:.3} USDC", s.margin_balance);
        println!("Position: {:.6} ({})", s.position_size, s.position_side);
        println!("Matched Profit:  {:.4}", s.matched_profit);
        println!("Net PnL:  {:.4}", s.unrealized_pnl);
        println!("Leverage: {}x", s.leverage);
    }

    /// Render grid state.
    fn render_grid(g: &GridState) {
        println!("GRID STATE ({} Zones)", g.zones.len());
        println!(
            "{:<4} | {:<25} | {:<10} | {:<12} | {:<12} | {:<6} | STATUS",
            "IDX", "RANGE", "SPD %", "SIZE", "EXP_PNL", "SIDE"
        );
        println!("{}", "-".repeat(100));

        // Limit display to first 50 + last 50 if too many zones
        let display_zones: Vec<_> = if g.zones.len() > 100 {
            g.zones
                .iter()
                .take(50)
                .chain(g.zones.iter().skip(g.zones.len() - 50))
                .collect()
        } else {
            g.zones.iter().collect()
        };

        for z in display_zones {
            let range = format!("{:.6}-{:.6}", z.buy_price, z.sell_price);
            let status = if z.has_order {
                if z.is_reduce_only {
                    "ACTIVE (RO)"
                } else {
                    "ACTIVE"
                }
            } else {
                "WAITING"
            };

            // Calculate spread and expected PnL
            let spread_pct = if z.buy_price > 0.0 {
                ((z.sell_price - z.buy_price) / z.buy_price) * 100.0
            } else {
                0.0
            };
            let exp_pnl = (z.sell_price - z.buy_price) * z.size;

            println!(
                "{:<4} | {:<25} | {:<10.2} | {:<12.6} | {:<12.4} | {:<6} | {}",
                z.index, range, spread_pct, z.size, exp_pnl, z.order_side, status
            );
        }

        if g.zones.len() > 100 {
            println!("... (Hiding {} zones) ...", g.zones.len() - 100);
        }
    }

    /// Render proposed actions (orders).
    fn render_action_plan(orders: &[OrderRequest]) {
        println!("PROPOSED ACTIONS (What would happen next):");

        if orders.is_empty() {
            println!("  [WAIT] No immediate orders generated.");
            return;
        }

        for order in orders {
            match order {
                OrderRequest::Limit {
                    side,
                    price,
                    sz,
                    reduce_only,
                    ..
                } => {
                    let ro_tag = if *reduce_only { " [ReduceOnly]" } else { "" };
                    println!("  [ORDER] {:?} {:.6} @ {:.6}{}", side, sz, price, ro_tag);
                }
                OrderRequest::Market { side, sz, .. } => {
                    println!("  [ORDER] {:?} {:.6} @ MARKET", side, sz);
                }
                OrderRequest::Cancel { cloid } => {
                    println!("  [CANCEL] CLOID {}", cloid);
                }
            }
        }
    }
}

/// Parse symbol into (base, quote).
fn parse_symbol(symbol: &str) -> (String, String) {
    if let Some(idx) = symbol.find('/') {
        (symbol[..idx].to_string(), symbol[idx + 1..].to_string())
    } else {
        (symbol.to_string(), "USDC".to_string())
    }
}

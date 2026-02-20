use super::types::GridType;
use std::time::Duration;

/// Format a Duration as a human-readable uptime string.
///
/// Examples: "2d 14h 30m", "5h 45m", "30m 15s", "45s"
pub fn format_uptime(duration: Duration) -> String {
    let total_secs = duration.as_secs();

    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Checks if the current price has crossed the trigger price based on the initial start price.
///
/// * `current_price` - The latest market price.
/// * `trigger_price` - The target price to trigger the strategy.
/// * `start_price` - The price when the strategy entered the WaitingForTrigger state.
///
/// Returns `true` if triggered, otherwise `false`.
pub fn check_trigger(current_price: f64, trigger_price: f64, start_price: f64) -> bool {
    if start_price < trigger_price {
        // Waiting for price to go UP to trigger
        if current_price >= trigger_price {
            return true;
        }
    } else {
        // Waiting for price to go DOWN to trigger
        if current_price <= trigger_price {
            return true;
        }
    }
    false
}

/// Calculates the grid levels (prices) based on the configuration.
///
/// * `grid_type` - Arithmetic or Geometric.
/// * `grid_range_low` - The bottom of the grid range.
/// * `grid_range_high` - The top of the grid range.
/// * `grid_count` - The number of levels to generate.
///
/// Returns a `Vec<f64>` containing the calculated prices.
pub fn calculate_grid_prices(
    grid_type: GridType,
    grid_range_low: f64,
    grid_range_high: f64,
    grid_count: u32,
) -> Vec<f64> {
    let mut prices = Vec::with_capacity(grid_count as usize);

    match grid_type {
        GridType::Arithmetic => {
            let step = (grid_range_high - grid_range_low) / (grid_count as f64 - 1.0);
            for i in 0..grid_count {
                let price = grid_range_low + (i as f64 * step);
                prices.push(price);
            }
        }
        GridType::Geometric => {
            let ratio = (grid_range_high / grid_range_low).powf(1.0 / (grid_count as f64 - 1.0));
            for i in 0..grid_count {
                let price = grid_range_low * ratio.powi(i as i32);
                prices.push(price);
            }
        }
    }
    prices
}

/// Calculates the grid levels (prices) based on spread in basis points.
///
/// * `grid_range_low` - The bottom of the grid range.
/// * `grid_range_high` - The top of the grid range.
/// * `spread_bips` - Spread between levels in basis points (1 bip = 0.01%).
///
/// Returns a `Vec<f64>` containing the calculated prices.
pub fn calculate_grid_prices_by_spread(
    grid_range_low: f64,
    grid_range_high: f64,
    spread_bips: f64,
) -> Vec<f64> {
    let mut prices = Vec::new();
    if grid_range_low >= grid_range_high {
        return prices;
    }

    // 1 bip = 0.01% = 0.0001
    // ratio = 1 + (spread_bips / 10000)
    let ratio = 1.0 + (spread_bips / 10000.0);

    let mut current_price = grid_range_low;
    while current_price <= grid_range_high {
        prices.push(current_price);
        current_price *= ratio;
    }

    prices
}

/// Calculate grid spacing as percentage (min, max).
///
/// * `grid_type` - Arithmetic or Geometric.
/// * `grid_range_low` - The bottom of the grid range.
/// * `grid_range_high` - The top of the grid range.
/// * `grid_count` - The number of grid zones.
///
/// Returns `(min%, max%)`:
/// - For geometric: both values are the same (constant ratio).
/// - For arithmetic: min is at highest price, max is at lowest price.
pub fn calculate_grid_spacing_pct(
    grid_type: &GridType,
    grid_range_low: f64,
    grid_range_high: f64,
    grid_count: u32,
) -> (f64, f64) {
    let n = grid_count as f64;

    match grid_type {
        GridType::Geometric => {
            // Geometric: constant ratio between levels
            // ratio = (upper/lower)^(1/n)
            // spacing_pct = (ratio - 1) * 100
            let ratio = (grid_range_high / grid_range_low).powf(1.0 / n);
            let spacing_pct = (ratio - 1.0) * 100.0;
            (spacing_pct, spacing_pct)
        }
        GridType::Arithmetic => {
            // Arithmetic: constant dollar spacing
            // spacing = (upper - lower) / n
            // At lower prices, the % is higher; at higher prices, the % is lower
            let spacing = (grid_range_high - grid_range_low) / n;
            let min_pct = (spacing / grid_range_high) * 100.0; // Smallest % at highest price
            let max_pct = (spacing / grid_range_low) * 100.0; // Largest % at lowest price
            (min_pct, max_pct)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_trigger_up() {
        // Start below trigger
        let start = 100.0;
        let trigger = 110.0;

        // Not triggered yet
        assert_eq!(check_trigger(105.0, trigger, start), false);

        // Triggered
        assert_eq!(check_trigger(110.0, trigger, start), true);
        assert_eq!(check_trigger(111.0, trigger, start), true);
    }

    #[test]
    fn test_check_trigger_down() {
        // Start above trigger
        let start = 100.0;
        let trigger = 90.0;

        // Not triggered yet
        assert_eq!(check_trigger(95.0, trigger, start), false);

        // Triggered
        assert_eq!(check_trigger(90.0, trigger, start), true);
        assert_eq!(check_trigger(89.0, trigger, start), true);
    }

    #[test]
    fn test_calculate_grid_prices_arithmetic() {
        let prices = calculate_grid_prices(GridType::Arithmetic, 100.0, 200.0, 3);
        assert_eq!(prices.len(), 3);
        assert!((prices[0] - 100.0).abs() < 1e-9);
        assert!((prices[1] - 150.0).abs() < 1e-9); // Midpoint
        assert!((prices[2] - 200.0).abs() < 1e-9);
    }

    #[test]
    fn test_calculate_grid_prices_geometric() {
        // Geometric progression: 100, 200, 400 (ratio = 2.0)
        let prices = calculate_grid_prices(GridType::Geometric, 100.0, 400.0, 3);
        assert_eq!(prices.len(), 3);
        assert!((prices[0] - 100.0).abs() < 1e-9);
        assert!((prices[1] - 200.0).abs() < 1e-9); // 100 * 2
        assert!((prices[2] - 400.0).abs() < 1e-9); // 200 * 2
    }

    #[test]
    fn test_grid_spacing_geometric() {
        // 10 zones from 100 to 200
        let (min, max) = calculate_grid_spacing_pct(&GridType::Geometric, 100.0, 200.0, 10);
        // For geometric, min == max
        assert!((min - max).abs() < 1e-9);
        // ratio = (200/100)^(1/10) = 2^0.1 ≈ 1.0718
        // spacing ≈ 7.18%
        assert!((min - 7.177).abs() < 0.01);
    }

    #[test]
    fn test_grid_spacing_arithmetic() {
        // 10 zones from 100 to 200, step = 10
        let (min, max) = calculate_grid_spacing_pct(&GridType::Arithmetic, 100.0, 200.0, 10);
        // At 200: 10/200 = 5%
        // At 100: 10/100 = 10%
        assert!((min - 5.0).abs() < 1e-9);
        assert!((max - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_format_uptime_seconds() {
        assert_eq!(format_uptime(Duration::from_secs(45)), "45s");
        assert_eq!(format_uptime(Duration::from_secs(0)), "0s");
    }

    #[test]
    fn test_format_uptime_minutes() {
        assert_eq!(format_uptime(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_uptime(Duration::from_secs(3599)), "59m 59s");
    }

    #[test]
    fn test_format_uptime_hours() {
        assert_eq!(format_uptime(Duration::from_secs(3600)), "1h 0m");
        assert_eq!(format_uptime(Duration::from_secs(5400)), "1h 30m");
        assert_eq!(format_uptime(Duration::from_secs(86399)), "23h 59m");
    }

    #[test]
    fn test_format_uptime_days() {
        assert_eq!(format_uptime(Duration::from_secs(86400)), "1d 0h 0m");
        assert_eq!(format_uptime(Duration::from_secs(90061)), "1d 1h 1m");
        assert_eq!(format_uptime(Duration::from_secs(259200)), "3d 0h 0m"); // 3 days
    }
}

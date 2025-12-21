use super::types::GridType;

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
/// * `lower_price` - The bottom of the grid range.
/// * `upper_price` - The top of the grid range.
/// * `grid_count` - The number of levels to generate.
///
/// Returns a `Vec<f64>` containing the calculated prices.
pub fn calculate_grid_prices(
    grid_type: GridType,
    lower_price: f64,
    upper_price: f64,
    grid_count: u32,
) -> Vec<f64> {
    let mut prices = Vec::with_capacity(grid_count as usize);

    match grid_type {
        GridType::Arithmetic => {
            let step = (upper_price - lower_price) / (grid_count as f64 - 1.0);
            for i in 0..grid_count {
                let price = lower_price + (i as f64 * step);
                prices.push(price);
            }
        }
        GridType::Geometric => {
            let ratio = (upper_price / lower_price).powf(1.0 / (grid_count as f64 - 1.0));
            for i in 0..grid_count {
                let price = lower_price * ratio.powi(i as i32);
                prices.push(price);
            }
        }
    }
    prices
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
}

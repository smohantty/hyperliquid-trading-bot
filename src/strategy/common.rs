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
}

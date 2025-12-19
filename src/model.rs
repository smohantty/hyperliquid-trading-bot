use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderRequest {
    Limit {
        symbol: String,
        is_buy: bool,
        price: f64,
        sz: f64,
        reduce_only: bool,
        cloid: Option<u128>,
    },
    Market {
        symbol: String,
        is_buy: bool,
        sz: f64,
        cloid: Option<u128>,
    },
    Cancel {
        cloid: u128,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub u64);

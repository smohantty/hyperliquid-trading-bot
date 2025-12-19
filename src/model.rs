use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderRequest {
    Limit {
        symbol: String,
        is_buy: bool,
        price: f64,
        sz: f64,
        reduce_only: bool,
        cloid: Option<uuid::Uuid>,
    },
    Market {
        symbol: String,
        is_buy: bool,
        sz: f64,
        cloid: Option<uuid::Uuid>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub u64);

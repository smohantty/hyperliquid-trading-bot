// ============================================================
// Strategy Configuration
// ============================================================

export interface StrategyConfig {
    type: 'spot_grid' | 'perp_grid';
    symbol: string;
    upper_price: number;
    lower_price: number;
    grid_type: 'arithmetic' | 'geometric';
    grid_count: number;
    total_investment: number;
    trigger_price?: number | null;
    // Perp specific
    leverage?: number;
    is_isolated?: boolean;
    grid_bias?: 'long' | 'short' | 'neutral';
    sz_decimals?: number;
}

// ============================================================
// Strategy Summaries (High-level metrics)
// ============================================================

export interface SpotGridSummary {
    symbol: string;
    price: number;
    state: string;
    uptime: string; // Human-readable uptime, e.g. "2d 14h 30m"
    position_size: number;
    avg_entry_price: number;
    realized_pnl: number;
    unrealized_pnl: number;
    total_fees: number;
    start_price?: number;
    grid_count: number;
    range_low: number;
    range_high: number;
    grid_spacing_pct: [number, number]; // (min%, max%) - same for geometric, different for arithmetic
    roundtrips: number;
    base_balance: number;
    quote_balance: number;
}

export interface PerpGridSummary {
    symbol: string;
    price: number;
    state: string;
    uptime: string; // Human-readable uptime, e.g. "2d 14h 30m"
    position_size: number;
    position_side: 'Long' | 'Short' | 'Flat';
    avg_entry_price: number;
    realized_pnl: number;
    unrealized_pnl: number;
    total_fees: number;
    leverage: number;
    grid_bias: 'Long' | 'Short' | 'Neutral';
    grid_count: number;
    range_low: number;
    range_high: number;
    grid_spacing_pct: [number, number]; // (min%, max%) - same for geometric, different for arithmetic
    roundtrips: number;
    margin_balance: number;
}

// Union type for any strategy summary
export type StrategySummary =
    | { type: 'spot_grid'; data: SpotGridSummary }
    | { type: 'perp_grid'; data: PerpGridSummary };

// ============================================================
// Grid State (Zone data for CLOB visualization)
// ============================================================

export interface ZoneInfo {
    index: number;
    lower_price: number;
    upper_price: number;
    size: number;
    pending_side: 'Buy' | 'Sell';
    has_order: boolean;
    is_reduce_only: boolean;

    entry_price: number;
    roundtrip_count: number;
}

export interface GridState {
    symbol: string;
    strategy_type: string;
    current_price: number;
    grid_bias?: string;
    zones: ZoneInfo[];
}

// ============================================================
// Order and Market Events
// ============================================================

export interface OrderEvent {
    oid: number;
    cloid?: string | null;
    side: string;
    price: number;
    size: number;
    status: string;
    fee: number;
    is_taker: boolean;
}

export interface MarketEvent {
    price: number;
}

// ============================================================
// WebSocket Event Types
// ============================================================

export type WebSocketEvent =
    | { event_type: 'config'; data: StrategyConfig }
    | { event_type: 'spot_grid_summary'; data: SpotGridSummary }
    | { event_type: 'perp_grid_summary'; data: PerpGridSummary }
    | { event_type: 'grid_state'; data: GridState }
    | { event_type: 'order_update'; data: OrderEvent }
    | { event_type: 'market_update'; data: MarketEvent }
    | { event_type: 'error'; data: string };

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
}

export interface InventoryStats {
    base_size: number;
    avg_entry_price: number;
}

export interface WalletStats {
    base_balance: number;
    quote_balance: number;
}

export type ZoneSide = 'Buy' | 'Sell';
export type ZoneStatusType = 'Open' | 'Idle';

export interface ZoneStatus {
    price: number;
    side: ZoneSide;
    status: ZoneStatusType;
    size: number;
}

export interface SpotGridCustomData {
    grid_count: number;
    range_low: number;
    range_high: number;
    roundtrips: number;
}

export interface PerpGridCustomData {
    leverage: number;
    grid_bias: string;
    long_inventory: number;
    short_inventory: number;
    state: string;
}

export interface StatusSummary {
    strategy_name: string;
    symbol: string;
    realized_pnl: number;
    unrealized_pnl: number;
    total_fees: number;
    inventory: InventoryStats;
    wallet: WalletStats;
    price: number;
    zones: ZoneStatus[];
    custom: SpotGridCustomData | PerpGridCustomData;
}

export interface OrderEvent {
    oid: number;
    cloid?: string | null;
    side: string;
    price: number;
    size: number;
    status: string;
    fee: number;
}

export type WebSocketEvent =
    | { event_type: 'config'; data: StrategyConfig }
    | { event_type: 'summary'; data: StatusSummary }
    | { event_type: 'order_update'; data: OrderEvent }
    | { event_type: 'market_update'; data: { price: number } }
    | { event_type: 'error'; data: string };

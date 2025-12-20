import React from 'react';
import { useBotStore } from '../context/WebSocketContext';

const MetricCard: React.FC<{ label: string; value: string | number; color?: string; subValue?: string }> = ({ label, value, color, subValue }) => (
    <div style={{
        background: 'var(--bg-card)',
        padding: '1rem',
        borderRadius: '8px',
        border: '1px solid var(--border-light)',
        minWidth: '180px'
    }}>
        <div style={{ color: 'var(--text-muted)', fontSize: '0.8rem', marginBottom: '0.5rem' }}>{label}</div>
        <div style={{ fontSize: '1.5rem', fontWeight: 600, color: color || 'var(--text-primary)' }}>{value}</div>
        {subValue && <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginTop: '0.2rem' }}>{subValue}</div>}
    </div>
);

const HeaderMetrics: React.FC = () => {
    const { summary, lastTickTime } = useBotStore();

    if (!summary) return <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--text-muted)' }}>Waiting for strategy data...</div>;

    const pnlColor = (summary.realized_pnl + summary.unrealized_pnl) >= 0 ? 'var(--color-buy)' : 'var(--color-sell)';

    const timeStr = lastTickTime ? new Date(lastTickTime).toLocaleTimeString() : '--:--:--';

    return (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '1rem', marginBottom: '2rem' }}>
            <MetricCard
                label="MARKET PRICE"
                value={`$${summary.price.toFixed(4)}`}
                color="var(--accent-primary)"
                subValue={`${summary.symbol} • ${timeStr}`}
            />
            <MetricCard
                label="TOTAL PnL"
                value={`$${(summary.realized_pnl + summary.unrealized_pnl).toFixed(2)}`}
                color={pnlColor}
                subValue={`Unrealized: $${summary.unrealized_pnl.toFixed(2)}`}
            />
            <MetricCard
                label="WALLET BALANCE"
                value={`$${summary.wallet.quote_balance.toFixed(2)}`}
                subValue={`Base: ${summary.wallet.base_balance.toFixed(4)}`}
            />
            <MetricCard
                label="INVENTORY"
                value={`${summary.inventory.base_size.toFixed(4)}`}
                subValue={`Avg Entry: $${summary.inventory.avg_entry_price.toFixed(4)}`}
            />
        </div>
    );
};

export default HeaderMetrics;

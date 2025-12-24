import React from 'react';
import { useBotStore } from '../context/WebSocketContext';

const MetricCard: React.FC<{
    label: string;
    value: string | number;
    color?: string;
    subValue?: string;
    badge?: { text: string; color: string };
}> = ({ label, value, color, subValue, badge }) => (
    <div style={{
        background: 'var(--bg-card)',
        padding: '1rem',
        borderRadius: '8px',
        border: '1px solid var(--border-light)',
        minWidth: '180px',
        position: 'relative'
    }}>
        {badge && (
            <span style={{
                position: 'absolute',
                top: '0.5rem',
                right: '0.5rem',
                background: badge.color,
                color: '#000',
                fontSize: '0.65rem',
                fontWeight: 700,
                padding: '2px 6px',
                borderRadius: '4px',
                textTransform: 'uppercase'
            }}>
                {badge.text}
            </span>
        )}
        <div style={{ color: 'var(--text-muted)', fontSize: '0.8rem', marginBottom: '0.5rem' }}>{label}</div>
        <div style={{ fontSize: '1.5rem', fontWeight: 600, color: color || 'var(--text-primary)' }}>{value}</div>
        {subValue && <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginTop: '0.2rem' }}>{subValue}</div>}
    </div>
);

const HeaderMetrics: React.FC = () => {
    const { summary, lastTickTime } = useBotStore();

    if (!summary) {
        return (
            <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--text-muted)' }}>
                Waiting for strategy data...
            </div>
        );
    }

    const timeStr = lastTickTime ? new Date(lastTickTime).toLocaleTimeString() : '--:--:--';

    // Render based on strategy type
    if (summary.type === 'spot_grid') {
        const s = summary.data;
        const totalPnl = s.realized_pnl + s.unrealized_pnl - s.total_fees;
        const pnlColor = totalPnl >= 0 ? 'var(--color-buy)' : 'var(--color-sell)';

        return (
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '1rem', marginBottom: '2rem' }}>
                <MetricCard
                    label="MARKET PRICE"
                    value={`$${s.price.toFixed(4)}`}
                    color="var(--accent-primary)"
                    subValue={`${s.symbol} • ${timeStr}`}
                    badge={{ text: 'SPOT', color: 'var(--accent-primary)' }}
                />
                <MetricCard
                    label="TOTAL PnL"
                    value={`$${totalPnl.toFixed(2)}`}
                    color={pnlColor}
                    subValue={`Unrealized: $${s.unrealized_pnl.toFixed(2)} • Fees: $${s.total_fees.toFixed(2)}`}
                />
                <MetricCard
                    label="POSITION"
                    value={`${s.position_size.toFixed(4)}`}
                    subValue={`Avg Entry: $${s.avg_entry_price.toFixed(4)}`}
                />
                <MetricCard
                    label="WALLET"
                    value={`$${s.quote_balance.toFixed(2)}`}
                    subValue={`Base: ${s.base_balance.toFixed(4)}`}
                />
                <MetricCard
                    label="GRID"
                    value={`${s.grid_count} zones`}
                    subValue={`${s.range_low.toFixed(2)} - ${s.range_high.toFixed(2)} • ${s.roundtrips} roundtrips`}
                />
            </div>
        );
    }

    // Perp Grid
    const s = summary.data;
    const totalPnl = s.realized_pnl + s.unrealized_pnl - s.total_fees;
    const pnlColor = totalPnl >= 0 ? 'var(--color-buy)' : 'var(--color-sell)';

    const positionIcon = s.position_side === 'Long' ? '📈' : s.position_side === 'Short' ? '📉' : '➖';
    const positionColor = s.position_side === 'Long' ? 'var(--color-buy)' : s.position_side === 'Short' ? 'var(--color-sell)' : 'var(--text-primary)';

    const biasColor = s.grid_bias === 'Long' ? 'var(--color-buy)' : s.grid_bias === 'Short' ? 'var(--color-sell)' : 'var(--accent-primary)';

    return (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '1rem', marginBottom: '2rem' }}>
            <MetricCard
                label="MARKET PRICE"
                value={`$${s.price.toFixed(4)}`}
                color="var(--accent-primary)"
                subValue={`${s.symbol} • ${timeStr}`}
                badge={{ text: `PERP ${s.leverage}x`, color: biasColor }}
            />
            <MetricCard
                label="TOTAL PnL"
                value={`$${totalPnl.toFixed(2)}`}
                color={pnlColor}
                subValue={`Unrealized: $${s.unrealized_pnl.toFixed(2)} • Fees: $${s.total_fees.toFixed(2)}`}
            />
            <MetricCard
                label="POSITION"
                value={`${positionIcon} ${Math.abs(s.position_size).toFixed(4)}`}
                color={positionColor}
                subValue={`${s.position_side} @ $${s.avg_entry_price.toFixed(4)}`}
            />
            <MetricCard
                label="MARGIN"
                value={`$${s.margin_balance.toFixed(2)}`}
                subValue={`Bias: ${s.grid_bias}`}
            />
            <MetricCard
                label="GRID"
                value={`${s.grid_count} zones`}
                subValue={`${s.range_low.toFixed(2)} - ${s.range_high.toFixed(2)} • ${s.roundtrips} roundtrips`}
            />
        </div>
    );
};

export default HeaderMetrics;

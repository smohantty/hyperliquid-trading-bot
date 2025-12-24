import React from 'react';
import { useBotStore } from '../context/WebSocketContext';
import Tooltip from './Tooltip';

const SummaryCard: React.FC = () => {
    const { summary, lastTickTime } = useBotStore();

    if (!summary) {
        return (
            <div style={{
                background: 'var(--bg-secondary)',
                borderRadius: '8px',
                border: '1px solid var(--border-color)',
                padding: '40px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--text-tertiary)'
            }}>
                Waiting for strategy data...
            </div>
        );
    }

    const timeStr = lastTickTime ? new Date(lastTickTime).toLocaleTimeString() : '--:--:--';
    const isPerp = summary.type === 'perp_grid';
    const s = summary.data;

    const totalPnl = s.realized_pnl + s.unrealized_pnl;
    const pnlColor = totalPnl >= 0 ? 'var(--color-buy)' : 'var(--color-sell)';
    const pnlSign = totalPnl >= 0 ? '+' : '';

    if (isPerp) {
        const perpData = summary.data as typeof summary.data & {
            position_side: string;
            leverage: number;
            grid_bias: string;
            margin_balance: number;
        };

        const positionColor = perpData.position_side === 'Long' ? 'var(--color-buy)' :
            perpData.position_side === 'Short' ? 'var(--color-sell)' :
                'var(--text-tertiary)';

        return (
            <div style={{
                background: 'var(--bg-secondary)',
                borderRadius: '8px',
                border: '1px solid var(--border-color)',
                overflow: 'hidden'
            }}>
                {/* Header */}
                <div style={{
                    padding: '16px 20px',
                    borderBottom: '1px solid var(--border-color)',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center'
                }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <span style={{ fontSize: '18px', fontWeight: 600 }}>{s.symbol}</span>
                        <span style={{
                            background: perpData.grid_bias === 'Long' ? 'rgba(14, 203, 129, 0.15)' :
                                perpData.grid_bias === 'Short' ? 'rgba(246, 70, 93, 0.15)' :
                                    'rgba(240, 185, 11, 0.15)',
                            color: perpData.grid_bias === 'Long' ? 'var(--color-buy)' :
                                perpData.grid_bias === 'Short' ? 'var(--color-sell)' :
                                    'var(--accent-yellow)',
                            padding: '4px 8px',
                            borderRadius: '4px',
                            fontSize: '11px',
                            fontWeight: 600
                        }}>
                            {perpData.leverage}x {perpData.grid_bias.toUpperCase()}
                        </span>
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                            ⏱️ {s.uptime}
                        </span>
                        <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>{timeStr}</span>
                    </div>
                </div>

                {/* Price & PnL */}
                <div style={{ display: 'flex', borderBottom: '1px solid var(--border-color)' }}>
                    <div style={{ flex: 1, padding: '20px', borderRight: '1px solid var(--border-color)' }}>
                        <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginBottom: '8px', textTransform: 'uppercase' }}>
                            Market Price
                        </div>
                        <div style={{ fontSize: '24px', fontWeight: 600, fontFamily: 'var(--font-mono)' }}>
                            ${s.price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                        </div>
                    </div>
                    <div style={{ flex: 1, padding: '20px' }}>
                        <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginBottom: '8px', textTransform: 'uppercase' }}>
                            Total PnL
                        </div>
                        <div style={{ fontSize: '24px', fontWeight: 600, color: pnlColor, fontFamily: 'var(--font-mono)' }}>
                            {pnlSign}${Math.abs(totalPnl).toFixed(2)}
                        </div>
                        <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginTop: '4px' }}>
                            Realized: ${s.realized_pnl.toFixed(2)} · Unrealized: ${s.unrealized_pnl.toFixed(2)}
                        </div>
                    </div>
                </div>

                {/* Stats Row */}
                <div style={{ display: 'flex', borderBottom: '1px solid var(--border-color)' }}>
                    <StatItem label="Position" value={Math.abs(perpData.position_size).toFixed(4)} subValue={perpData.position_side} valueColor={positionColor} />
                    <StatItem label="Avg Entry" value={`$${perpData.avg_entry_price.toFixed(2)}`} />
                    <StatItem label="Margin" value={`$${perpData.margin_balance.toFixed(2)}`} />
                    <StatItem label="Fees" value={`$${s.total_fees.toFixed(2)}`} valueColor="var(--color-sell)" isLast />
                </div>

                {/* Footer */}
                <div style={{
                    padding: '12px 20px',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    fontSize: '12px',
                    color: 'var(--text-secondary)'
                }}>
                    <span>
                        <Tooltip content="Active Grid Levels">
                            <span style={{ cursor: 'help' }}>
                                Grid: {s.grid_count} zones
                            </span>
                        </Tooltip>
                        {' · '}
                        <Tooltip content="Price Range (Min - Max)">
                            <span style={{ cursor: 'help' }}>
                                ${s.range_low.toLocaleString()} - ${s.range_high.toLocaleString()}
                            </span>
                        </Tooltip>
                        {' · '}
                        <Tooltip content="Grid Spacing Percentage (Distance between levels)">
                            <span style={{ color: 'var(--accent-yellow)', cursor: 'help' }}>
                                {formatSpacing(s.grid_spacing_pct)}
                            </span>
                        </Tooltip>
                    </span>
                    <span style={{ color: 'var(--accent-yellow)', fontWeight: 500 }}>
                        {s.roundtrips} roundtrips
                    </span>
                </div>
            </div>
        );
    }

    // Spot Grid
    const spotData = summary.data as typeof summary.data & {
        base_balance: number;
        quote_balance: number;
    };

    return (
        <div style={{
            background: 'var(--bg-secondary)',
            borderRadius: '8px',
            border: '1px solid var(--border-color)',
            overflow: 'hidden'
        }}>
            {/* Header */}
            <div style={{
                padding: '16px 20px',
                borderBottom: '1px solid var(--border-color)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center'
            }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <span style={{ fontSize: '18px', fontWeight: 600 }}>{s.symbol}</span>
                    <span style={{
                        background: 'rgba(30, 144, 255, 0.15)',
                        color: 'var(--accent-blue)',
                        padding: '4px 8px',
                        borderRadius: '4px',
                        fontSize: '11px',
                        fontWeight: 600
                    }}>
                        SPOT GRID
                    </span>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                        ⏱️ {s.uptime}
                    </span>
                    <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>{timeStr}</span>
                </div>
            </div>

            {/* Price & PnL */}
            <div style={{ display: 'flex', borderBottom: '1px solid var(--border-color)' }}>
                <div style={{ flex: 1, padding: '20px', borderRight: '1px solid var(--border-color)' }}>
                    <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginBottom: '8px', textTransform: 'uppercase' }}>
                        Market Price
                    </div>
                    <div style={{ fontSize: '24px', fontWeight: 600, fontFamily: 'var(--font-mono)' }}>
                        ${s.price.toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 4 })}
                    </div>
                </div>
                <div style={{ flex: 1, padding: '20px' }}>
                    <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginBottom: '8px', textTransform: 'uppercase' }}>
                        Total PnL
                    </div>
                    <div style={{ fontSize: '24px', fontWeight: 600, color: pnlColor, fontFamily: 'var(--font-mono)' }}>
                        {pnlSign}${Math.abs(totalPnl).toFixed(2)}
                    </div>
                    <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginTop: '4px' }}>
                        Realized: ${s.realized_pnl.toFixed(2)} · Unrealized: ${s.unrealized_pnl.toFixed(2)}
                    </div>
                </div>
            </div>

            {/* Stats Row */}
            <div style={{ display: 'flex', borderBottom: '1px solid var(--border-color)' }}>
                <StatItem label="Position" value={spotData.position_size.toFixed(4)} />
                <StatItem label="Avg Entry" value={`$${spotData.avg_entry_price.toFixed(4)}`} />
                <StatItem label="Quote" value={`$${spotData.quote_balance.toFixed(2)}`} />
                <StatItem label="Fees" value={`$${s.total_fees.toFixed(2)}`} valueColor="var(--color-sell)" isLast />
            </div>

            {/* Footer */}
            <div style={{
                padding: '12px 20px',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                fontSize: '12px',
                color: 'var(--text-secondary)'
            }}>
                <span>
                    <Tooltip content="Active Grid Levels">
                        <span style={{ cursor: 'help' }}>
                            Grid: {s.grid_count} zones
                        </span>
                    </Tooltip>
                    {' · '}
                    <Tooltip content="Price Range (Min - Max)">
                        <span style={{ cursor: 'help' }}>
                            ${s.range_low.toFixed(2)} - ${s.range_high.toFixed(2)}
                        </span>
                    </Tooltip>
                    {' · '}
                    <Tooltip content="Grid Spacing Percentage (Distance between levels)">
                        <span style={{ color: 'var(--accent-yellow)', cursor: 'help' }} >
                            {formatSpacing(s.grid_spacing_pct)}
                        </span>
                    </Tooltip>
                </span>
                <span style={{ color: 'var(--accent-yellow)', fontWeight: 500 }}>
                    {s.roundtrips} roundtrips
                </span>
            </div>
        </div>
    );
};

// Format grid spacing: "2.50%" for geometric, "0.167% - 0.172%" for arithmetic
const formatSpacing = (spacing: [number, number]): string => {
    const [min, max] = spacing;

    // Determine precision based on value magnitude
    // Small values need more precision to show meaningful differences
    const decimals = min < 1 ? 3 : 2;

    // Use relative difference check: if difference is < 1% of the value, treat as same
    const relativeDiff = Math.abs(max - min) / Math.max(min, max);
    if (relativeDiff < 0.01) {
        return `${min.toFixed(decimals)}%`;
    }
    return `${min.toFixed(decimals)}% - ${max.toFixed(decimals)}%`;
};

const StatItem: React.FC<{
    label: string;
    value: string;
    subValue?: string;
    valueColor?: string;
    isLast?: boolean;
}> = ({ label, value, subValue, valueColor, isLast }) => (
    <div style={{
        flex: 1,
        padding: '16px 20px',
        borderRight: isLast ? 'none' : '1px solid var(--border-color)'
    }}>
        <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginBottom: '6px', textTransform: 'uppercase' }}>
            {label}
        </div>
        <div style={{ fontSize: '14px', fontWeight: 500, color: valueColor || 'var(--text-primary)', fontFamily: 'var(--font-mono)' }}>
            {value}
        </div>
        {subValue && (
            <div style={{ fontSize: '11px', color: valueColor || 'var(--text-secondary)', marginTop: '2px' }}>
                {subValue}
            </div>
        )}
    </div>
);

export default SummaryCard;

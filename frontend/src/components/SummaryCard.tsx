import React from 'react';
import { useBotStore } from '../context/WebSocketContext';

const SummaryCard: React.FC = () => {
    const { summary, lastTickTime } = useBotStore();

    if (!summary) {
        return (
            <div style={{
                background: 'linear-gradient(135deg, var(--bg-card) 0%, rgba(20, 21, 26, 0.8) 100%)',
                borderRadius: '12px',
                border: '1px solid var(--border-light)',
                padding: '1.5rem',
                minHeight: '200px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--text-muted)'
            }}>
                Waiting for strategy data...
            </div>
        );
    }

    const timeStr = lastTickTime ? new Date(lastTickTime).toLocaleTimeString() : '--:--:--';
    const isPerp = summary.type === 'perp_grid';
    const s = summary.data;

    // Common calculations
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
                              'var(--text-muted)';
        const biasColor = perpData.grid_bias === 'Long' ? 'var(--color-buy)' : 
                          perpData.grid_bias === 'Short' ? 'var(--color-sell)' : 
                          'var(--accent-primary)';

        return (
            <div style={{
                background: 'linear-gradient(135deg, var(--bg-card) 0%, rgba(20, 21, 26, 0.9) 100%)',
                borderRadius: '12px',
                border: '1px solid var(--border-light)',
                overflow: 'hidden'
            }}>
                {/* Header */}
                <div style={{
                    padding: '1rem 1.5rem',
                    background: 'linear-gradient(90deg, rgba(112, 0, 255, 0.15) 0%, transparent 100%)',
                    borderBottom: '1px solid var(--border-light)',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center'
                }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                        <span style={{ 
                            fontSize: '1.25rem', 
                            fontWeight: 700,
                            color: 'var(--text-primary)'
                        }}>
                            {s.symbol}
                        </span>
                        <span style={{
                            background: biasColor,
                            color: '#000',
                            padding: '3px 10px',
                            borderRadius: '4px',
                            fontSize: '0.7rem',
                            fontWeight: 700
                        }}>
                            PERP {perpData.leverage}x {perpData.grid_bias}
                        </span>
                    </div>
                    <div style={{ 
                        fontSize: '0.75rem', 
                        color: 'var(--text-muted)',
                        fontFamily: 'var(--font-mono)'
                    }}>
                        {timeStr}
                    </div>
                </div>

                {/* Price & PnL Row */}
                <div style={{
                    display: 'grid',
                    gridTemplateColumns: '1fr 1fr',
                    borderBottom: '1px solid var(--border-light)'
                }}>
                    <div style={{ padding: '1.25rem 1.5rem', borderRight: '1px solid var(--border-light)' }}>
                        <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', marginBottom: '0.5rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                            Market Price
                        </div>
                        <div style={{ 
                            fontSize: '1.75rem', 
                            fontWeight: 700, 
                            color: 'var(--accent-primary)',
                            fontFamily: 'var(--font-mono)'
                        }}>
                            ${s.price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                        </div>
                    </div>
                    <div style={{ padding: '1.25rem 1.5rem' }}>
                        <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', marginBottom: '0.5rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                            Total PnL
                        </div>
                        <div style={{ 
                            fontSize: '1.75rem', 
                            fontWeight: 700, 
                            color: pnlColor,
                            fontFamily: 'var(--font-mono)'
                        }}>
                            {pnlSign}${totalPnl.toFixed(2)}
                        </div>
                        <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)', marginTop: '0.25rem' }}>
                            Realized: ${s.realized_pnl.toFixed(2)} • Unrealized: ${s.unrealized_pnl.toFixed(2)}
                        </div>
                    </div>
                </div>

                {/* Stats Grid */}
                <div style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(4, 1fr)',
                    gap: '1px',
                    background: 'var(--border-light)'
                }}>
                    <StatCell 
                        label="Position" 
                        value={`${Math.abs(perpData.position_size).toFixed(4)}`}
                        subValue={perpData.position_side}
                        valueColor={positionColor}
                    />
                    <StatCell 
                        label="Avg Entry" 
                        value={`$${perpData.avg_entry_price.toFixed(2)}`}
                    />
                    <StatCell 
                        label="Margin" 
                        value={`$${perpData.margin_balance.toFixed(2)}`}
                    />
                    <StatCell 
                        label="Fees" 
                        value={`-$${s.total_fees.toFixed(2)}`}
                        valueColor="var(--color-sell)"
                    />
                </div>

                {/* Grid Stats Footer */}
                <div style={{
                    padding: '0.75rem 1.5rem',
                    background: 'rgba(0, 0, 0, 0.2)',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    fontSize: '0.8rem'
                }}>
                    <span style={{ color: 'var(--text-muted)' }}>
                        Grid: <span style={{ color: 'var(--text-primary)' }}>{s.grid_count} zones</span>
                        {' • '}
                        <span style={{ color: 'var(--text-primary)' }}>${s.range_low.toLocaleString()} - ${s.range_high.toLocaleString()}</span>
                    </span>
                    <span style={{ 
                        color: 'var(--accent-primary)',
                        fontWeight: 600
                    }}>
                        🔄 {s.roundtrips} roundtrips
                    </span>
                </div>
            </div>
        );
    }

    // Spot Grid Summary
    const spotData = summary.data as typeof summary.data & {
        base_balance: number;
        quote_balance: number;
    };

    return (
        <div style={{
            background: 'linear-gradient(135deg, var(--bg-card) 0%, rgba(20, 21, 26, 0.9) 100%)',
            borderRadius: '12px',
            border: '1px solid var(--border-light)',
            overflow: 'hidden'
        }}>
            {/* Header */}
            <div style={{
                padding: '1rem 1.5rem',
                background: 'linear-gradient(90deg, rgba(0, 240, 255, 0.1) 0%, transparent 100%)',
                borderBottom: '1px solid var(--border-light)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center'
            }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                    <span style={{ 
                        fontSize: '1.25rem', 
                        fontWeight: 700,
                        color: 'var(--text-primary)'
                    }}>
                        {s.symbol}
                    </span>
                    <span style={{
                        background: 'var(--accent-primary)',
                        color: '#000',
                        padding: '3px 10px',
                        borderRadius: '4px',
                        fontSize: '0.7rem',
                        fontWeight: 700
                    }}>
                        SPOT GRID
                    </span>
                </div>
                <div style={{ 
                    fontSize: '0.75rem', 
                    color: 'var(--text-muted)',
                    fontFamily: 'var(--font-mono)'
                }}>
                    {timeStr}
                </div>
            </div>

            {/* Price & PnL Row */}
            <div style={{
                display: 'grid',
                gridTemplateColumns: '1fr 1fr',
                borderBottom: '1px solid var(--border-light)'
            }}>
                <div style={{ padding: '1.25rem 1.5rem', borderRight: '1px solid var(--border-light)' }}>
                    <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', marginBottom: '0.5rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                        Market Price
                    </div>
                    <div style={{ 
                        fontSize: '1.75rem', 
                        fontWeight: 700, 
                        color: 'var(--accent-primary)',
                        fontFamily: 'var(--font-mono)'
                    }}>
                        ${s.price.toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 4 })}
                    </div>
                </div>
                <div style={{ padding: '1.25rem 1.5rem' }}>
                    <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', marginBottom: '0.5rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                        Total PnL
                    </div>
                    <div style={{ 
                        fontSize: '1.75rem', 
                        fontWeight: 700, 
                        color: pnlColor,
                        fontFamily: 'var(--font-mono)'
                    }}>
                        {pnlSign}${totalPnl.toFixed(2)}
                    </div>
                    <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)', marginTop: '0.25rem' }}>
                        Realized: ${s.realized_pnl.toFixed(2)} • Unrealized: ${s.unrealized_pnl.toFixed(2)}
                    </div>
                </div>
            </div>

            {/* Stats Grid */}
            <div style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(4, 1fr)',
                gap: '1px',
                background: 'var(--border-light)'
            }}>
                <StatCell 
                    label="Position" 
                    value={`${spotData.position_size.toFixed(4)}`}
                />
                <StatCell 
                    label="Avg Entry" 
                    value={`$${spotData.avg_entry_price.toFixed(4)}`}
                />
                <StatCell 
                    label="Quote" 
                    value={`$${spotData.quote_balance.toFixed(2)}`}
                />
                <StatCell 
                    label="Fees" 
                    value={`-$${s.total_fees.toFixed(2)}`}
                    valueColor="var(--color-sell)"
                />
            </div>

            {/* Grid Stats Footer */}
            <div style={{
                padding: '0.75rem 1.5rem',
                background: 'rgba(0, 0, 0, 0.2)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                fontSize: '0.8rem'
            }}>
                <span style={{ color: 'var(--text-muted)' }}>
                    Grid: <span style={{ color: 'var(--text-primary)' }}>{s.grid_count} zones</span>
                    {' • '}
                    <span style={{ color: 'var(--text-primary)' }}>${s.range_low.toFixed(2)} - ${s.range_high.toFixed(2)}</span>
                </span>
                <span style={{ 
                    color: 'var(--accent-primary)',
                    fontWeight: 600
                }}>
                    🔄 {s.roundtrips} roundtrips
                </span>
            </div>
        </div>
    );
};

// Reusable stat cell component
const StatCell: React.FC<{ 
    label: string; 
    value: string; 
    subValue?: string;
    valueColor?: string;
}> = ({ label, value, subValue, valueColor }) => (
    <div style={{ 
        padding: '1rem 1.25rem',
        background: 'var(--bg-card)'
    }}>
        <div style={{ 
            fontSize: '0.65rem', 
            color: 'var(--text-muted)', 
            marginBottom: '0.35rem',
            textTransform: 'uppercase',
            letterSpacing: '0.05em'
        }}>
            {label}
        </div>
        <div style={{ 
            fontSize: '1rem', 
            fontWeight: 600, 
            color: valueColor || 'var(--text-primary)',
            fontFamily: 'var(--font-mono)'
        }}>
            {value}
        </div>
        {subValue && (
            <div style={{ 
                fontSize: '0.7rem', 
                color: valueColor || 'var(--text-secondary)',
                marginTop: '0.2rem'
            }}>
                {subValue}
            </div>
        )}
    </div>
);

export default SummaryCard;


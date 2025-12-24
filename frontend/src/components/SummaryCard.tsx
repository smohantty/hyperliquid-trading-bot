import React from 'react';
import { useBotStore } from '../context/WebSocketContext';
import Tooltip from './Tooltip';

const SummaryCard: React.FC = () => {
    const { summary, lastTickTime, connectionStatus } = useBotStore();

    if (!summary) {
        return (
            <div className="card" style={{
                padding: '60px 40px',
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                gap: '16px',
                animationDelay: '0ms'
            }}>
                <div className="skeleton" style={{ width: '200px', height: '40px' }} />
                <div className="skeleton" style={{ width: '140px', height: '20px' }} />
            </div>
        );
    }

    const timeStr = lastTickTime ? new Date(lastTickTime).toLocaleTimeString() : '--:--:--';
    const isPerp = summary.type === 'perp_grid';
    const s = summary.data;

    const totalPnl = s.realized_pnl + s.unrealized_pnl - s.total_fees;
    const pnlColor = totalPnl >= 0 ? 'var(--color-buy)' : 'var(--color-sell)';
    const pnlGlow = totalPnl >= 0 ? 'var(--color-buy-glow)' : 'var(--color-sell-glow)';
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

        const biasClass = perpData.grid_bias === 'Long' ? 'badge-buy' :
            perpData.grid_bias === 'Short' ? 'badge-sell' : 'badge-neutral';

        return (
            <div className="card" style={{
                overflow: 'hidden',
                animationDelay: '0ms'
            }}>
                {/* Header */}
                <div style={{
                    padding: '18px 22px',
                    borderBottom: '1px solid var(--border-color)',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center'
                }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <span style={{
                            fontSize: '22px',
                            fontWeight: 700,
                            letterSpacing: '-0.02em',
                            color: 'var(--text-primary)'
                        }}>
                            {s.symbol}
                        </span>
                        <span className={`badge ${biasClass}`}>
                            {perpData.grid_bias.toUpperCase()}
                        </span>
                        <ConnectionStatus status={connectionStatus} />
                    </div>
                    <span style={{
                        fontSize: '12px',
                        color: 'var(--text-tertiary)',
                        fontFamily: 'var(--font-mono)'
                    }}>
                        {timeStr}
                    </span>
                </div>

                {/* Price & PnL Hero Section - 4 Column Grid */}
                <div style={{
                    display: 'grid',
                    gridTemplateColumns: '1.2fr 1fr 1fr 1.2fr',
                    borderBottom: '1px solid var(--border-color)'
                }}>
                    {/* Market Price */}
                    <div style={{
                        padding: '24px 20px',
                        borderRight: '1px solid var(--border-color)',
                        background: 'linear-gradient(135deg, rgba(0, 240, 192, 0.02) 0%, transparent 100%)'
                    }}>
                        <div style={{
                            fontSize: '10px',
                            color: 'var(--text-tertiary)',
                            marginBottom: '8px',
                            textTransform: 'uppercase',
                            letterSpacing: '0.5px',
                            fontWeight: 500
                        }}>
                            Market Price
                        </div>
                        <div style={{
                            fontSize: '26px',
                            fontWeight: 700,
                            fontFamily: 'var(--font-mono)',
                            color: 'var(--text-primary)',
                            letterSpacing: '-0.02em'
                        }}>
                            ${s.price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                        </div>
                    </div>

                    {/* Realized PnL */}
                    <div style={{
                        padding: '24px 20px',
                        borderRight: '1px solid var(--border-color)'
                    }}>
                        <div style={{
                            fontSize: '10px',
                            color: 'var(--text-tertiary)',
                            marginBottom: '8px',
                            textTransform: 'uppercase',
                            letterSpacing: '0.5px',
                            fontWeight: 500
                        }}>
                            Realized
                        </div>
                        <div style={{
                            fontSize: '18px',
                            fontWeight: 600,
                            fontFamily: 'var(--font-mono)',
                            color: s.realized_pnl >= 0 ? 'var(--color-buy)' : 'var(--color-sell)',
                            letterSpacing: '-0.01em'
                        }}>
                            {s.realized_pnl >= 0 ? '+' : ''}${s.realized_pnl.toFixed(2)}
                        </div>
                    </div>

                    {/* Unrealized PnL */}
                    <div style={{
                        padding: '24px 20px',
                        borderRight: '1px solid var(--border-color)'
                    }}>
                        <div style={{
                            fontSize: '10px',
                            color: 'var(--text-tertiary)',
                            marginBottom: '8px',
                            textTransform: 'uppercase',
                            letterSpacing: '0.5px',
                            fontWeight: 500
                        }}>
                            Unrealized
                        </div>
                        <div style={{
                            fontSize: '18px',
                            fontWeight: 600,
                            fontFamily: 'var(--font-mono)',
                            color: s.unrealized_pnl >= 0 ? 'var(--color-buy)' : 'var(--color-sell)',
                            letterSpacing: '-0.01em'
                        }}>
                            {s.unrealized_pnl >= 0 ? '+' : ''}${s.unrealized_pnl.toFixed(2)}
                        </div>
                    </div>

                    {/* Total PnL - Hero */}
                    <div style={{
                        padding: '24px 20px',
                        background: totalPnl >= 0
                            ? 'linear-gradient(135deg, rgba(0, 230, 118, 0.05) 0%, transparent 100%)'
                            : 'linear-gradient(135deg, rgba(255, 82, 82, 0.05) 0%, transparent 100%)'
                    }}>
                        <div style={{
                            fontSize: '10px',
                            color: 'var(--text-tertiary)',
                            marginBottom: '8px',
                            textTransform: 'uppercase',
                            letterSpacing: '0.5px',
                            fontWeight: 500
                        }}>
                            Total PnL
                        </div>
                        <div style={{
                            fontSize: '26px',
                            fontWeight: 700,
                            color: pnlColor,
                            fontFamily: 'var(--font-mono)',
                            textShadow: `0 0 20px ${pnlGlow}`,
                            letterSpacing: '-0.02em'
                        }}>
                            {pnlSign}${Math.abs(totalPnl).toFixed(2)}
                        </div>
                    </div>
                </div>

                {/* Stats Row */}
                <div style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(5, 1fr)',
                    borderBottom: '1px solid var(--border-color)'
                }}>
                    <StatItem
                        label="Position"
                        value={Math.abs(perpData.position_size).toFixed(4)}
                        subValue={perpData.position_side}
                        valueColor={positionColor}
                    />
                    <StatItem label="Avg Entry" value={`$${perpData.avg_entry_price.toFixed(2)}`} />
                    <StatItem
                        label="Initial Entry"
                        value={s.initial_entry_price ? `$${s.initial_entry_price.toFixed(2)}` : '--'}
                        tooltip="Average acquisition cost when strategy started"
                    />
                    <StatItem label="Margin" value={`$${perpData.margin_balance.toFixed(2)}`} />
                    <StatItem label="Fees" value={`$${s.total_fees.toFixed(2)}`} valueColor="var(--color-sell)" isLast />
                </div>

                {/* Footer - Uptime & Roundtrips */}
                <div style={{
                    padding: '16px 22px',
                    display: 'flex',
                    justifyContent: 'center',
                    alignItems: 'center',
                    gap: '32px',
                    background: 'rgba(0, 0, 0, 0.2)'
                }}>
                    {/* Uptime */}
                    <div style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '10px'
                    }}>
                        <div style={{
                            width: '36px',
                            height: '36px',
                            borderRadius: '50%',
                            background: 'var(--bg-hover)',
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center'
                        }}>
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--text-secondary)" strokeWidth="2">
                                <circle cx="12" cy="12" r="10" />
                                <polyline points="12 6 12 12 16 14" />
                            </svg>
                        </div>
                        <div>
                            <div style={{
                                fontSize: '10px',
                                color: 'var(--text-tertiary)',
                                textTransform: 'uppercase',
                                letterSpacing: '0.5px',
                                marginBottom: '2px'
                            }}>
                                Uptime
                            </div>
                            <div style={{
                                fontSize: '14px',
                                fontWeight: 600,
                                color: 'var(--text-primary)',
                                fontFamily: 'var(--font-mono)'
                            }}>
                                {s.uptime}
                            </div>
                        </div>
                    </div>

                    {/* Divider */}
                    <div style={{
                        width: '1px',
                        height: '32px',
                        background: 'var(--border-color)'
                    }} />

                    {/* Roundtrips */}
                    <div style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '10px'
                    }}>
                        <div style={{
                            width: '36px',
                            height: '36px',
                            borderRadius: '50%',
                            background: 'var(--accent-subtle)',
                            border: '1px solid rgba(0, 240, 192, 0.15)',
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center'
                        }}>
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-primary)" strokeWidth="2">
                                <path d="M17 1l4 4-4 4" />
                                <path d="M3 11V9a4 4 0 0 1 4-4h14" />
                                <path d="M7 23l-4-4 4-4" />
                                <path d="M21 13v2a4 4 0 0 1-4 4H3" />
                            </svg>
                        </div>
                        <div>
                            <div style={{
                                fontSize: '10px',
                                color: 'var(--text-tertiary)',
                                textTransform: 'uppercase',
                                letterSpacing: '0.5px',
                                marginBottom: '2px'
                            }}>
                                Roundtrips
                            </div>
                            <div style={{
                                fontSize: '14px',
                                fontWeight: 600,
                                color: 'var(--accent-primary)',
                                fontFamily: 'var(--font-mono)',
                                textShadow: '0 0 10px var(--accent-glow)'
                            }}>
                                {s.roundtrips}
                            </div>
                        </div>
                    </div>
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
        <div className="card" style={{
            overflow: 'hidden',
            animationDelay: '0ms'
        }}>
            {/* Header */}
            <div style={{
                padding: '18px 22px',
                borderBottom: '1px solid var(--border-color)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center'
            }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <span style={{
                        fontSize: '22px',
                        fontWeight: 700,
                        letterSpacing: '-0.02em'
                    }}>
                        {s.symbol}
                    </span>
                    <span className="badge badge-muted">
                        SPOT
                    </span>
                    <ConnectionStatus status={connectionStatus} />
                </div>
                <span style={{
                    fontSize: '12px',
                    color: 'var(--text-tertiary)',
                    fontFamily: 'var(--font-mono)'
                }}>
                    {timeStr}
                </span>
            </div>

            {/* Price & PnL - 4 Column Grid */}
            <div style={{
                display: 'grid',
                gridTemplateColumns: '1.2fr 1fr 1fr 1.2fr',
                borderBottom: '1px solid var(--border-color)'
            }}>
                {/* Market Price */}
                <div style={{
                    padding: '24px 20px',
                    borderRight: '1px solid var(--border-color)',
                    background: 'linear-gradient(135deg, rgba(0, 240, 192, 0.02) 0%, transparent 100%)'
                }}>
                    <div style={{
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        marginBottom: '8px',
                        textTransform: 'uppercase',
                        letterSpacing: '0.5px',
                        fontWeight: 500
                    }}>
                        Market Price
                    </div>
                    <div style={{
                        fontSize: '26px',
                        fontWeight: 700,
                        fontFamily: 'var(--font-mono)',
                        color: 'var(--text-primary)',
                        letterSpacing: '-0.02em'
                    }}>
                        ${s.price.toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 4 })}
                    </div>
                </div>

                {/* Realized PnL */}
                <div style={{
                    padding: '24px 20px',
                    borderRight: '1px solid var(--border-color)'
                }}>
                    <div style={{
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        marginBottom: '8px',
                        textTransform: 'uppercase',
                        letterSpacing: '0.5px',
                        fontWeight: 500
                    }}>
                        Realized
                    </div>
                    <div style={{
                        fontSize: '18px',
                        fontWeight: 600,
                        fontFamily: 'var(--font-mono)',
                        color: s.realized_pnl >= 0 ? 'var(--color-buy)' : 'var(--color-sell)',
                        letterSpacing: '-0.01em'
                    }}>
                        {s.realized_pnl >= 0 ? '+' : ''}${s.realized_pnl.toFixed(2)}
                    </div>
                </div>

                {/* Unrealized PnL */}
                <div style={{
                    padding: '24px 20px',
                    borderRight: '1px solid var(--border-color)'
                }}>
                    <div style={{
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        marginBottom: '8px',
                        textTransform: 'uppercase',
                        letterSpacing: '0.5px',
                        fontWeight: 500
                    }}>
                        Unrealized
                    </div>
                    <div style={{
                        fontSize: '18px',
                        fontWeight: 600,
                        fontFamily: 'var(--font-mono)',
                        color: s.unrealized_pnl >= 0 ? 'var(--color-buy)' : 'var(--color-sell)',
                        letterSpacing: '-0.01em'
                    }}>
                        {s.unrealized_pnl >= 0 ? '+' : ''}${s.unrealized_pnl.toFixed(2)}
                    </div>
                </div>

                {/* Total PnL - Hero */}
                <div style={{
                    padding: '24px 20px',
                    background: totalPnl >= 0
                        ? 'linear-gradient(135deg, rgba(0, 230, 118, 0.05) 0%, transparent 100%)'
                        : 'linear-gradient(135deg, rgba(255, 82, 82, 0.05) 0%, transparent 100%)'
                }}>
                    <div style={{
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        marginBottom: '8px',
                        textTransform: 'uppercase',
                        letterSpacing: '0.5px',
                        fontWeight: 500
                    }}>
                        Total PnL
                    </div>
                    <div style={{
                        fontSize: '26px',
                        fontWeight: 700,
                        color: pnlColor,
                        fontFamily: 'var(--font-mono)',
                        textShadow: `0 0 20px ${pnlGlow}`,
                        letterSpacing: '-0.02em'
                    }}>
                        {pnlSign}${Math.abs(totalPnl).toFixed(2)}
                    </div>
                </div>
            </div>

            {/* Stats Row */}
            <div style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(5, 1fr)',
                borderBottom: '1px solid var(--border-color)'
            }}>
                <StatItem label="Position" value={spotData.position_size.toFixed(4)} />
                <StatItem label="Avg Entry" value={`$${spotData.avg_entry_price.toFixed(4)}`} />
                <StatItem
                    label="Initial Entry"
                    value={s.initial_entry_price ? `$${s.initial_entry_price.toFixed(4)}` : '--'}
                    tooltip="Average acquisition cost when strategy started"
                />
                <StatItem label="Quote" value={`$${spotData.quote_balance.toFixed(2)}`} />
                <StatItem label="Fees" value={`$${s.total_fees.toFixed(2)}`} valueColor="var(--color-sell)" isLast />
            </div>

            {/* Footer - Uptime & Roundtrips */}
            <div style={{
                padding: '16px 22px',
                display: 'flex',
                justifyContent: 'center',
                alignItems: 'center',
                gap: '32px',
                background: 'rgba(0, 0, 0, 0.2)'
            }}>
                {/* Uptime */}
                <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px'
                }}>
                    <div style={{
                        width: '36px',
                        height: '36px',
                        borderRadius: '50%',
                        background: 'var(--bg-hover)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center'
                    }}>
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--text-secondary)" strokeWidth="2">
                            <circle cx="12" cy="12" r="10" />
                            <polyline points="12 6 12 12 16 14" />
                        </svg>
                    </div>
                    <div>
                        <div style={{
                            fontSize: '10px',
                            color: 'var(--text-tertiary)',
                            textTransform: 'uppercase',
                            letterSpacing: '0.5px',
                            marginBottom: '2px'
                        }}>
                            Uptime
                        </div>
                        <div style={{
                            fontSize: '14px',
                            fontWeight: 600,
                            color: 'var(--text-primary)',
                            fontFamily: 'var(--font-mono)'
                        }}>
                            {s.uptime}
                        </div>
                    </div>
                </div>

                {/* Divider */}
                <div style={{
                    width: '1px',
                    height: '32px',
                    background: 'var(--border-color)'
                }} />

                {/* Roundtrips */}
                <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px'
                }}>
                    <div style={{
                        width: '36px',
                        height: '36px',
                        borderRadius: '50%',
                        background: 'var(--accent-subtle)',
                        border: '1px solid rgba(0, 240, 192, 0.15)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center'
                    }}>
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-primary)" strokeWidth="2">
                            <path d="M17 1l4 4-4 4" />
                            <path d="M3 11V9a4 4 0 0 1 4-4h14" />
                            <path d="M7 23l-4-4 4-4" />
                            <path d="M21 13v2a4 4 0 0 1-4 4H3" />
                        </svg>
                    </div>
                    <div>
                        <div style={{
                            fontSize: '10px',
                            color: 'var(--text-tertiary)',
                            textTransform: 'uppercase',
                            letterSpacing: '0.5px',
                            marginBottom: '2px'
                        }}>
                            Roundtrips
                        </div>
                        <div style={{
                            fontSize: '14px',
                            fontWeight: 600,
                            color: 'var(--accent-primary)',
                            fontFamily: 'var(--font-mono)',
                            textShadow: '0 0 10px var(--accent-glow)'
                        }}>
                            {s.roundtrips}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
};

const StatItem: React.FC<{
    label: string;
    value: string;
    subValue?: string;
    valueColor?: string;
    isLast?: boolean;
    tooltip?: string;
}> = ({ label, value, subValue, valueColor, isLast, tooltip }) => (
    <div style={{
        padding: '18px 20px',
        borderRight: isLast ? 'none' : '1px solid var(--border-color)',
        transition: 'background var(--transition-fast)'
    }}>
        <div style={{
            fontSize: '10px',
            color: 'var(--text-tertiary)',
            marginBottom: '8px',
            textTransform: 'uppercase',
            letterSpacing: '0.5px',
            fontWeight: 500,
            display: 'flex',
            alignItems: 'center',
            gap: '4px'
        }}>
            {tooltip ? (
                <Tooltip content={tooltip}>
                    <span style={{
                        cursor: 'help'
                    }}>
                        {label}
                    </span>
                </Tooltip>
            ) : (
                label
            )}
        </div>
        <div style={{
            fontSize: '15px',
            fontWeight: 600,
            color: valueColor || 'var(--text-primary)',
            fontFamily: 'var(--font-mono)',
            letterSpacing: '-0.01em'
        }}>
            {value}
        </div>
        {subValue && (
            <div style={{
                fontSize: '11px',
                color: valueColor || 'var(--text-secondary)',
                marginTop: '4px',
                fontWeight: 500
            }}>
                {subValue}
            </div>
        )}
    </div>
);

const ConnectionStatus: React.FC<{ status: 'connected' | 'connecting' | 'disconnected' }> = ({ status }) => {
    const statusConfig = {
        connected: {
            label: 'Live',
            color: 'var(--color-buy)',
            bgColor: 'var(--color-buy-bg)',
            dotClass: 'connected'
        },
        connecting: {
            label: 'Connecting',
            color: 'var(--color-warning)',
            bgColor: 'rgba(255, 171, 0, 0.1)',
            dotClass: 'connecting'
        },
        disconnected: {
            label: 'Offline',
            color: 'var(--color-sell)',
            bgColor: 'var(--color-sell-bg)',
            dotClass: 'disconnected'
        }
    };

    const config = statusConfig[status];

    return (
        <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
            padding: '4px 10px',
            borderRadius: 'var(--radius-sm)',
            background: config.bgColor,
            border: `1px solid ${config.color}25`
        }}>
            <div className={`status-dot ${config.dotClass}`} style={{ width: '6px', height: '6px' }} />
            <span style={{
                fontSize: '11px',
                fontWeight: 600,
                color: config.color,
                letterSpacing: '0.3px'
            }}>
                {config.label}
            </span>
        </div>
    );
};

export default SummaryCard;

import React, { useMemo } from 'react';
import { useBotStore } from '../context/WebSocketContext';
import type { ZoneInfo } from '../types/schema';
import Tooltip from './Tooltip';

const OrderBook: React.FC = () => {
    const { gridState, lastPrice, config } = useBotStore();

    const szDecimals = config?.sz_decimals || 4;

    const { asks, bids, maxSize } = useMemo(() => {
        if (!gridState || !lastPrice) return { asks: [], bids: [], maxSize: 0 };

        const sortedZones = [...gridState.zones].sort((a, b) => b.upper_price - a.upper_price);

        const asks: ZoneInfo[] = [];
        const bids: ZoneInfo[] = [];
        let maxSize = 0;

        sortedZones.forEach(zone => {
            if (zone.size > maxSize) maxSize = zone.size;
            if (zone.pending_side === 'Sell') {
                asks.push(zone);
            } else {
                bids.push(zone);
            }
        });

        asks.reverse();
        return { asks, bids, maxSize };
    }, [gridState, lastPrice]);

    if (!gridState || !lastPrice) {
        return (
            <div className="card" style={{
                padding: '60px 40px',
                textAlign: 'center',
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                gap: '16px',
                animationDelay: '200ms'
            }}>
                <div className="skeleton" style={{ width: '100%', height: '200px' }} />
            </div>
        );
    }

    const isPerp = gridState.strategy_type === 'perp_grid';

    return (
        <div className="card" style={{
            overflow: 'hidden',
            animationDelay: '200ms'
        }}>
            {/* Header */}
            <div className="card-header">
                <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ color: 'var(--text-secondary)' }}>
                        <path d="M3 3v18h18"/>
                        <path d="M18 9l-5 5-4-4-3 3"/>
                    </svg>
                    <span className="card-header-title">Order Book</span>
                </div>
                <span className={`badge ${isPerp ? 'badge-neutral' : 'badge-muted'}`}>
                    {isPerp ? `PERP · ${gridState.grid_bias}` : 'SPOT'}
                </span>
            </div>

            {/* CLOB Layout */}
            <div style={{ display: 'flex', minHeight: '320px' }}>
                {/* Asks Side */}
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
                    <div style={{
                        display: 'grid',
                        gridTemplateColumns: '1fr 80px 50px 90px',
                        padding: '10px 16px',
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        textTransform: 'uppercase',
                        letterSpacing: '0.5px',
                        fontWeight: 500,
                        borderBottom: '1px solid var(--border-color)',
                        background: 'rgba(255, 82, 82, 0.03)'
                    }}>
                        <span>Price</span>
                        <span style={{ textAlign: 'center' }}>Size</span>
                        <span style={{ textAlign: 'center' }}>Trades</span>
                        <span style={{ textAlign: 'right' }}>Action</span>
                    </div>
                    <div style={{
                        flex: 1,
                        overflowY: 'auto',
                        maxHeight: '280px'
                    }}>
                        {asks.length === 0 ? (
                            <EmptyState side="asks" />
                        ) : (
                            asks.map(zone => (
                                <ZoneRow
                                    key={zone.index}
                                    zone={zone}
                                    side="ask"
                                    szDecimals={szDecimals}
                                    maxSize={maxSize}
                                />
                            ))
                        )}
                    </div>
                </div>

                {/* Center Price Column */}
                <div style={{
                    width: '160px',
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    justifyContent: 'center',
                    padding: '24px 16px',
                    background: 'linear-gradient(180deg, var(--bg-base) 0%, rgba(5, 7, 10, 0.95) 100%)',
                    borderLeft: '1px solid var(--border-color)',
                    borderRight: '1px solid var(--border-color)',
                    position: 'relative'
                }}>
                    {/* Decorative glow */}
                    <div style={{
                        position: 'absolute',
                        width: '100px',
                        height: '100px',
                        borderRadius: '50%',
                        background: 'var(--accent-glow)',
                        filter: 'blur(40px)',
                        opacity: 0.3
                    }} />

                    <div style={{
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        marginBottom: '12px',
                        textTransform: 'uppercase',
                        letterSpacing: '1px',
                        fontWeight: 500
                    }}>
                        Current
                    </div>
                    <div style={{
                        fontSize: '20px',
                        fontWeight: 700,
                        fontFamily: 'var(--font-mono)',
                        color: 'var(--text-primary)',
                        textShadow: '0 0 20px var(--accent-glow)',
                        position: 'relative',
                        zIndex: 1
                    }}>
                        ${lastPrice.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                    </div>

                    <div style={{
                        marginTop: '24px',
                        display: 'flex',
                        gap: '24px',
                        position: 'relative',
                        zIndex: 1
                    }}>
                        <div style={{ textAlign: 'center' }}>
                            <div style={{
                                color: 'var(--color-sell)',
                                fontWeight: 700,
                                fontSize: '20px',
                                fontFamily: 'var(--font-mono)',
                                textShadow: '0 0 15px var(--color-sell-glow)'
                            }}>
                                {asks.length}
                            </div>
                            <div style={{
                                color: 'var(--text-tertiary)',
                                fontSize: '10px',
                                textTransform: 'uppercase',
                                letterSpacing: '0.5px',
                                marginTop: '4px'
                            }}>
                                Asks
                            </div>
                        </div>
                        <div style={{ textAlign: 'center' }}>
                            <div style={{
                                color: 'var(--color-buy)',
                                fontWeight: 700,
                                fontSize: '20px',
                                fontFamily: 'var(--font-mono)',
                                textShadow: '0 0 15px var(--color-buy-glow)'
                            }}>
                                {bids.length}
                            </div>
                            <div style={{
                                color: 'var(--text-tertiary)',
                                fontSize: '10px',
                                textTransform: 'uppercase',
                                letterSpacing: '0.5px',
                                marginTop: '4px'
                            }}>
                                Bids
                            </div>
                        </div>
                    </div>
                </div>

                {/* Bids Side */}
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
                    <div style={{
                        display: 'grid',
                        gridTemplateColumns: '90px 50px 80px 1fr',
                        padding: '10px 16px',
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        textTransform: 'uppercase',
                        letterSpacing: '0.5px',
                        fontWeight: 500,
                        borderBottom: '1px solid var(--border-color)',
                        background: 'rgba(0, 230, 118, 0.03)'
                    }}>
                        <span>Action</span>
                        <span style={{ textAlign: 'center' }}>Trades</span>
                        <span style={{ textAlign: 'center' }}>Size</span>
                        <span style={{ textAlign: 'right' }}>Price</span>
                    </div>
                    <div style={{
                        flex: 1,
                        overflowY: 'auto',
                        maxHeight: '280px'
                    }}>
                        {bids.length === 0 ? (
                            <EmptyState side="bids" />
                        ) : (
                            bids.map(zone => (
                                <ZoneRow
                                    key={zone.index}
                                    zone={zone}
                                    side="bid"
                                    szDecimals={szDecimals}
                                    maxSize={maxSize}
                                />
                            ))
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
};

const EmptyState: React.FC<{ side: 'asks' | 'bids' }> = ({ side }) => (
    <div style={{
        padding: '40px 20px',
        textAlign: 'center',
        color: 'var(--text-tertiary)',
        fontSize: '12px',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: '8px'
    }}>
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ opacity: 0.5 }}>
            <circle cx="12" cy="12" r="10"/>
            <line x1="15" y1="9" x2="9" y2="15"/>
            <line x1="9" y1="9" x2="15" y2="15"/>
        </svg>
        No {side}
    </div>
);

const ZoneRow: React.FC<{
    zone: ZoneInfo;
    side: 'ask' | 'bid';
    szDecimals: number;
    maxSize: number;
}> = ({ zone, side, szDecimals, maxSize }) => {
    const isAsk = side === 'ask';
    const displayPrice = isAsk ? zone.upper_price : zone.lower_price;
    const nextPrice = isAsk ? zone.lower_price : zone.upper_price;

    const isClose = zone.is_reduce_only;
    const actionColor = isClose ? 'var(--color-warning)' :
        zone.pending_side === 'Buy' ? 'var(--color-buy)' : 'var(--color-sell)';

    const displayLabel = isClose
        ? (zone.pending_side === 'Buy' ? 'Buy (Close)' : 'Sell (Close)')
        : (zone.pending_side === 'Buy' ? 'Buy (Open)' : 'Sell (Open)');

    // Calculate depth bar width
    const depthPercent = maxSize > 0 ? (zone.size / maxSize) * 100 : 0;

    const actionBadge = (
        <span style={{
            background: `${actionColor}15`,
            border: `1px solid ${actionColor}30`,
            color: actionColor,
            padding: '3px 8px',
            borderRadius: 'var(--radius-sm)',
            fontSize: '9px',
            fontWeight: 600,
            letterSpacing: '0.3px',
            whiteSpace: 'nowrap'
        }}>
            {displayLabel}
        </span>
    );

    const priceDisplay = (
        <div style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: isAsk ? 'flex-start' : 'flex-end',
            gap: '2px'
        }}>
            <Tooltip content="Current Active Limit Order">
                <span style={{
                    color: isAsk ? 'var(--color-sell)' : 'var(--color-buy)',
                    fontFamily: 'var(--font-mono)',
                    fontWeight: 600,
                    cursor: 'help',
                    fontSize: '12px'
                }}>
                    {displayPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}
                </span>
            </Tooltip>
            <Tooltip content="Next Order Price (Ping-Pong)">
                <span style={{
                    color: 'var(--text-muted)',
                    fontFamily: 'var(--font-mono)',
                    cursor: 'help',
                    fontSize: '10px'
                }}>
                    {nextPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}
                </span>
            </Tooltip>
        </div>
    );

    return (
        <div style={{
            position: 'relative',
            display: 'grid',
            gridTemplateColumns: isAsk ? '1fr 80px 50px 90px' : '90px 50px 80px 1fr',
            alignItems: 'center',
            padding: '8px 16px',
            fontSize: '12px',
            opacity: zone.has_order ? 1 : 0.35,
            borderBottom: '1px solid var(--border-color)',
            transition: 'background var(--transition-fast), opacity var(--transition-fast)'
        }}
        onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--bg-hover)';
        }}
        onMouseLeave={(e) => {
            e.currentTarget.style.background = 'transparent';
        }}
        >
            {/* Depth Bar */}
            <div
                className={`depth-bar ${side === 'ask' ? 'ask' : 'bid'}`}
                style={{
                    width: `${depthPercent}%`,
                    [side === 'ask' ? 'right' : 'left']: 0
                }}
            />

            {isAsk ? (
                <>
                    <div style={{ position: 'relative', zIndex: 1 }}>{priceDisplay}</div>
                    <span style={{
                        textAlign: 'center',
                        fontFamily: 'var(--font-mono)',
                        color: 'var(--text-secondary)',
                        position: 'relative',
                        zIndex: 1
                    }}>
                        {zone.size.toFixed(szDecimals)}
                    </span>
                    <span style={{
                        textAlign: 'center',
                        fontFamily: 'var(--font-mono)',
                        color: zone.roundtrip_count > 0 ? 'var(--accent-primary)' : 'var(--text-muted)',
                        fontSize: '11px',
                        position: 'relative',
                        zIndex: 1
                    }}>
                        {zone.roundtrip_count}
                    </span>
                    <div style={{ textAlign: 'right', position: 'relative', zIndex: 1 }}>{actionBadge}</div>
                </>
            ) : (
                <>
                    <div style={{ position: 'relative', zIndex: 1 }}>{actionBadge}</div>
                    <span style={{
                        textAlign: 'center',
                        fontFamily: 'var(--font-mono)',
                        color: zone.roundtrip_count > 0 ? 'var(--accent-primary)' : 'var(--text-muted)',
                        fontSize: '11px',
                        position: 'relative',
                        zIndex: 1
                    }}>
                        {zone.roundtrip_count}
                    </span>
                    <span style={{
                        textAlign: 'center',
                        fontFamily: 'var(--font-mono)',
                        color: 'var(--text-secondary)',
                        position: 'relative',
                        zIndex: 1
                    }}>
                        {zone.size.toFixed(szDecimals)}
                    </span>
                    <div style={{ textAlign: 'right', position: 'relative', zIndex: 1 }}>{priceDisplay}</div>
                </>
            )}
        </div>
    );
};

export default OrderBook;

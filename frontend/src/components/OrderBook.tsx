import React, { useMemo } from 'react';
import { useBotStore } from '../context/WebSocketContext';
import type { ZoneInfo } from '../types/schema';
import Tooltip from './Tooltip';

const OrderBook: React.FC = () => {
    const { gridState, lastPrice, config } = useBotStore();

    const szDecimals = config?.sz_decimals || 4;

    const { asks, bids, maxSize } = useMemo(() => {
        if (!gridState || !lastPrice) return { asks: [], bids: [], maxSize: 0 };

        // Sort all zones by price descending (highest first)
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

        // IMPORTANT: Keep both in descending order (highest at top)
        // This makes prices "meet" at the center:
        // - Asks: highest at top, LOWEST (closest to current) at BOTTOM (near center)
        // - Bids: HIGHEST (closest to current) at TOP (near center), lowest at bottom
        // NO reverse needed - both descending creates the visual spread effect

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

    // Best ask (lowest sell) and best bid (highest buy) for spread display
    const bestAsk = asks.length > 0 ? asks[asks.length - 1] : null;
    const bestBid = bids.length > 0 ? bids[0] : null;
    const spread = bestAsk && bestBid
        ? ((bestAsk.upper_price - bestBid.lower_price) / lastPrice * 100).toFixed(3)
        : null;

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

            {/* CLOB Layout - Prices meet in the middle */}
            <div style={{ display: 'flex', minHeight: '320px' }}>
                {/* Asks Side - Price column on RIGHT (adjacent to center) */}
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
                    {/* Header: Action | Trades | Size | Price */}
                    <div style={{
                        display: 'grid',
                        gridTemplateColumns: '85px 50px 70px 1fr',
                        padding: '10px 16px',
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        textTransform: 'uppercase',
                        letterSpacing: '0.5px',
                        fontWeight: 500,
                        borderBottom: '1px solid var(--border-color)',
                        background: 'rgba(255, 82, 82, 0.03)'
                    }}>
                        <span>Action</span>
                        <span style={{ textAlign: 'center' }}>Trades</span>
                        <span style={{ textAlign: 'center' }}>Size</span>
                        <span style={{ textAlign: 'right' }}>Price</span>
                    </div>
                    <div style={{
                        flex: 1,
                        overflowY: 'auto',
                        maxHeight: '280px',
                        display: 'flex',
                        flexDirection: 'column'
                    }}>
                        {asks.length === 0 ? (
                            <EmptyState side="asks" />
                        ) : (
                            asks.map((zone, idx) => (
                                <ZoneRow
                                    key={zone.index}
                                    zone={zone}
                                    side="ask"
                                    szDecimals={szDecimals}
                                    maxSize={maxSize}
                                    isNearSpread={idx === asks.length - 1}
                                />
                            ))
                        )}
                    </div>
                </div>

                {/* Center Price Column */}
                <div style={{
                    width: '150px',
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    justifyContent: 'center',
                    padding: '20px 12px',
                    background: 'linear-gradient(180deg, var(--bg-base) 0%, rgba(5, 7, 10, 0.95) 100%)',
                    borderLeft: '1px solid var(--border-color)',
                    borderRight: '1px solid var(--border-color)',
                    position: 'relative'
                }}>
                    {/* Decorative glow */}
                    <div style={{
                        position: 'absolute',
                        width: '80px',
                        height: '80px',
                        borderRadius: '50%',
                        background: 'var(--accent-glow)',
                        filter: 'blur(35px)',
                        opacity: 0.4
                    }} />

                    <div style={{
                        fontSize: '9px',
                        color: 'var(--text-tertiary)',
                        marginBottom: '8px',
                        textTransform: 'uppercase',
                        letterSpacing: '1px',
                        fontWeight: 500
                    }}>
                        Current
                    </div>
                    <div style={{
                        fontSize: '18px',
                        fontWeight: 700,
                        fontFamily: 'var(--font-mono)',
                        color: 'var(--text-primary)',
                        textShadow: '0 0 20px var(--accent-glow)',
                        position: 'relative',
                        zIndex: 1
                    }}>
                        ${lastPrice.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                    </div>

                    {/* Spread indicator */}
                    {spread && (
                        <div style={{
                            marginTop: '12px',
                            padding: '4px 10px',
                            background: 'var(--bg-hover)',
                            borderRadius: 'var(--radius-sm)',
                            fontSize: '10px',
                            color: 'var(--text-secondary)',
                            fontFamily: 'var(--font-mono)',
                            position: 'relative',
                            zIndex: 1
                        }}>
                            <span style={{ color: 'var(--text-tertiary)' }}>Spread:</span> {spread}%
                        </div>
                    )}

                    <div style={{
                        marginTop: '16px',
                        display: 'flex',
                        gap: '20px',
                        position: 'relative',
                        zIndex: 1
                    }}>
                        <div style={{ textAlign: 'center' }}>
                            <div style={{
                                color: 'var(--color-sell)',
                                fontWeight: 700,
                                fontSize: '18px',
                                fontFamily: 'var(--font-mono)',
                                textShadow: '0 0 12px var(--color-sell-glow)'
                            }}>
                                {asks.length}
                            </div>
                            <div style={{
                                color: 'var(--text-tertiary)',
                                fontSize: '9px',
                                textTransform: 'uppercase',
                                letterSpacing: '0.5px',
                                marginTop: '2px'
                            }}>
                                Asks
                            </div>
                        </div>
                        <div style={{ textAlign: 'center' }}>
                            <div style={{
                                color: 'var(--color-buy)',
                                fontWeight: 700,
                                fontSize: '18px',
                                fontFamily: 'var(--font-mono)',
                                textShadow: '0 0 12px var(--color-buy-glow)'
                            }}>
                                {bids.length}
                            </div>
                            <div style={{
                                color: 'var(--text-tertiary)',
                                fontSize: '9px',
                                textTransform: 'uppercase',
                                letterSpacing: '0.5px',
                                marginTop: '2px'
                            }}>
                                Bids
                            </div>
                        </div>
                    </div>
                </div>

                {/* Bids Side - Price column on LEFT (adjacent to center) */}
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
                    {/* Header: Price | Size | Trades | Action */}
                    <div style={{
                        display: 'grid',
                        gridTemplateColumns: '1fr 70px 50px 85px',
                        padding: '10px 16px',
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        textTransform: 'uppercase',
                        letterSpacing: '0.5px',
                        fontWeight: 500,
                        borderBottom: '1px solid var(--border-color)',
                        background: 'rgba(0, 230, 118, 0.03)'
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
                        {bids.length === 0 ? (
                            <EmptyState side="bids" />
                        ) : (
                            bids.map((zone, idx) => (
                                <ZoneRow
                                    key={zone.index}
                                    zone={zone}
                                    side="bid"
                                    szDecimals={szDecimals}
                                    maxSize={maxSize}
                                    isNearSpread={idx === 0}
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
        justifyContent: 'center',
        flex: 1,
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
    isNearSpread?: boolean;
}> = ({ zone, side, szDecimals, maxSize, isNearSpread }) => {
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
            alignItems: isAsk ? 'flex-end' : 'flex-start',
            gap: '2px'
        }}>
            <Tooltip content="Current Active Limit Order">
                <span style={{
                    color: isAsk ? 'var(--color-sell)' : 'var(--color-buy)',
                    fontFamily: 'var(--font-mono)',
                    fontWeight: 600,
                    cursor: 'help',
                    fontSize: '12px',
                    textShadow: isNearSpread ? `0 0 8px ${isAsk ? 'var(--color-sell-glow)' : 'var(--color-buy-glow)'}` : 'none'
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

    // Column order:
    // Asks: Action | Trades | Size | Price (price on right, near center)
    // Bids: Price | Size | Trades | Action (price on left, near center)

    return (
        <div style={{
            position: 'relative',
            display: 'grid',
            gridTemplateColumns: isAsk ? '85px 50px 70px 1fr' : '1fr 70px 50px 85px',
            alignItems: 'center',
            padding: '8px 16px',
            fontSize: '12px',
            opacity: zone.has_order ? 1 : 0.35,
            borderBottom: '1px solid var(--border-color)',
            background: isNearSpread ? (isAsk ? 'rgba(255, 82, 82, 0.03)' : 'rgba(0, 230, 118, 0.03)') : 'transparent',
            transition: 'background var(--transition-fast), opacity var(--transition-fast)'
        }}
        onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--bg-hover)';
        }}
        onMouseLeave={(e) => {
            e.currentTarget.style.background = isNearSpread
                ? (isAsk ? 'rgba(255, 82, 82, 0.03)' : 'rgba(0, 230, 118, 0.03)')
                : 'transparent';
        }}
        >
            {/* Depth Bar - grows from center outward */}
            <div
                className={`depth-bar ${side === 'ask' ? 'ask' : 'bid'}`}
                style={{
                    width: `${depthPercent}%`,
                    // Asks: bar grows from right (center) to left
                    // Bids: bar grows from left (center) to right
                    [isAsk ? 'right' : 'left']: 0
                }}
            />

            {isAsk ? (
                // Asks: Action | Trades | Size | Price
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
            ) : (
                // Bids: Price | Size | Trades | Action
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
            )}
        </div>
    );
};

export default OrderBook;

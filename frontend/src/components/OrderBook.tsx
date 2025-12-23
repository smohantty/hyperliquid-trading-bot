import React, { useMemo } from 'react';
import { useBotStore } from '../context/WebSocketContext';
import type { ZoneInfo } from '../types/schema';

const OrderBook: React.FC = () => {
    const { gridState, lastPrice, config } = useBotStore();

    const szDecimals = config?.sz_decimals || 4;

    const { asks, bids } = useMemo(() => {
        if (!gridState || !lastPrice) return { asks: [], bids: [] };

        const sortedZones = [...gridState.zones].sort((a, b) => b.upper_price - a.upper_price);

        const asks: ZoneInfo[] = [];
        const bids: ZoneInfo[] = [];

        sortedZones.forEach(zone => {
            if (zone.pending_side === 'Sell') {
                asks.push(zone);
            } else {
                bids.push(zone);
            }
        });

        asks.reverse();
        return { asks, bids };
    }, [gridState, lastPrice]);

    if (!gridState || !lastPrice) {
        return (
            <div style={{
                background: 'var(--bg-secondary)',
                borderRadius: '8px',
                border: '1px solid var(--border-color)',
                padding: '40px',
                textAlign: 'center',
                color: 'var(--text-tertiary)'
            }}>
                Loading Grid State...
            </div>
        );
    }

    const isPerp = gridState.strategy_type === 'perp_grid';

    return (
        <div style={{
            background: 'var(--bg-secondary)',
            borderRadius: '8px',
            border: '1px solid var(--border-color)',
            overflow: 'hidden'
        }}>
            {/* Header */}
            <div style={{
                padding: '12px 16px',
                borderBottom: '1px solid var(--border-color)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center'
            }}>
                <span style={{ fontSize: '12px', fontWeight: 500, color: 'var(--text-secondary)' }}>
                    Order Book
                </span>
                <span style={{
                    fontSize: '10px',
                    color: 'var(--text-tertiary)',
                    background: 'var(--bg-tertiary)',
                    padding: '2px 6px',
                    borderRadius: '3px'
                }}>
                    {isPerp ? `PERP · ${gridState.grid_bias}` : 'SPOT'}
                </span>
            </div>

            {/* CLOB Layout */}
            <div style={{ display: 'flex' }}>
                {/* Asks */}
                <div style={{ flex: 1, borderRight: '1px solid var(--border-color)' }}>
                    <div style={{
                        display: 'flex',
                        padding: '8px 12px',
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        textTransform: 'uppercase',
                        borderBottom: '1px solid var(--border-color)',
                        background: 'rgba(246, 70, 93, 0.03)'
                    }}>
                        <span style={{ flex: 1 }}>Price</span>
                        <span style={{ flex: 1, textAlign: 'center' }}>Size</span>
                        <span style={{ flex: 1, textAlign: 'right' }}>Action</span>
                    </div>
                    <div style={{ maxHeight: '240px', overflowY: 'auto' }}>
                        {asks.length === 0 ? (
                            <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-tertiary)', fontSize: '11px' }}>
                                No asks
                            </div>
                        ) : (
                            asks.map(zone => <ZoneRow key={zone.index} zone={zone} side="ask" szDecimals={szDecimals} />)
                        )}
                    </div>
                </div>

                {/* Center Price */}
                <div style={{
                    width: '140px',
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    justifyContent: 'center',
                    padding: '20px 16px',
                    background: 'var(--bg-primary)'
                }}>
                    <div style={{ fontSize: '10px', color: 'var(--text-tertiary)', marginBottom: '8px', textTransform: 'uppercase' }}>
                        Current
                    </div>
                    <div style={{
                        fontSize: '16px',
                        fontWeight: 600,
                        fontFamily: 'var(--font-mono)',
                        color: 'var(--text-primary)'
                    }}>
                        ${lastPrice.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                    </div>
                    <div style={{
                        marginTop: '16px',
                        display: 'flex',
                        gap: '20px',
                        fontSize: '11px'
                    }}>
                        <div style={{ textAlign: 'center' }}>
                            <div style={{ color: 'var(--color-sell)', fontWeight: 600 }}>{asks.length}</div>
                            <div style={{ color: 'var(--text-tertiary)', fontSize: '9px' }}>Asks</div>
                        </div>
                        <div style={{ textAlign: 'center' }}>
                            <div style={{ color: 'var(--color-buy)', fontWeight: 600 }}>{bids.length}</div>
                            <div style={{ color: 'var(--text-tertiary)', fontSize: '9px' }}>Bids</div>
                        </div>
                    </div>
                </div>

                {/* Bids */}
                <div style={{ flex: 1, borderLeft: '1px solid var(--border-color)' }}>
                    <div style={{
                        display: 'flex',
                        padding: '8px 12px',
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        textTransform: 'uppercase',
                        borderBottom: '1px solid var(--border-color)',
                        background: 'rgba(14, 203, 129, 0.03)'
                    }}>
                        <span style={{ flex: 1 }}>Action</span>
                        <span style={{ flex: 1, textAlign: 'center' }}>Size</span>
                        <span style={{ flex: 1, textAlign: 'right' }}>Price</span>
                    </div>
                    <div style={{ maxHeight: '240px', overflowY: 'auto' }}>
                        {bids.length === 0 ? (
                            <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-tertiary)', fontSize: '11px' }}>
                                No bids
                            </div>
                        ) : (
                            bids.map(zone => <ZoneRow key={zone.index} zone={zone} side="bid" szDecimals={szDecimals} />)
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
};

const ZoneRow: React.FC<{ zone: ZoneInfo; side: 'ask' | 'bid'; szDecimals: number }> = ({ zone, side, szDecimals }) => {
    const isAsk = side === 'ask';
    const displayPrice = isAsk ? zone.upper_price : zone.lower_price;

    const isClose = zone.is_reduce_only;
    // Derive action label/type if needed, or simply use pending side color
    const actionColor = isClose ? 'var(--accent-yellow)' :
        zone.pending_side === 'Buy' ? 'var(--color-buy)' : 'var(--color-sell)';

    let displayLabel: string = zone.pending_side;
    if (isClose) {
        // If reducing, maybe we want special label?
        // Backend used to send "Close Long", "Close Short" etc.
        // We can infer context if we had strategy type passed down.
        // But simplistic view:
        displayLabel = zone.pending_side === 'Buy' ? 'Buy (Close)' : 'Sell (Close)';
    } else {
        displayLabel = zone.pending_side === 'Buy' ? 'Buy (Open)' : 'Sell (Open)';
    }

    // Better logic:
    // If we want exact parity with backend logic:
    // We need to know if it's Long/Short/Neutral mode (which is in gridState, but not passed to ZoneRow deeply... wait gridState is in context)

    // For now, let's keep it simple or use what we have.
    // The previous code usedt's improve ZoneRow to take gridBias/strategyType if we want specific "Open Long" text.
    // But user said "frontend can easily figure it out".

    // Let's use logic similar to backend.
    // We don't have ZoneMode in frontend explicitly attached to ZoneInfo easily without passing parent state.
    // But we know:
    // Perp + Long Bias + Sell = Close Long
    // etc.

    // However, ZoneRow doesn't receive strategy info currently.
    // I should update ZoneRow signature to receive context info if needed?
    // Or just simplify.

    // Let's assume passed prop or update signature.
    // Looking at OrderBook.tsx, it maps zones.


    const actionBadge = (
        <span style={{
            background: `${actionColor}20`,
            color: actionColor,
            padding: '2px 6px',
            borderRadius: '3px',
            fontSize: '9px',
            fontWeight: 600
        }}>
            {displayLabel}
        </span>
    );

    return (
        <div style={{
            display: 'flex',
            alignItems: 'center',
            padding: '6px 12px',
            fontSize: '12px',
            opacity: zone.has_order ? 1 : 0.4,
            borderBottom: '1px solid var(--border-color)'
        }}>
            {isAsk ? (
                <>
                    <span style={{ flex: 1, color: 'var(--color-sell)', fontFamily: 'var(--font-mono)' }}>
                        {displayPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}
                    </span>
                    <span style={{ flex: 1, textAlign: 'center', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }}>
                        {zone.size.toFixed(szDecimals)}
                    </span>
                    <span style={{ flex: 1, textAlign: 'right' }}>{actionBadge}</span>
                </>
            ) : (
                <>
                    <span style={{ flex: 1 }}>{actionBadge}</span>
                    <span style={{ flex: 1, textAlign: 'center', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }}>
                        {zone.size.toFixed(szDecimals)}
                    </span>
                    <span style={{ flex: 1, textAlign: 'right', color: 'var(--color-buy)', fontFamily: 'var(--font-mono)' }}>
                        {displayPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}
                    </span>
                </>
            )}
        </div>
    );
};

export default OrderBook;

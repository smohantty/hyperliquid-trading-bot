import React, { useMemo } from 'react';
import { useBotStore } from '../context/WebSocketContext';
import type { ZoneInfo } from '../types/schema';

const OrderBook: React.FC = () => {
    const { gridState, lastPrice } = useBotStore();

    // Split zones into asks (above price) and bids (below price)
    const { asks, bids } = useMemo(() => {
        if (!gridState || !lastPrice) return { asks: [], bids: [] };
        
        const sortedZones = [...gridState.zones].sort((a, b) => b.upper_price - a.upper_price);
        
        const asks: ZoneInfo[] = [];
        const bids: ZoneInfo[] = [];
        
        sortedZones.forEach(zone => {
            // Use midpoint of zone for classification
            const midPrice = (zone.lower_price + zone.upper_price) / 2;
            if (midPrice > lastPrice) {
                asks.push(zone);
            } else {
                bids.push(zone);
            }
        });
        
        // Asks: lowest first (closest to current price at bottom)
        asks.reverse();
        
        return { asks, bids };
    }, [gridState, lastPrice]);

    if (!gridState || !lastPrice) {
        return (
            <div style={{
                background: 'var(--bg-card)',
                borderRadius: '12px',
                border: '1px solid var(--border-light)',
                padding: '3rem',
                textAlign: 'center',
                color: 'var(--text-muted)'
            }}>
                Loading Grid State...
            </div>
        );
    }

    const isPerp = gridState.strategy_type === 'perp_grid';
    const biasLabel = gridState.grid_bias ? ` (${gridState.grid_bias})` : '';

    return (
        <div style={{
            background: 'var(--bg-card)',
            borderRadius: '12px',
            border: '1px solid var(--border-light)',
            overflow: 'hidden'
        }}>
            {/* Header */}
            <div style={{ 
                padding: '0.75rem 1.25rem', 
                borderBottom: '1px solid var(--border-light)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                background: 'rgba(255, 255, 255, 0.02)'
            }}>
                <span style={{ 
                    fontSize: '0.75rem', 
                    fontWeight: 600,
                    color: 'var(--text-secondary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em'
                }}>
                    📊 Live Grid Structure
                </span>
                <span style={{ 
                    fontSize: '0.65rem', 
                    color: isPerp ? 'var(--accent-secondary)' : 'var(--accent-primary)',
                    background: 'rgba(255,255,255,0.05)',
                    padding: '2px 8px',
                    borderRadius: '4px'
                }}>
                    {isPerp ? `PERP${biasLabel}` : 'SPOT'}
                </span>
            </div>

            {/* CLOB Style Layout */}
            <div style={{
                display: 'grid',
                gridTemplateColumns: '1fr auto 1fr',
                minHeight: '300px'
            }}>
                {/* Asks (Sell side) - Left */}
                <div style={{ 
                    borderRight: '1px solid var(--border-light)',
                    display: 'flex',
                    flexDirection: 'column'
                }}>
                    <div style={{
                        padding: '0.5rem 1rem',
                        background: 'rgba(255, 0, 85, 0.05)',
                        borderBottom: '1px solid var(--border-light)',
                        fontSize: '0.65rem',
                        color: 'var(--color-sell)',
                        fontWeight: 600,
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em',
                        display: 'flex',
                        justifyContent: 'space-between'
                    }}>
                        <span>Price</span>
                        <span>Size</span>
                        <span>Action</span>
                    </div>
                    <div style={{ flex: 1, overflowY: 'auto', maxHeight: '280px' }}>
                        {asks.map(zone => (
                            <ZoneRow key={zone.index} zone={zone} side="ask" />
                        ))}
                        {asks.length === 0 && (
                            <div style={{ padding: '1rem', color: 'var(--text-muted)', textAlign: 'center', fontSize: '0.8rem' }}>
                                No asks
                            </div>
                        )}
                    </div>
                </div>

                {/* Current Price - Center */}
                <div style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    justifyContent: 'center',
                    padding: '1rem 2rem',
                    background: 'linear-gradient(180deg, rgba(0, 240, 255, 0.05) 0%, transparent 50%, rgba(0, 255, 157, 0.05) 100%)',
                    minWidth: '180px'
                }}>
                    <div style={{ 
                        fontSize: '0.65rem', 
                        color: 'var(--text-muted)', 
                        marginBottom: '0.5rem',
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em'
                    }}>
                        Current Price
                    </div>
                    <div style={{
                        fontSize: '1.5rem',
                        fontWeight: 700,
                        color: 'var(--accent-primary)',
                        fontFamily: 'var(--font-mono)'
                    }}>
                        ${lastPrice.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                    </div>
                    <div style={{
                        marginTop: '1rem',
                        display: 'flex',
                        gap: '1.5rem',
                        fontSize: '0.75rem'
                    }}>
                        <div style={{ textAlign: 'center' }}>
                            <div style={{ color: 'var(--color-sell)', fontWeight: 600 }}>{asks.length}</div>
                            <div style={{ color: 'var(--text-muted)', fontSize: '0.6rem' }}>ASKS</div>
                        </div>
                        <div style={{ textAlign: 'center' }}>
                            <div style={{ color: 'var(--color-buy)', fontWeight: 600 }}>{bids.length}</div>
                            <div style={{ color: 'var(--text-muted)', fontSize: '0.6rem' }}>BIDS</div>
                        </div>
                    </div>
                </div>

                {/* Bids (Buy side) - Right */}
                <div style={{ 
                    borderLeft: '1px solid var(--border-light)',
                    display: 'flex',
                    flexDirection: 'column'
                }}>
                    <div style={{
                        padding: '0.5rem 1rem',
                        background: 'rgba(0, 255, 157, 0.05)',
                        borderBottom: '1px solid var(--border-light)',
                        fontSize: '0.65rem',
                        color: 'var(--color-buy)',
                        fontWeight: 600,
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em',
                        display: 'flex',
                        justifyContent: 'space-between'
                    }}>
                        <span>Action</span>
                        <span>Size</span>
                        <span>Price</span>
                    </div>
                    <div style={{ flex: 1, overflowY: 'auto', maxHeight: '280px' }}>
                        {bids.map(zone => (
                            <ZoneRow key={zone.index} zone={zone} side="bid" />
                        ))}
                        {bids.length === 0 && (
                            <div style={{ padding: '1rem', color: 'var(--text-muted)', textAlign: 'center', fontSize: '0.8rem' }}>
                                No bids
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
};

// Zone row component
const ZoneRow: React.FC<{ zone: ZoneInfo; side: 'ask' | 'bid' }> = ({ zone, side }) => {
    const isAsk = side === 'ask';
    const displayPrice = isAsk ? zone.upper_price : zone.lower_price;
    
    // Color based on action type
    let actionColor: string;
    if (zone.is_reduce_only || zone.action_type === 'close') {
        actionColor = '#ffaa00'; // Yellow for closing
    } else if (zone.pending_side === 'Buy') {
        actionColor = 'var(--color-buy)';
    } else {
        actionColor = 'var(--color-sell)';
    }

    const content = isAsk ? (
        <>
            <span style={{ 
                flex: 1, 
                color: 'var(--color-sell)',
                fontFamily: 'var(--font-mono)'
            }}>
                {displayPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}
            </span>
            <span style={{ 
                flex: 1, 
                textAlign: 'center',
                fontFamily: 'var(--font-mono)'
            }}>
                {zone.size.toFixed(4)}
            </span>
            <span style={{ flex: 1, textAlign: 'right' }}>
                <span style={{
                    background: actionColor,
                    color: '#000',
                    padding: '2px 6px',
                    borderRadius: '3px',
                    fontSize: '0.6rem',
                    fontWeight: 700
                }}>
                    {zone.action_label}
                </span>
            </span>
        </>
    ) : (
        <>
            <span style={{ flex: 1 }}>
                <span style={{
                    background: actionColor,
                    color: '#000',
                    padding: '2px 6px',
                    borderRadius: '3px',
                    fontSize: '0.6rem',
                    fontWeight: 700
                }}>
                    {zone.action_label}
                </span>
            </span>
            <span style={{ 
                flex: 1, 
                textAlign: 'center',
                fontFamily: 'var(--font-mono)'
            }}>
                {zone.size.toFixed(4)}
            </span>
            <span style={{ 
                flex: 1, 
                textAlign: 'right',
                color: 'var(--color-buy)',
                fontFamily: 'var(--font-mono)'
            }}>
                {displayPrice.toLocaleString(undefined, { minimumFractionDigits: 2 })}
            </span>
        </>
    );

    return (
        <div style={{
            display: 'flex',
            alignItems: 'center',
            padding: '0.4rem 1rem',
            fontSize: '0.75rem',
            opacity: zone.has_order ? 1 : 0.4,
            background: zone.has_order 
                ? (isAsk ? 'rgba(255, 0, 85, 0.03)' : 'rgba(0, 255, 157, 0.03)')
                : 'transparent',
            borderBottom: '1px solid rgba(255,255,255,0.03)'
        }}>
            {content}
        </div>
    );
};

export default OrderBook;

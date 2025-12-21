import React, { useMemo, useRef, useEffect } from 'react';
import { useBotStore } from '../context/WebSocketContext';
import type { ZoneInfo } from '../types/schema';

const OrderBook: React.FC = () => {
    const { gridState, lastPrice } = useBotStore();
    const scrollContainerRef = useRef<HTMLDivElement>(null);

    // Sort zones by price (highest first for CLOB view)
    const sortedZones = useMemo(() => {
        if (!gridState) return [];
        return [...gridState.zones].sort((a, b) => b.upper_price - a.upper_price);
    }, [gridState]);

    // Auto-scroll to center the active price when zones change
    useEffect(() => {
        if (scrollContainerRef.current) {
            const indicator = scrollContainerRef.current.querySelector('#active-price-indicator');
            if (indicator) {
                indicator.scrollIntoView({ block: 'center', behavior: 'smooth' });
            }
        }
    }, [gridState?.zones]);

    if (!gridState || !lastPrice) {
        return (
            <div style={{
                background: 'var(--bg-card)',
                borderRadius: '8px',
                border: '1px solid var(--border-light)',
                padding: '2rem',
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
            borderRadius: '8px',
            border: '1px solid var(--border-light)',
            height: '600px',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden'
        }}>
            {/* Header */}
            <div style={{ 
                padding: '1rem', 
                borderBottom: '1px solid var(--border-light)', 
                fontWeight: 600,
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center'
            }}>
                <span>LIVE GRID STRUCTURE</span>
                <span style={{ 
                    fontSize: '0.75rem', 
                    color: isPerp ? 'var(--accent-secondary)' : 'var(--accent-primary)',
                    background: 'rgba(255,255,255,0.05)',
                    padding: '4px 8px',
                    borderRadius: '4px'
                }}>
                    {isPerp ? `PERP${biasLabel}` : 'SPOT'}
                </span>
            </div>

            {/* Column Headers */}
            <div style={{
                display: 'flex',
                padding: '0.5rem 1rem',
                fontSize: '0.75rem',
                color: 'var(--text-muted)',
                borderBottom: '1px solid var(--border-light)',
                background: 'rgba(255,255,255,0.02)'
            }}>
                <div style={{ width: '25%' }}>PRICE</div>
                <div style={{ width: '20%', textAlign: 'right' }}>SIZE</div>
                <div style={{ width: '30%', textAlign: 'center' }}>ACTION</div>
                <div style={{ width: '25%', textAlign: 'right' }}>STATUS</div>
            </div>

            {/* Zone List */}
            <div
                ref={scrollContainerRef}
                style={{
                    flex: 1,
                    overflowY: 'auto',
                    position: 'relative',
                    display: 'flex',
                    flexDirection: 'column'
                }}
            >
                <div style={{ margin: 'auto 0', width: '100%' }}>
                    {sortedZones.map((zone, i) => {
                        // Determine the display price (use lower for buys, upper for sells)
                        const displayPrice = zone.pending_side === 'Buy' ? zone.lower_price : zone.upper_price;
                        
                        // Check if we should render the "Current Price" indicator
                        const nextZone = sortedZones[i + 1];
                        const showPriceLine = nextZone && lastPrice <= zone.upper_price && lastPrice > nextZone.upper_price;
                        const isTopPrice = i === 0 && lastPrice > zone.upper_price;
                        const isBottomPrice = i === sortedZones.length - 1 && lastPrice < zone.lower_price;

                        return (
                            <React.Fragment key={zone.index}>
                                {isTopPrice && <PriceIndicator price={lastPrice} id="active-price-indicator" />}
                                
                                <ZoneRow zone={zone} displayPrice={displayPrice} isPerp={isPerp} />
                                
                                {showPriceLine && <PriceIndicator price={lastPrice} id="active-price-indicator" />}
                                {isBottomPrice && <PriceIndicator price={lastPrice} id="active-price-indicator" />}
                            </React.Fragment>
                        );
                    })}
                </div>
            </div>
        </div>
    );
};

// Individual zone row component
const ZoneRow: React.FC<{ zone: ZoneInfo; displayPrice: number; isPerp: boolean }> = ({ zone, displayPrice }) => {
    // Determine colors based on action type
    const isBuy = zone.pending_side === 'Buy';
    
    // Color logic:
    // - Open orders: green for buy, red for sell
    // - Close orders (reduce_only): yellow/orange
    let rowColor: string;
    let bgColor: string;
    
    if (zone.is_reduce_only || zone.action_type === 'close') {
        rowColor = '#ffaa00'; // Yellow for closing orders
        bgColor = 'rgba(255, 170, 0, 0.05)';
    } else if (isBuy) {
        rowColor = 'var(--color-buy)';
        bgColor = 'rgba(0, 255, 157, 0.05)';
    } else {
        rowColor = 'var(--color-sell)';
        bgColor = 'rgba(255, 0, 85, 0.05)';
    }

    return (
        <div style={{
            display: 'flex',
            padding: '6px 1rem',
            fontSize: '0.9rem',
            backgroundColor: bgColor,
            borderLeft: `3px solid ${rowColor}`,
            marginBottom: '1px',
            opacity: zone.has_order ? 1 : 0.4,
            transition: 'opacity 0.2s ease'
        }}>
            {/* Price */}
            <div style={{ width: '25%', color: rowColor, fontFamily: 'var(--font-mono)' }}>
                {displayPrice.toFixed(4)}
            </div>
            
            {/* Size */}
            <div style={{ width: '20%', textAlign: 'right', fontFamily: 'var(--font-mono)' }}>
                {zone.size.toFixed(4)}
            </div>
            
            {/* Action Label */}
            <div style={{ 
                width: '30%', 
                textAlign: 'center',
                fontSize: '0.75rem',
                fontWeight: 600
            }}>
                <span style={{
                    background: rowColor,
                    color: '#000',
                    padding: '2px 8px',
                    borderRadius: '4px',
                    display: 'inline-block'
                }}>
                    {zone.action_label}
                </span>
            </div>
            
            {/* Status */}
            <div style={{ 
                width: '25%', 
                textAlign: 'right', 
                fontSize: '0.75rem', 
                color: 'var(--text-muted)',
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'flex-end',
                gap: '2px'
            }}>
                <span style={{ color: zone.has_order ? 'var(--color-active)' : 'var(--color-idle)' }}>
                    {zone.has_order ? '● OPEN' : '○ IDLE'}
                </span>
                {zone.roundtrip_count > 0 && (
                    <span style={{ fontSize: '0.65rem' }}>
                        🔄 {zone.roundtrip_count}
                    </span>
                )}
            </div>
        </div>
    );
};

// Current price indicator
const PriceIndicator: React.FC<{ price: number; id?: string }> = ({ price, id }) => (
    <div id={id} style={{
        padding: '10px 1rem',
        background: 'linear-gradient(90deg, rgba(0, 240, 255, 0.1) 0%, rgba(0, 240, 255, 0.02) 100%)',
        borderTop: '2px solid var(--accent-primary)',
        borderBottom: '2px solid var(--accent-primary)',
        color: 'var(--accent-primary)',
        fontWeight: 'bold',
        textAlign: 'center',
        margin: '4px 0',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        gap: '0.75rem',
        fontFamily: 'var(--font-mono)',
        fontSize: '1rem'
    }}>
        <span style={{ fontSize: '0.8rem' }}>▶</span>
        <span>CURRENT: ${price.toFixed(4)}</span>
        <span style={{ fontSize: '0.8rem' }}>◀</span>
    </div>
);

export default OrderBook;

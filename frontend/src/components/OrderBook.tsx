import React, { useMemo, useRef, useEffect } from 'react';
import { useBotStore } from '../context/WebSocketContext';

const OrderBook: React.FC = () => {
    const { summary, lastPrice } = useBotStore();
    const scrollContainerRef = useRef<HTMLDivElement>(null);

    const zones = useMemo(() => {
        if (!summary) return [];
        // Sort descending by price (High price at top)
        return [...summary.zones].sort((a, b) => b.price - a.price);
    }, [summary]);

    // split zones into Asks (Sell) and Bids (Buy)
    // Since we sorted descending:
    // Asks are typically higher prices (Top of list)
    // Bids are typically lower prices (Bottom of list)

    // However, in a Grid bot, a zone might be "WaitingBuy" or "WaitingSell".
    // WaitingSell -> We hold the asset, we want to sell high. (Ask)
    // WaitingBuy -> We hold quote, we want to buy low. (Bid)

    // We render the rows. We also want to inject a "Current Price" row in the correct spot.

    // Auto-scroll to center the active price when zones change
    useEffect(() => {
        if (scrollContainerRef.current) {
            const indicator = scrollContainerRef.current.querySelector('#active-price-indicator');
            if (indicator) {
                indicator.scrollIntoView({ block: 'center', behavior: 'smooth' });
            }
        }
    }, [summary?.zones]); // Re-center when the grid structure updates

    if (!summary || !lastPrice) return <div className="p-4 text-muted">Loading Order Book...</div>;

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
            <div style={{ padding: '1rem', borderBottom: '1px solid var(--border-light)', fontWeight: 600 }}>
                LIVE GRID STRUCTURE
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
                <div style={{ width: '30%' }}>PRICE</div>
                <div style={{ width: '30%', textAlign: 'right' }}>SIZE</div>
                <div style={{ width: '40%', textAlign: 'right' }}>STATUS</div>
            </div>

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
                    {zones.map((zone, i) => {
                        const isAsk = zone.side === 'Sell';

                        // Check if we should render the "Current Price" indicator here
                        const nextZone = zones[i + 1];
                        const showPriceLine = nextZone && lastPrice <= zone.price && lastPrice > nextZone.price;

                        const isTopPrice = i === 0 && lastPrice > zone.price;
                        const isBottomPrice = i === zones.length - 1 && lastPrice < zone.price;

                        return (
                            <React.Fragment key={zone.price}>
                                {isTopPrice && <PriceIndicator price={lastPrice} id="active-price-indicator" />}

                                <div style={{
                                    display: 'flex',
                                    padding: '4px 1rem',
                                    fontSize: '0.9rem',
                                    backgroundColor: isAsk ? 'rgba(255, 0, 85, 0.05)' : 'rgba(0, 255, 157, 0.05)',
                                    borderLeft: `3px solid ${isAsk ? 'var(--color-sell)' : 'var(--color-buy)'}`,
                                    marginBottom: '1px',
                                    opacity: zone.status === 'Idle' ? 0.3 : 1
                                }}>
                                    <div style={{ width: '30%', color: isAsk ? 'var(--color-sell)' : 'var(--color-buy)' }}>
                                        {zone.price.toFixed(4)}
                                    </div>
                                    <div style={{ width: '30%', textAlign: 'right' }}>
                                        {zone.size.toFixed(4)}
                                    </div>
                                    <div style={{ width: '40%', textAlign: 'right', fontSize: '0.8rem', color: 'var(--text-muted)' }}>
                                        {zone.status.toUpperCase()}
                                    </div>
                                </div>

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

const PriceIndicator: React.FC<{ price: number; id?: string }> = ({ price, id }) => (
    <div id={id} style={{
        padding: '8px 1rem',
        background: 'rgba(255, 255, 255, 0.05)',
        borderTop: '1px dashed var(--accent-primary)',
        borderBottom: '1px dashed var(--accent-primary)',
        color: 'var(--accent-primary)',
        fontWeight: 'bold',
        textAlign: 'center',
        margin: '4px 0',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        gap: '0.5rem'
    }}>
        <span>▶</span>
        {price.toFixed(4)}
        <span>◀</span>
    </div>
);

export default OrderBook;

import React from 'react';
import { useBotStore } from '../context/WebSocketContext';
import type { OrderEvent } from '../types/schema';

const ActivityLog: React.FC = () => {
    const { orderHistory } = useBotStore();

    return (
        <div style={{
            background: 'var(--bg-card)',
            borderRadius: '8px',
            border: '1px solid var(--border-light)',
            padding: '1rem',
            flex: 1,
            minHeight: '300px',
            maxHeight: '400px',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden'
        }}>
            <div style={{ 
                display: 'flex', 
                justifyContent: 'space-between', 
                alignItems: 'center',
                marginBottom: '0.75rem'
            }}>
                <h3 style={{ margin: 0, color: 'var(--text-secondary)', fontSize: '1rem' }}>
                    ACTIVITY LOG
                </h3>
                {orderHistory.length > 0 && (
                    <span style={{ 
                        fontSize: '0.7rem', 
                        color: 'var(--text-muted)',
                        background: 'rgba(255,255,255,0.05)',
                        padding: '2px 8px',
                        borderRadius: '4px'
                    }}>
                        {orderHistory.length} events
                    </span>
                )}
            </div>

            <div style={{ 
                flex: 1, 
                overflowY: 'auto',
                display: 'flex',
                flexDirection: 'column',
                gap: '4px'
            }}>
                {orderHistory.length === 0 ? (
                    <div style={{ 
                        color: 'var(--text-muted)', 
                        fontSize: '0.85rem',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        height: '100%',
                        opacity: 0.6
                    }}>
                        <span>Waiting for order updates...</span>
                    </div>
                ) : (
                    orderHistory.map((order, idx) => (
                        <OrderEventRow key={`${order.cloid || order.oid}-${idx}`} order={order} />
                    ))
                )}
            </div>
        </div>
    );
};

const OrderEventRow: React.FC<{ order: OrderEvent }> = ({ order }) => {
    const isBuy = order.side === 'Buy';
    const isFilled = order.status === 'FILLED';
    
    // Determine colors
    const sideColor = isBuy ? 'var(--color-buy)' : 'var(--color-sell)';
    const statusColor = isFilled ? 'var(--color-buy)' : 
                        order.status === 'OPEN' ? 'var(--accent-primary)' : 
                        order.status === 'CANCELLED' ? 'var(--text-muted)' : 
                        'var(--color-sell)';
    
    const icon = isFilled ? '✓' : 
                 order.status === 'OPEN' ? '○' : 
                 order.status === 'CANCELLED' ? '✗' : '•';

    return (
        <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: '0.5rem',
            padding: '6px 8px',
            background: 'rgba(255,255,255,0.02)',
            borderRadius: '4px',
            borderLeft: `3px solid ${sideColor}`,
            fontSize: '0.8rem'
        }}>
            {/* Status Icon */}
            <span style={{ 
                color: statusColor, 
                fontWeight: 'bold',
                width: '16px',
                textAlign: 'center'
            }}>
                {icon}
            </span>

            {/* Side Badge */}
            <span style={{
                background: sideColor,
                color: '#000',
                padding: '1px 6px',
                borderRadius: '3px',
                fontSize: '0.65rem',
                fontWeight: 700,
                minWidth: '32px',
                textAlign: 'center'
            }}>
                {order.side.toUpperCase()}
            </span>

            {/* Size @ Price */}
            <span style={{ 
                flex: 1, 
                fontFamily: 'var(--font-mono)',
                color: 'var(--text-primary)'
            }}>
                {order.size.toFixed(4)} @ ${order.price.toFixed(2)}
            </span>

            {/* Status */}
            <span style={{ 
                color: statusColor,
                fontSize: '0.7rem',
                fontWeight: 600
            }}>
                {order.status}
            </span>

            {/* Fee (if filled) */}
            {isFilled && order.fee > 0 && (
                <span style={{ 
                    color: 'var(--text-muted)',
                    fontSize: '0.65rem'
                }}>
                    -${order.fee.toFixed(4)}
                </span>
            )}
        </div>
    );
};

export default ActivityLog;


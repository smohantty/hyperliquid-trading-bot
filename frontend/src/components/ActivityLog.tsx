import React from 'react';
import { useBotStore } from '../context/WebSocketContext';
import type { OrderEvent } from '../types/schema';

const ActivityLog: React.FC = () => {
    const { orderHistory } = useBotStore();

    return (
        <div style={{
            background: 'var(--bg-secondary)',
            borderRadius: '8px',
            border: '1px solid var(--border-color)',
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
            minHeight: '200px'
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
                    Activity
                </span>
                {orderHistory.length > 0 && (
                    <span style={{
                        fontSize: '10px',
                        color: 'var(--text-tertiary)',
                        background: 'var(--bg-tertiary)',
                        padding: '2px 6px',
                        borderRadius: '3px'
                    }}>
                        {orderHistory.length} events
                    </span>
                )}
            </div>

            {/* Activity List */}
            <div style={{ flex: 1, overflowY: 'auto' }}>
                {orderHistory.length === 0 ? (
                    <div style={{
                        padding: '40px 20px',
                        textAlign: 'center',
                        color: 'var(--text-tertiary)',
                        fontSize: '12px'
                    }}>
                        Waiting for orders...
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

    const sideColor = isBuy ? 'var(--color-buy)' : 'var(--color-sell)';
    const statusColor = isFilled ? 'var(--color-buy)' :
                        order.status === 'OPEN' ? 'var(--text-primary)' :
                        order.status === 'OPENING' ? 'var(--accent-yellow)' :
                        order.status === 'CANCELLED' ? 'var(--text-tertiary)' :
                        'var(--color-sell)';

    return (
        <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            padding: '8px 16px',
            fontSize: '11px',
            borderBottom: '1px solid var(--border-color)'
        }}>
            {/* Side Badge */}
            <span style={{
                background: `${sideColor}15`,
                color: sideColor,
                padding: '2px 6px',
                borderRadius: '3px',
                fontSize: '9px',
                fontWeight: 600,
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
                {order.size.toFixed(4)} <span style={{ color: 'var(--text-tertiary)' }}>@</span> ${order.price.toLocaleString(undefined, { minimumFractionDigits: 2 })}
            </span>

            {/* Status */}
            <span style={{
                color: statusColor,
                fontSize: '10px',
                fontWeight: 500
            }}>
                {order.status}
            </span>

            {/* Fee */}
            {isFilled && order.fee > 0 && (
                <span style={{
                    color: 'var(--text-tertiary)',
                    fontSize: '10px',
                    fontFamily: 'var(--font-mono)'
                }}>
                    -${order.fee.toFixed(4)}
                </span>
            )}
        </div>
    );
};

export default ActivityLog;

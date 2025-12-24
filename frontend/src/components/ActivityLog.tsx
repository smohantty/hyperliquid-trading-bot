import React, { useEffect, useRef } from 'react';
import { useBotStore } from '../context/WebSocketContext';
import type { OrderEvent } from '../types/schema';

const ActivityLog: React.FC = () => {
    const { orderHistory, config } = useBotStore();
    const szDecimals = config?.sz_decimals || 4;
    const scrollRef = useRef<HTMLDivElement>(null);

    // Auto-scroll to top on new orders
    useEffect(() => {
        if (scrollRef.current && orderHistory.length > 0) {
            scrollRef.current.scrollTop = 0;
        }
    }, [orderHistory.length]);

    return (
        <div className="card" style={{
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
            minHeight: '180px',
            maxHeight: '340px',
            animationDelay: '160ms'
        }}>
            {/* Header */}
            <div className="card-header">
                <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ color: 'var(--text-secondary)' }}>
                        <path d="M12 8v4l3 3" />
                        <circle cx="12" cy="12" r="10" />
                    </svg>
                    <span className="card-header-title">Activity</span>
                </div>
                {orderHistory.length > 0 && (
                    <span className="badge badge-muted">
                        {orderHistory.length} events
                    </span>
                )}
            </div>

            {/* Column Headers */}
            {orderHistory.length > 0 && (
                <div style={{
                    display: 'grid',
                    gridTemplateColumns: '70px 50px 65px 1fr 70px',
                    padding: '10px 16px',
                    fontSize: '9px',
                    color: 'var(--text-tertiary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.5px',
                    fontWeight: 600,
                    borderBottom: '1px solid var(--border-color)',
                    background: 'rgba(0, 0, 0, 0.2)',
                    alignItems: 'center'
                }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <span style={{
                            width: '10px',
                            display: 'inline-block',
                            textAlign: 'center',
                            fontSize: '10px',
                            opacity: 0
                        }}>●</span>
                        <span>Status</span>
                    </div>
                    <span>Side</span>

                    <span>Size</span>
                    <span>Price</span>

                    <span style={{ textAlign: 'right' }}>Fee</span>
                </div>
            )}

            {/* Activity List */}
            <div
                ref={scrollRef}
                style={{
                    flex: 1,
                    overflowY: 'auto',
                    overflowX: 'hidden'
                }}
            >
                {orderHistory.length === 0 ? (
                    <div style={{
                        padding: '50px 20px',
                        textAlign: 'center',
                        display: 'flex',
                        flexDirection: 'column',
                        alignItems: 'center',
                        gap: '12px'
                    }}>
                        <div style={{
                            width: '48px',
                            height: '48px',
                            borderRadius: '50%',
                            background: 'var(--bg-hover)',
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center'
                        }}>
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--text-tertiary)" strokeWidth="2">
                                <path d="M12 8v4l3 3" />
                                <circle cx="12" cy="12" r="10" />
                            </svg>
                        </div>
                        <span style={{
                            color: 'var(--text-tertiary)',
                            fontSize: '13px'
                        }}>
                            Waiting for orders...
                        </span>
                    </div>
                ) : (
                    orderHistory.map((order, idx) => (
                        <OrderEventRow
                            key={`${order.cloid || order.oid}-${idx}`}
                            order={order}
                            szDecimals={szDecimals}
                            isFirst={idx === 0}
                        />
                    ))
                )}
            </div>
        </div>
    );
};

const OrderEventRow: React.FC<{
    order: OrderEvent;
    szDecimals: number;
    isFirst?: boolean;
}> = ({ order, szDecimals, isFirst }) => {
    const isBuy = order.side === 'Buy';
    const isFilled = order.status === 'FILLED';

    const sideColor = isBuy ? 'var(--color-buy)' : 'var(--color-sell)';

    const getStatusConfig = () => {
        switch (order.status) {
            case 'FILLED':
                return {
                    icon: '✓',
                    label: 'Filled',
                    color: 'var(--color-buy)',
                    bg: 'var(--color-buy-bg)'
                };
            case 'OPEN':
                return {
                    icon: '●',
                    label: 'Open',
                    color: 'var(--accent-primary)',
                    bg: 'var(--accent-subtle)'
                };
            case 'OPENING':
                return {
                    icon: '◐',
                    label: 'Opening',
                    color: 'var(--color-warning)',
                    bg: 'rgba(255, 171, 0, 0.1)'
                };
            case 'CANCELLED':
                return {
                    icon: '✕',
                    label: 'Cancelled',
                    color: 'var(--text-tertiary)',
                    bg: 'transparent'
                };
            default:
                return {
                    icon: '!',
                    label: order.status,
                    color: 'var(--color-sell)',
                    bg: 'var(--color-sell-bg)'
                };
        }
    };

    const statusConfig = getStatusConfig();

    return (
        <div style={{
            display: 'grid',
            gridTemplateColumns: '70px 50px 65px 1fr 70px',
            alignItems: 'center',
            padding: '10px 16px',
            fontSize: '12px',
            borderBottom: '1px solid var(--border-color)',
            background: isFirst ? 'rgba(0, 240, 192, 0.02)' : 'transparent',
            animation: isFirst ? 'fadeIn 0.3s ease-out' : 'none',
            transition: 'background var(--transition-fast)'
        }}
            onMouseEnter={(e) => {
                e.currentTarget.style.background = 'var(--bg-hover)';
            }}
            onMouseLeave={(e) => {
                e.currentTarget.style.background = isFirst ? 'rgba(0, 240, 192, 0.02)' : 'transparent';
            }}
        >
            {/* Status - First Column */}
            <div style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px'
            }}>
                <span style={{
                    color: statusConfig.color,
                    fontSize: '10px'
                }}>
                    {statusConfig.icon}
                </span>
                <span style={{
                    color: statusConfig.color,
                    fontSize: '11px',
                    fontWeight: 600
                }}>
                    {statusConfig.label}
                </span>
            </div>

            {/* Side - Second Column */}
            <span style={{
                color: sideColor,
                fontSize: '11px',
                fontWeight: 600,
                textTransform: 'uppercase'
            }}>
                {order.side}
            </span>

            {/* Size - Third Column */}
            <span style={{
                fontFamily: 'var(--font-mono)',
                color: 'var(--text-primary)',
                fontWeight: 500,
                fontSize: '11px'
            }}>
                {order.size.toFixed(szDecimals)}
            </span>

            {/* Price - Fourth Column */}
            <span style={{
                color: 'var(--text-secondary)',
                fontFamily: 'var(--font-mono)',
                fontSize: '11px'
            }}>
                ${order.price.toLocaleString(undefined, { minimumFractionDigits: 2 })}
            </span>

            {/* Fee - Fifth Column */}
            <span style={{
                color: isFilled && order.fee > 0 ? 'var(--color-sell)' : 'var(--text-muted)',
                fontSize: '11px',
                fontFamily: 'var(--font-mono)',
                textAlign: 'right'
            }}>
                {isFilled && order.fee > 0 ? `-$${order.fee.toFixed(4)}` : '--'}
            </span>
        </div>
    );
};

export default ActivityLog;

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
                        <path d="M12 8v4l3 3"/>
                        <circle cx="12" cy="12" r="10"/>
                    </svg>
                    <span className="card-header-title">Activity</span>
                </div>
                {orderHistory.length > 0 && (
                    <span className="badge badge-muted">
                        {orderHistory.length} events
                    </span>
                )}
            </div>

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
                                <path d="M12 8v4l3 3"/>
                                <circle cx="12" cy="12" r="10"/>
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
    const sideBg = isBuy ? 'var(--color-buy-bg)' : 'var(--color-sell-bg)';

    const getStatusStyle = () => {
        switch (order.status) {
            case 'FILLED':
                return {
                    color: 'var(--color-buy)',
                    bg: 'var(--color-buy-bg)',
                    glow: 'var(--color-buy-glow)'
                };
            case 'OPEN':
                return {
                    color: 'var(--text-primary)',
                    bg: 'var(--bg-hover)',
                    glow: 'none'
                };
            case 'OPENING':
                return {
                    color: 'var(--color-warning)',
                    bg: 'rgba(255, 171, 0, 0.1)',
                    glow: 'var(--color-warning-glow)'
                };
            case 'CANCELLED':
                return {
                    color: 'var(--text-tertiary)',
                    bg: 'transparent',
                    glow: 'none'
                };
            default:
                return {
                    color: 'var(--color-sell)',
                    bg: 'var(--color-sell-bg)',
                    glow: 'var(--color-sell-glow)'
                };
        }
    };

    const statusStyle = getStatusStyle();

    return (
        <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: '12px',
            padding: '12px 18px',
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
            {/* Side Badge */}
            <span style={{
                background: sideBg,
                border: `1px solid ${sideColor}30`,
                color: sideColor,
                padding: '3px 8px',
                borderRadius: 'var(--radius-sm)',
                fontSize: '10px',
                fontWeight: 600,
                minWidth: '40px',
                textAlign: 'center',
                letterSpacing: '0.3px'
            }}>
                {order.side.toUpperCase()}
            </span>

            {/* Size @ Price */}
            <div style={{
                flex: 1,
                fontFamily: 'var(--font-mono)',
                display: 'flex',
                alignItems: 'center',
                gap: '6px'
            }}>
                <span style={{ color: 'var(--text-primary)', fontWeight: 500 }}>
                    {order.size.toFixed(szDecimals)}
                </span>
                <span style={{ color: 'var(--text-tertiary)' }}>@</span>
                <span style={{ color: 'var(--text-secondary)' }}>
                    ${order.price.toLocaleString(undefined, { minimumFractionDigits: 2 })}
                </span>
            </div>

            {/* Status */}
            <span style={{
                background: statusStyle.bg,
                color: statusStyle.color,
                padding: '3px 8px',
                borderRadius: 'var(--radius-sm)',
                fontSize: '10px',
                fontWeight: 600,
                letterSpacing: '0.3px',
                boxShadow: statusStyle.glow !== 'none' ? `0 0 10px ${statusStyle.glow}` : 'none'
            }}>
                {order.status}
            </span>

            {/* Fee */}
            {isFilled && order.fee > 0 && (
                <span style={{
                    color: 'var(--text-tertiary)',
                    fontSize: '11px',
                    fontFamily: 'var(--font-mono)',
                    minWidth: '60px',
                    textAlign: 'right'
                }}>
                    -${order.fee.toFixed(4)}
                </span>
            )}
        </div>
    );
};

export default ActivityLog;

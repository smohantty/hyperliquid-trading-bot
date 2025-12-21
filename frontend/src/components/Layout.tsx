import React, { type ReactNode } from 'react';
import { useBotStore } from '../context/WebSocketContext';

interface LayoutProps {
    children: ReactNode;
}

const Layout: React.FC<LayoutProps> = ({ children }) => {
    const { connectionStatus, config } = useBotStore();

    const getStatusColor = () => {
        switch (connectionStatus) {
            case 'connected': return 'var(--color-buy)';
            case 'connecting': return 'var(--accent-yellow)';
            case 'disconnected': return 'var(--color-sell)';
        }
    };

    return (
        <div style={{ minHeight: '100vh', background: 'var(--bg-primary)' }}>
            {/* Header */}
            <header style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: '0 24px',
                height: '48px',
                background: 'var(--bg-secondary)',
                borderBottom: '1px solid var(--border-color)'
            }}>
                {/* Left: Logo & Nav */}
                <div style={{ display: 'flex', alignItems: 'center', gap: '24px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" style={{ color: 'var(--accent-yellow)' }}>
                            <path d="M12 2L2 7L12 12L22 7L12 2Z" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                            <path d="M2 17L12 22L22 17" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                            <path d="M2 12L12 17L22 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                        </svg>
                        <span style={{
                            fontSize: '14px',
                            fontWeight: 600,
                            color: 'var(--text-primary)'
                        }}>
                            Grid Bot
                        </span>
                    </div>

                    {/* Nav Items */}
                    <nav style={{ display: 'flex', gap: '4px' }}>
                        <NavItem label="Dashboard" active />
                        <NavItem label="History" />
                        <NavItem label="Settings" />
                    </nav>
                </div>

                {/* Right: Status & Info */}
                <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                    {config && (
                        <div style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '8px',
                            padding: '6px 12px',
                            background: 'var(--bg-tertiary)',
                            borderRadius: '4px'
                        }}>
                            <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                                {config.symbol}
                            </span>
                            <span style={{
                                fontSize: '10px',
                                fontWeight: 600,
                                color: config.type === 'perp_grid' ? 'var(--accent-yellow)' : 'var(--accent-blue)',
                                background: config.type === 'perp_grid' ? 'rgba(240, 185, 11, 0.15)' : 'rgba(30, 144, 255, 0.15)',
                                padding: '2px 6px',
                                borderRadius: '3px'
                            }}>
                                {config.type === 'perp_grid' ? 'PERP' : 'SPOT'}
                            </span>
                        </div>
                    )}

                    <div style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '6px',
                        fontSize: '12px'
                    }}>
                        <div style={{
                            width: '6px',
                            height: '6px',
                            borderRadius: '50%',
                            backgroundColor: getStatusColor()
                        }} />
                        <span style={{ color: 'var(--text-secondary)' }}>
                            {connectionStatus === 'connected' ? 'Live' : connectionStatus}
                        </span>
                    </div>
                </div>
            </header>

            {/* Main Content */}
            <main style={{
                padding: '20px 24px',
                maxWidth: '1400px',
                margin: '0 auto'
            }}>
                {children}
            </main>
        </div>
    );
};

const NavItem: React.FC<{ label: string; active?: boolean }> = ({ label, active }) => (
    <button style={{
        background: active ? 'var(--bg-tertiary)' : 'transparent',
        border: 'none',
        padding: '6px 12px',
        borderRadius: '4px',
        fontSize: '12px',
        fontWeight: 500,
        color: active ? 'var(--text-primary)' : 'var(--text-tertiary)',
        cursor: 'pointer',
        transition: 'all 0.15s ease'
    }}>
        {label}
    </button>
);

export default Layout;

import React, { type ReactNode } from 'react';
import { useBotStore } from '../context/WebSocketContext';

interface LayoutProps {
    children: ReactNode;
}

const Layout: React.FC<LayoutProps> = ({ children }) => {
    const { connectionStatus } = useBotStore();

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
                height: '56px',
                background: 'var(--bg-secondary)',
                borderBottom: '1px solid var(--border-color)'
            }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <div style={{
                        width: '8px',
                        height: '8px',
                        borderRadius: '50%',
                        backgroundColor: getStatusColor()
                    }} />
                    <span style={{ 
                        fontSize: '16px', 
                        fontWeight: 600,
                        color: 'var(--text-primary)'
                    }}>
                        Hyperliquid Bot
                    </span>
                </div>
                <div style={{ 
                    fontSize: '12px', 
                    color: 'var(--text-secondary)',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '8px'
                }}>
                    <span style={{ color: getStatusColor(), fontWeight: 500 }}>
                        {connectionStatus.toUpperCase()}
                    </span>
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

export default Layout;

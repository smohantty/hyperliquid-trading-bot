import React, { type ReactNode } from 'react';
import { useBotStore } from '../context/WebSocketContext';

interface LayoutProps {
    children: ReactNode;
    title?: string;
}

const Layout: React.FC<LayoutProps> = ({ children, title = 'Hyperliquid Bot' }) => {
    const { connectionStatus } = useBotStore();

    const getStatusColor = () => {
        switch (connectionStatus) {
            case 'connected': return '#00ff9d';
            case 'connecting': return '#ffaa00';
            case 'disconnected': return '#ff0055';
        }
    };

    return (
        <div className="layout">
            {/* Top Bar */}
            <header style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: '1rem 2rem',
                borderBottom: '1px solid var(--border-light)',
                background: 'var(--bg-card)'
            }}>
                <div className="flex-row items-center gap-2">
                    <div style={{
                        width: '12px',
                        height: '12px',
                        borderRadius: '50%',
                        backgroundColor: getStatusColor(),
                        boxShadow: `0 0 8px ${getStatusColor()}`
                    }} />
                    <h1 style={{ margin: 0, fontSize: '1.2rem', fontWeight: 600 }}>{title}</h1>
                </div>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>
                    STATUS: <span style={{ color: getStatusColor() }}>{connectionStatus.toUpperCase()}</span>
                </div>
            </header>

            {/* Main Content */}
            <main style={{
                padding: '2rem',
                maxWidth: '1600px',
                margin: '0 auto',
                minHeight: 'calc(100vh - 70px)'
            }}>
                {children}
            </main>
        </div>
    );
};

export default Layout;

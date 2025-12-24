import React, { type ReactNode } from 'react';

interface LayoutProps {
    children: ReactNode;
}

const Layout: React.FC<LayoutProps> = ({ children }) => {

    return (
        <div style={{
            minHeight: '100vh',
            display: 'flex',
            flexDirection: 'column',
            position: 'relative'
        }}>
            {/* Header */}
            <header style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: '0 28px',
                height: '56px',
                background: 'rgba(10, 13, 18, 0.75)',
                backdropFilter: 'blur(20px) saturate(180%)',
                borderBottom: '1px solid var(--border-subtle)',
                position: 'sticky',
                top: 0,
                zIndex: 100,
                animation: 'fadeIn 0.4s ease-out'
            }}>
                {/* Left: Logo & Nav */}
                <div style={{ display: 'flex', alignItems: 'center', gap: '32px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                        {/* Logo Icon */}
                        <div style={{
                            width: '32px',
                            height: '32px',
                            borderRadius: '8px',
                            background: 'linear-gradient(135deg, var(--accent-primary) 0%, rgba(0, 240, 192, 0.4) 100%)',
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            boxShadow: '0 0 20px var(--accent-glow)'
                        }}>
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" style={{ color: '#000' }}>
                                <path d="M3 3h7v7H3V3z" fill="currentColor"/>
                                <path d="M14 3h7v7h-7V3z" fill="currentColor" opacity="0.6"/>
                                <path d="M3 14h7v7H3v-7z" fill="currentColor" opacity="0.6"/>
                                <path d="M14 14h7v7h-7v-7z" fill="currentColor" opacity="0.3"/>
                            </svg>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '0' }}>
                            <span style={{
                                fontSize: '15px',
                                fontWeight: 600,
                                color: 'var(--text-primary)',
                                letterSpacing: '-0.02em'
                            }}>
                                GridBot
                            </span>
                            <span style={{
                                fontSize: '10px',
                                color: 'var(--text-tertiary)',
                                letterSpacing: '0.5px'
                            }}>
                                HYPERLIQUID
                            </span>
                        </div>
                    </div>

                    {/* Nav Items */}
                    <nav style={{ display: 'flex', gap: '4px' }}>
                        <NavItem label="Dashboard" active />
                        <NavItem label="Analytics" />
                        <NavItem label="Settings" />
                    </nav>
                </div>

            </header>

            {/* Main Content */}
            <main style={{
                flex: 1,
                padding: '28px 32px',
                maxWidth: '1520px',
                width: '100%',
                margin: '0 auto'
            }}>
                {children}
            </main>
        </div>
    );
};

const NavItem: React.FC<{ label: string; active?: boolean }> = ({ label, active }) => (
    <button style={{
        background: active ? 'var(--bg-hover)' : 'transparent',
        border: 'none',
        padding: '8px 14px',
        borderRadius: 'var(--radius-sm)',
        fontSize: '13px',
        fontWeight: 500,
        color: active ? 'var(--text-primary)' : 'var(--text-tertiary)',
        cursor: 'pointer',
        transition: 'all var(--transition-fast)',
        position: 'relative'
    }}
    onMouseEnter={(e) => {
        if (!active) {
            e.currentTarget.style.color = 'var(--text-secondary)';
            e.currentTarget.style.background = 'rgba(255,255,255,0.03)';
        }
    }}
    onMouseLeave={(e) => {
        if (!active) {
            e.currentTarget.style.color = 'var(--text-tertiary)';
            e.currentTarget.style.background = 'transparent';
        }
    }}
    >
        {label}
        {active && (
            <div style={{
                position: 'absolute',
                bottom: '-2px',
                left: '50%',
                transform: 'translateX(-50%)',
                width: '20px',
                height: '2px',
                background: 'var(--accent-primary)',
                borderRadius: '1px',
                boxShadow: '0 0 10px var(--accent-glow)'
            }} />
        )}
    </button>
);

export default Layout;

import React, { useState, useEffect } from 'react';
import { WebSocketProvider } from '../context/WebSocketContext';
import ConnectionManager from './ConnectionManager';
import { getDefaultConnection, getStoredConnections, saveConnections } from '../utils/storage';
import type { BotConnection } from '../types/connection';

interface MultiBotLayoutProps {
    children: React.ReactNode;
}

const MultiBotLayout: React.FC<MultiBotLayoutProps> = ({ children }) => {
    const [connections, setConnections] = useState<BotConnection[]>([]);
    const [activeTabId, setActiveTabId] = useState<string>('');
    const [showManager, setShowManager] = useState(false);

    // Initialize connections
    useEffect(() => {
        const stored = getStoredConnections();
        if (stored.length === 0) {
            const def = getDefaultConnection();
            setConnections([def]);
            setActiveTabId(def.id);
            // We can choose to save this default or not. 
            // Saving it makes it explicit for the user to see/edit later.
            saveConnections([def]);
        } else {
            setConnections(stored);
            setActiveTabId(stored[0].id);
        }
    }, []);

    const handleConnectionsChange = (newConnections: BotConnection[]) => {
        setConnections(newConnections);
        // If the active tab was deleted, switch to the first available or empty
        if (!newConnections.find(c => c.id === activeTabId)) {
            if (newConnections.length > 0) {
                setActiveTabId(newConnections[0].id);
            } else {
                setActiveTabId('');
            }
        }
    };

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden' }}>
            {/* Top Bar: Tabs + Settings */}
            <div style={{
                display: 'flex',
                alignItems: 'center',
                backgroundColor: '#1a1a1a',
                borderBottom: '1px solid #333',
                padding: '0 16px',
                height: '48px',
                flexShrink: 0
            }}>
                <div style={{ display: 'flex', gap: '4px', flex: 1, overflowX: 'auto' }}>
                    {connections.map(conn => (
                        <button
                            key={conn.id}
                            onClick={() => setActiveTabId(conn.id)}
                            style={{
                                padding: '8px 16px',
                                backgroundColor: activeTabId === conn.id ? '#2e2e2e' : 'transparent',
                                color: activeTabId === conn.id ? '#fff' : '#888',
                                border: 'none',
                                borderBottom: activeTabId === conn.id ? '2px solid #0070f3' : '2px solid transparent',
                                cursor: 'pointer',
                                whiteSpace: 'nowrap',
                                fontWeight: activeTabId === conn.id ? 'bold' : 'normal',
                                transition: 'all 0.2s',
                                fontSize: '14px'
                            }}
                        >
                            {conn.name}
                        </button>
                    ))}
                </div>

                <div style={{ marginLeft: '16px' }}>
                    <button
                        onClick={() => setShowManager(true)}
                        style={{
                            padding: '6px 12px',
                            backgroundColor: '#333',
                            color: '#fff',
                            border: '1px solid #444',
                            borderRadius: '4px',
                            cursor: 'pointer',
                            fontSize: '12px'
                        }}
                    >
                        Manage Bots
                    </button>
                </div>
            </div>

            {/* Content Area */}
            <div style={{ flex: 1, position: 'relative', overflow: 'hidden' }}>
                {connections.length === 0 ? (
                    <div style={{
                        height: '100%',
                        display: 'flex',
                        flexDirection: 'column',
                        justifyContent: 'center',
                        alignItems: 'center',
                        color: '#666'
                    }}>
                        <p>No active connections.</p>
                        <button onClick={() => setShowManager(true)} style={{ marginTop: '10px', padding: '8px 16px', cursor: 'pointer' }}>Add Connection</button>
                    </div>
                ) : (
                    connections.map(conn => (
                        <div
                            key={conn.id}
                            style={{
                                display: activeTabId === conn.id ? 'block' : 'none',
                                height: '100%',
                                overflowY: 'auto'
                            }}
                        >
                            <WebSocketProvider url={conn.url}>
                                {children}
                            </WebSocketProvider>
                        </div>
                    ))
                )}
            </div>

            {/* Connection Manager Modal */}
            {showManager && (
                <ConnectionManager
                    onClose={() => setShowManager(false)}
                    onConnectionsChange={handleConnectionsChange}
                />
            )}
        </div>
    );
};

export default MultiBotLayout;

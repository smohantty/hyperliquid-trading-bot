import React, { useState, useEffect } from 'react';
import type { BotConnection } from '../types/connection';
import { getStoredConnections, saveConnections } from '../utils/storage';

interface ConnectionManagerProps {
    onClose: () => void;
    onConnectionsChange: (connections: BotConnection[]) => void;
}

const ConnectionManager: React.FC<ConnectionManagerProps> = ({ onClose, onConnectionsChange }) => {
    const [connections, setConnections] = useState<BotConnection[]>([]);
    const [newNotName, setNewBotName] = useState('');
    const [newBotUrl, setNewBotUrl] = useState('');

    useEffect(() => {
        const stored = getStoredConnections();
        if (stored.length === 0) {
            // If no connections, suggest the default one? 
            // Or just leave it empty and let the parent handle the default "implicit" connection?
            // Let's show the stored list.
        }
        setConnections(stored);
    }, []);

    const handleAdd = () => {
        if (!newNotName || !newBotUrl) return;
        const newConnection: BotConnection = {
            id: crypto.randomUUID(),
            name: newNotName,
            url: newBotUrl
        };
        const updated = [...connections, newConnection];
        setConnections(updated);
        saveConnections(updated);
        onConnectionsChange(updated);
        setNewBotName('');
        setNewBotUrl('');
    };

    const handleDelete = (id: string) => {
        const updated = connections.filter(c => c.id !== id);
        setConnections(updated);
        saveConnections(updated);
        onConnectionsChange(updated);
    };

    return (
        <div style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: 'rgba(0,0,0,0.8)',
            display: 'flex',
            justifyContent: 'center',
            alignItems: 'center',
            zIndex: 1000
        }}>
            <div style={{
                backgroundColor: '#1e1e1e',
                padding: '24px',
                borderRadius: '8px',
                width: '500px',
                maxWidth: '90%'
            }}>
                <h2 style={{ marginTop: 0 }}>Manage Connections</h2>

                <div style={{ marginBottom: '20px' }}>
                    <h3>Add New Bot</h3>
                    <div style={{ display: 'flex', gap: '10px', marginBottom: '10px' }}>
                        <input
                            type="text"
                            placeholder="Bot Name (e.g. Lighter Bot)"
                            value={newNotName}
                            onChange={e => setNewBotName(e.target.value)}
                            style={{ flex: 1, padding: '8px', borderRadius: '4px', border: '1px solid #444', background: '#333', color: 'white' }}
                        />
                        <input
                            type="text"
                            placeholder="WebSocket URL (e.g. ws://localhost:9001)"
                            value={newBotUrl}
                            onChange={e => setNewBotUrl(e.target.value)}
                            style={{ flex: 2, padding: '8px', borderRadius: '4px', border: '1px solid #444', background: '#333', color: 'white' }}
                        />
                    </div>
                    <button
                        onClick={handleAdd}
                        disabled={!newNotName || !newBotUrl}
                        style={{
                            padding: '8px 16px',
                            backgroundColor: '#0070f3',
                            color: 'white',
                            border: 'none',
                            borderRadius: '4px',
                            cursor: 'pointer'
                        }}
                    >
                        Add Connection
                    </button>
                </div>

                <div style={{ marginBottom: '20px' }}>
                    <h3>Saved Connections</h3>
                    {connections.length === 0 ? (
                        <p style={{ color: '#888' }}>No saved connections.</p>
                    ) : (
                        <ul style={{ listStyle: 'none', padding: 0 }}>
                            {connections.map(c => (
                                <li key={c.id} style={{
                                    display: 'flex',
                                    justifyContent: 'space-between',
                                    alignItems: 'center',
                                    padding: '10px',
                                    background: '#2a2a2a',
                                    marginBottom: '8px',
                                    borderRadius: '4px'
                                }}>
                                    <div>
                                        <strong>{c.name}</strong>
                                        <div style={{ fontSize: '0.85em', color: '#aaa' }}>{c.url}</div>
                                    </div>
                                    <button
                                        onClick={() => handleDelete(c.id)}
                                        style={{
                                            padding: '4px 8px',
                                            backgroundColor: '#ff4444',
                                            color: 'white',
                                            border: 'none',
                                            borderRadius: '4px',
                                            cursor: 'pointer'
                                        }}
                                    >
                                        Delete
                                    </button>
                                </li>
                            ))}
                        </ul>
                    )}
                </div>

                <div style={{ textAlign: 'right' }}>
                    <button
                        onClick={onClose}
                        style={{
                            padding: '8px 16px',
                            backgroundColor: '#444',
                            color: 'white',
                            border: 'none',
                            borderRadius: '4px',
                            cursor: 'pointer'
                        }}
                    >
                        Close
                    </button>
                </div>
            </div>
        </div>
    );
};

export default ConnectionManager;

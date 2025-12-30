import type { BotConnection } from '../types/connection';

const STORAGE_KEY = 'bot_connections';

export const getStoredConnections = (): BotConnection[] => {
    try {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (stored) {
            return JSON.parse(stored);
        }
    } catch (e) {
        console.error('Failed to load connections:', e);
    }
    return [];
};

export const saveConnections = (connections: BotConnection[]) => {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(connections));
    } catch (e) {
        console.error('Failed to save connections:', e);
    }
};

export const getDefaultConnection = (): BotConnection => {
    const port = import.meta.env.VITE_WS_PORT || '9000';
    // Fallback: if VITE_WS_URL is set, use it; otherwise construct from hostname + port
    // However, for the purpose of a "default connection" object to be editable,
    // we usually want a concrete URL.
    const defaultUrl = import.meta.env.VITE_WS_URL || `ws://${window.location.hostname}:${port}`;

    return {
        id: 'default',
        name: 'Default Bot',
        url: defaultUrl
    };
};

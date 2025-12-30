import React, { createContext, useContext, useEffect, useRef, useState } from 'react';
import {
    type StrategyConfig,
    type SpotGridSummary,
    type PerpGridSummary,
    type GridState,
    type WebSocketEvent,
    type OrderEvent,
    type SystemInfo
} from '../types/schema';

// Union type for summaries
type Summary =
    | { type: 'spot_grid'; data: SpotGridSummary }
    | { type: 'perp_grid'; data: PerpGridSummary };

interface WebSocketContextType {
    isConnected: boolean;
    config: StrategyConfig | null;
    summary: Summary | null;
    gridState: GridState | null;
    lastPrice: number | null;
    lastTickTime: number | null;
    orderHistory: OrderEvent[];
    connectionStatus: 'connecting' | 'connected' | 'disconnected';
    systemInfo: SystemInfo | null;
}

const WebSocketContext = createContext<WebSocketContextType | null>(null);

export const WebSocketProvider: React.FC<{ children: React.ReactNode; url: string }> = ({ children, url }) => {
    const [isConnected, setIsConnected] = useState(false);
    const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'disconnected'>('disconnected');
    const [config, setConfig] = useState<StrategyConfig | null>(null);
    const [summary, setSummary] = useState<Summary | null>(null);
    const [gridState, setGridState] = useState<GridState | null>(null);
    const [lastPrice, setLastPrice] = useState<number | null>(null);
    const [lastTickTime, setLastTickTime] = useState<number | null>(null);
    const [orderHistory, setOrderHistory] = useState<OrderEvent[]>([]);
    const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);

    const wsRef = useRef<WebSocket | null>(null);
    const reconnectTimeoutRef = useRef<number | null>(null);

    const connect = () => {
        try {
            setConnectionStatus('connecting');
            const ws = new WebSocket(url);
            wsRef.current = ws;

            ws.onopen = () => {
                console.log('Connected to Bot WebSocket:', url);
                setIsConnected(true);
                setConnectionStatus('connected');
                if (reconnectTimeoutRef.current) {
                    clearTimeout(reconnectTimeoutRef.current);
                    reconnectTimeoutRef.current = null;
                }
            };

            ws.onclose = () => {
                console.log('Disconnected from Bot WebSocket:', url);
                setIsConnected(false);
                setConnectionStatus('disconnected');
                // Auto-reconnect
                reconnectTimeoutRef.current = window.setTimeout(() => {
                    console.log('Attempting reconnect to', url);
                    connect();
                }, 3000);
            };

            ws.onerror = (err) => {
                console.error('WebSocket Error:', err);
                ws.close();
            };

            ws.onmessage = (event) => {
                try {
                    const message: WebSocketEvent = JSON.parse(event.data);

                    switch (message.event_type) {
                        case 'config':
                            setConfig(message.data);
                            break;
                        case 'info':
                            setSystemInfo(message.data);
                            break;
                        case 'spot_grid_summary':
                            setSummary({ type: 'spot_grid', data: message.data });
                            setLastPrice(message.data.price);
                            break;
                        case 'perp_grid_summary':
                            setSummary({ type: 'perp_grid', data: message.data });
                            setLastPrice(message.data.price);
                            break;
                        case 'grid_state':
                            setGridState(message.data);
                            setLastPrice(message.data.current_price);
                            break;
                        case 'market_update':
                            setLastPrice(message.data.price);
                            setLastTickTime(Date.now());
                            break;
                        case 'order_update':
                            setOrderHistory(prev => [message.data, ...prev].slice(0, 50)); // Keep last 50
                            break;
                        case 'error':
                            console.error('Bot error:', message.data);
                            break;
                    }
                } catch (e) {
                    console.error('Failed to parse message:', e);
                }
            };

        } catch (e) {
            console.error('Connection failed:', e);
            setConnectionStatus('disconnected');
        }
    };

    // Reconnect if URL changes
    useEffect(() => {
        connect();
        return () => {
            if (wsRef.current) {
                wsRef.current.close();
            }
            if (reconnectTimeoutRef.current) {
                clearTimeout(reconnectTimeoutRef.current);
            }
        };
    }, [url]);

    return (
        <WebSocketContext.Provider value={{
            isConnected,
            connectionStatus,
            config,
            summary,
            gridState,
            lastPrice,
            lastTickTime,
            orderHistory,
            systemInfo
        }}>
            {children}
        </WebSocketContext.Provider>
    );
};

export const useBotStore = () => {
    const context = useContext(WebSocketContext);
    if (!context) {
        throw new Error('useBotStore must be used within a WebSocketProvider');
    }
    return context;
};

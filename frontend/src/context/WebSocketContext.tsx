import React, { createContext, useContext, useEffect, useRef, useState } from 'react';
import {
    type StrategyConfig,
    type StatusSummary,
    type WebSocketEvent,
    type OrderEvent
} from '../types/schema';

interface WebSocketContextType {
    isConnected: boolean;
    config: StrategyConfig | null;
    summary: StatusSummary | null;
    lastPrice: number | null;
    lastTickTime: number | null;
    orderHistory: OrderEvent[];
    connectionStatus: 'connecting' | 'connected' | 'disconnected';
}

const WebSocketContext = createContext<WebSocketContextType | null>(null);

export const WebSocketProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const [isConnected, setIsConnected] = useState(false);
    const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'disconnected'>('disconnected');
    const [config, setConfig] = useState<StrategyConfig | null>(null);
    const [summary, setSummary] = useState<StatusSummary | null>(null);
    const [lastPrice, setLastPrice] = useState<number | null>(null);
    const [lastTickTime, setLastTickTime] = useState<number | null>(null);
    const [orderHistory, setOrderHistory] = useState<OrderEvent[]>([]);

    const wsRef = useRef<WebSocket | null>(null);
    const reconnectTimeoutRef = useRef<number | null>(null);

    const connect = () => {
        try {
            setConnectionStatus('connecting');
            const ws = new WebSocket('ws://localhost:9000');
            wsRef.current = ws;

            ws.onopen = () => {
                console.log('Connected to Bot WebSocket');
                setIsConnected(true);
                setConnectionStatus('connected');
                if (reconnectTimeoutRef.current) {
                    clearTimeout(reconnectTimeoutRef.current);
                    reconnectTimeoutRef.current = null;
                }
            };

            ws.onclose = () => {
                console.log('Disconnected from Bot WebSocket');
                setIsConnected(false);
                setConnectionStatus('disconnected');
                // Auto-reconnect
                reconnectTimeoutRef.current = window.setTimeout(() => {
                    console.log('Attempting reconnect...');
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
                        case 'summary':
                            setSummary(message.data);
                            setLastPrice(message.data.price);
                            break;
                        case 'market_update':
                            setLastPrice(message.data.price);
                            setLastTickTime(Date.now());
                            console.log('Tick:', message.data.price);
                            break;
                        case 'order_update':
                            setOrderHistory(prev => [message.data, ...prev].slice(0, 50)); // Keep last 50
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
    }, []);

    return (
        <WebSocketContext.Provider value={{
            isConnected,
            connectionStatus,
            config,
            summary,
            lastPrice,
            lastTickTime,
            orderHistory
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

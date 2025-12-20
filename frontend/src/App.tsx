import React from 'react';
import { WebSocketProvider } from './context/WebSocketContext';
import Layout from './components/Layout';
import HeaderMetrics from './components/HeaderMetrics';
import OrderBook from './components/OrderBook';
import ConfigPanel from './components/ConfigPanel';

const DashboardContent: React.FC = () => {
  return (
    <Layout>
      <HeaderMetrics />

      <div style={{ display: 'grid', gridTemplateColumns: 'minmax(300px, 1fr) 400px', gap: '2rem' }}>
        {/* Left Column: Config & Logs (Placeholder) */}
        <div className="flex-col gap-4">
          <ConfigPanel />

          {/* Placeholder for future Chart or Logs */}
          <div style={{
            background: 'var(--bg-card)',
            borderRadius: '8px',
            border: '1px solid var(--border-light)',
            padding: '1rem',
            flex: 1,
            minHeight: '300px'
          }}>
            <h3 style={{ marginTop: 0, color: 'var(--text-secondary)', fontSize: '1rem' }}>ACTIVITY LOG</h3>
            <div style={{ color: 'var(--text-muted)', fontSize: '0.9rem' }}>
              <p>Waiting for order updates...</p>
              {/* Order history list could go here */}
            </div>
          </div>
        </div>

        {/* Right Column: Order Book */}
        <div>
          <OrderBook />
        </div>
      </div>
    </Layout>
  );
};

const App: React.FC = () => {
  return (
    <WebSocketProvider>
      <DashboardContent />
    </WebSocketProvider>
  );
};

export default App;

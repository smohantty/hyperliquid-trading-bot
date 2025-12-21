import React from 'react';
import { WebSocketProvider } from './context/WebSocketContext';
import Layout from './components/Layout';
import HeaderMetrics from './components/HeaderMetrics';
import OrderBook from './components/OrderBook';
import ConfigPanel from './components/ConfigPanel';
import ActivityLog from './components/ActivityLog';

const DashboardContent: React.FC = () => {
  return (
    <Layout>
      <HeaderMetrics />

      <div style={{ display: 'grid', gridTemplateColumns: 'minmax(300px, 1fr) 400px', gap: '2rem' }}>
        {/* Left Column: Config & Activity Log */}
        <div className="flex-col gap-4">
          <ConfigPanel />
          <ActivityLog />
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

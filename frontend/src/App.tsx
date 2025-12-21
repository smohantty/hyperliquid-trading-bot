import React from 'react';
import { WebSocketProvider } from './context/WebSocketContext';
import Layout from './components/Layout';
import SummaryCard from './components/SummaryCard';
import ConfigPanel from './components/ConfigPanel';
import OrderBook from './components/OrderBook';
import ActivityLog from './components/ActivityLog';

const DashboardContent: React.FC = () => {
  return (
    <Layout>
      {/* Top Section: Summary & Config side by side */}
      <div style={{ 
        display: 'grid', 
        gridTemplateColumns: '1fr 1fr', 
        gap: '1.5rem',
        marginBottom: '1.5rem'
      }}>
        <SummaryCard />
        <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
          <ConfigPanel />
          <ActivityLog />
        </div>
      </div>

      {/* Bottom Section: Order Book full width */}
      <OrderBook />
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

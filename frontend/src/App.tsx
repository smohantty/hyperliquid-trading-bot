import React from 'react';
import MultiBotLayout from './components/MultiBotLayout';
import Layout from './components/Layout';
import SummaryCard from './components/SummaryCard';
import ConfigPanel from './components/ConfigPanel';
import OrderBook from './components/OrderBook';
import ActivityLog from './components/ActivityLog';

const DashboardContent: React.FC = () => {
  return (
    <Layout>
      {/* Main Grid Layout */}
      <div style={{
        display: 'grid',
        gridTemplateColumns: '1fr 420px',
        gridTemplateRows: 'auto auto',
        gap: '24px'
      }}>
        {/* Left Column: Summary Card (spans full height) */}
        <div style={{ gridRow: '1 / 3' }}>
          <SummaryCard />
        </div>

        {/* Right Column: Config + Activity stacked */}
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '20px'
        }}>
          <ConfigPanel />
          <ActivityLog />
        </div>
      </div>

      {/* Order Book - Full Width Below */}
      <div style={{ marginTop: '24px' }}>
        <OrderBook />
      </div>
    </Layout>
  );
};

const App: React.FC = () => {
  return (
    <MultiBotLayout>
      <DashboardContent />
    </MultiBotLayout>
  );
};

export default App;

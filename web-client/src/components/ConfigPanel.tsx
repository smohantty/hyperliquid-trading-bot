import React from 'react';
import { useBotStore } from '../context/WebSocketContext';

const ConfigPanel: React.FC = () => {
    const { config } = useBotStore();

    if (!config) return null;

    return (
        <div style={{
            background: 'var(--bg-card)',
            borderRadius: '8px',
            border: '1px solid var(--border-light)',
            padding: '1rem',
            marginBottom: '1rem' // Add some spacing
        }}>
            <h3 style={{ marginTop: 0, marginBottom: '1rem', fontSize: '1rem', color: 'var(--text-secondary)' }}>
                ACTIVE CONFIGURATION
            </h3>

            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: '1rem' }}>
                <ConfigItem label="Strategy Type" value={config.type} />
                <ConfigItem label="Symbol" value={config.symbol} />
                <ConfigItem label="Range" value={`${config.lower_price} - ${config.upper_price}`} />
                <ConfigItem label="Grid Count" value={config.grid_count} />
                <ConfigItem label="Investment" value={`$${config.total_investment}`} />
                <ConfigItem label="Spacing" value={config.grid_type} />

                {config.type === 'perp_grid' && (
                    <>
                        <ConfigItem label="Leverage" value={`${config.leverage}x`} />
                        <ConfigItem label="Bias" value={config.grid_bias?.toUpperCase() || 'NEUTRAL'} />
                    </>
                )}
            </div>
        </div>
    );
};

const ConfigItem: React.FC<{ label: string; value: string | number | undefined }> = ({ label, value }) => (
    <div style={{ display: 'flex', flexDirection: 'column' }}>
        <span style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>{label}</span>
        <span style={{ fontSize: '0.9rem', color: 'var(--text-primary)' }}>{value}</span>
    </div>
);

export default ConfigPanel;

import React from 'react';
import { useBotStore } from '../context/WebSocketContext';

const ConfigPanel: React.FC = () => {
    const { config } = useBotStore();

    if (!config) return null;

    const isPerp = config.type === 'perp_grid';

    return (
        <div style={{
            background: 'var(--bg-card)',
            borderRadius: '12px',
            border: '1px solid var(--border-light)',
            overflow: 'hidden'
        }}>
            {/* Header */}
            <div style={{
                padding: '0.75rem 1.25rem',
                borderBottom: '1px solid var(--border-light)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                background: 'rgba(255, 255, 255, 0.02)'
            }}>
                <span style={{ 
                    fontSize: '0.75rem', 
                    fontWeight: 600,
                    color: 'var(--text-secondary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em'
                }}>
                    ⚙️ Configuration
                </span>
                <span style={{
                    fontSize: '0.65rem',
                    color: 'var(--text-muted)',
                    background: 'rgba(255, 255, 255, 0.05)',
                    padding: '2px 8px',
                    borderRadius: '4px'
                }}>
                    {config.type.replace('_', ' ').toUpperCase()}
                </span>
            </div>

            {/* Config Grid */}
            <div style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(120px, 1fr))',
                gap: '1px',
                background: 'var(--border-light)'
            }}>
                <ConfigCell label="Symbol" value={config.symbol} />
                <ConfigCell label="Range" value={`${config.lower_price} - ${config.upper_price}`} />
                <ConfigCell label="Zones" value={config.grid_count.toString()} />
                <ConfigCell label="Investment" value={`$${config.total_investment}`} />
                <ConfigCell label="Spacing" value={config.grid_type} />
                
                {isPerp && (
                    <>
                        <ConfigCell label="Leverage" value={`${config.leverage}x`} highlight />
                        <ConfigCell 
                            label="Bias" 
                            value={config.grid_bias?.toUpperCase() || 'NEUTRAL'} 
                            highlight
                            highlightColor={
                                config.grid_bias === 'long' ? 'var(--color-buy)' : 
                                config.grid_bias === 'short' ? 'var(--color-sell)' : 
                                'var(--accent-primary)'
                            }
                        />
                        <ConfigCell 
                            label="Margin" 
                            value={config.is_isolated ? 'Isolated' : 'Cross'} 
                        />
                    </>
                )}
            </div>
        </div>
    );
};

const ConfigCell: React.FC<{ 
    label: string; 
    value: string;
    highlight?: boolean;
    highlightColor?: string;
}> = ({ label, value, highlight, highlightColor }) => (
    <div style={{ 
        padding: '0.75rem 1rem',
        background: 'var(--bg-card)'
    }}>
        <div style={{ 
            fontSize: '0.6rem', 
            color: 'var(--text-muted)', 
            marginBottom: '0.25rem',
            textTransform: 'uppercase',
            letterSpacing: '0.03em'
        }}>
            {label}
        </div>
        <div style={{ 
            fontSize: '0.85rem', 
            fontWeight: highlight ? 600 : 500, 
            color: highlight ? (highlightColor || 'var(--accent-primary)') : 'var(--text-primary)',
            fontFamily: 'var(--font-mono)'
        }}>
            {value}
        </div>
    </div>
);

export default ConfigPanel;

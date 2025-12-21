import React from 'react';
import { useBotStore } from '../context/WebSocketContext';

const ConfigPanel: React.FC = () => {
    const { config } = useBotStore();

    if (!config) return null;

    const isPerp = config.type === 'perp_grid';

    return (
        <div style={{
            background: 'var(--bg-secondary)',
            borderRadius: '8px',
            border: '1px solid var(--border-color)',
            overflow: 'hidden'
        }}>
            {/* Header */}
            <div style={{
                padding: '12px 16px',
                borderBottom: '1px solid var(--border-color)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center'
            }}>
                <span style={{ fontSize: '12px', fontWeight: 500, color: 'var(--text-secondary)' }}>
                    Configuration
                </span>
                <span style={{
                    fontSize: '10px',
                    color: 'var(--text-tertiary)',
                    background: 'var(--bg-tertiary)',
                    padding: '2px 6px',
                    borderRadius: '3px'
                }}>
                    {config.type.replace('_', ' ').toUpperCase()}
                </span>
            </div>

            {/* Config Grid */}
            <div style={{ display: 'flex', flexWrap: 'wrap' }}>
                <ConfigCell label="Symbol" value={config.symbol} />
                <ConfigCell label="Range" value={`$${config.lower_price.toLocaleString()} - $${config.upper_price.toLocaleString()}`} />
                <ConfigCell label="Zones" value={config.grid_count.toString()} />
                <ConfigCell label="Investment" value={`$${config.total_investment.toLocaleString()}`} />
                <ConfigCell label="Spacing" value={config.grid_type.charAt(0).toUpperCase() + config.grid_type.slice(1)} />

                {isPerp && (
                    <>
                        <ConfigCell
                            label="Leverage"
                            value={`${config.leverage}x`}
                            highlight
                        />
                        <ConfigCell
                            label="Bias"
                            value={config.grid_bias?.toUpperCase() || 'NEUTRAL'}
                            highlight
                            highlightColor={
                                config.grid_bias === 'long' ? 'var(--color-buy)' :
                                config.grid_bias === 'short' ? 'var(--color-sell)' :
                                'var(--accent-yellow)'
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
        flex: '1 1 auto',
        minWidth: '100px',
        padding: '12px 16px',
        borderRight: '1px solid var(--border-color)',
        borderBottom: '1px solid var(--border-color)'
    }}>
        <div style={{ fontSize: '10px', color: 'var(--text-tertiary)', marginBottom: '4px', textTransform: 'uppercase' }}>
            {label}
        </div>
        <div style={{
            fontSize: '12px',
            fontWeight: highlight ? 600 : 400,
            color: highlight ? (highlightColor || 'var(--accent-yellow)') : 'var(--text-primary)',
            fontFamily: 'var(--font-mono)'
        }}>
            {value}
        </div>
    </div>
);

export default ConfigPanel;

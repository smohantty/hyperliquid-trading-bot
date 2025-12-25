import React from 'react';
import { useBotStore } from '../context/WebSocketContext';
import Tooltip from './Tooltip';

// Format grid spacing: "0.228%" for geometric, "0.167% - 0.172%" for arithmetic
const formatSpacing = (spacing: [number, number] | undefined): string => {
    if (!spacing) return '--';
    const [min, max] = spacing;
    const decimals = min < 1 ? 3 : 2;
    const relativeDiff = Math.abs(max - min) / Math.max(min, max);
    if (relativeDiff < 0.01) {
        return `${min.toFixed(decimals)}%`;
    }
    return `${min.toFixed(decimals)}% - ${max.toFixed(decimals)}%`;
};

const ConfigPanel: React.FC = () => {
    const { config, summary } = useBotStore();

    if (!config) return null;

    const isPerp = config.type === 'perp_grid';
    const gridSpacingPct = summary?.data?.grid_spacing_pct;

    return (
        <div className="card" style={{
            overflow: 'hidden',
            animationDelay: '80ms'
        }}>
            {/* Header */}
            <div className="card-header">
                <span className="card-header-title">Configuration</span>
                <span className="badge badge-muted">
                    {config.type.replace('_', ' ').toUpperCase()}
                </span>
            </div>

            {/* Config Grid */}
            <div style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(4, 1fr)'
            }}>
                <ConfigCell label="Symbol" value={config.symbol} icon="coin" />
                <ConfigCell
                    label="Range"
                    value={`$${config.lower_price.toLocaleString()} - $${config.upper_price.toLocaleString()}`}
                    icon="range"
                />
                <ConfigCell label="Zones" value={config.grid_count.toString()} icon="grid" />
                <ConfigCell
                    label="Investment"
                    value={`$${config.total_investment.toLocaleString()}`}
                    icon="wallet"
                    highlight
                />

                {isPerp ? (
                    <>
                        <ConfigCell
                            label="Leverage"
                            value={`${config.leverage}x`}
                            icon="leverage"
                            highlight
                            highlightColor="var(--accent-primary)"
                        />
                        <ConfigCell
                            label="Grid Bias"
                            value={config.grid_bias?.toUpperCase() || '-'}
                            icon="trend"
                            highlight
                            highlightColor={
                                config.grid_bias === 'long' ? 'var(--color-buy)' :
                                    config.grid_bias === 'short' ? 'var(--color-sell)' :
                                        'var(--text-primary)'
                            }
                        />
                        <ConfigCell
                            label="Margin"
                            value={config.is_isolated ? 'Isolated' : 'Cross'}
                            icon="shield"
                        />
                        <ConfigCell
                            label="Spacing"
                            value={formatSpacing(gridSpacingPct)}
                            subValue={config.grid_type.charAt(0).toUpperCase() + config.grid_type.slice(1)}
                            icon="percent"
                            highlight
                            highlightColor="var(--accent-primary)"
                            tooltip="Profit margin per roundtrip (before fees)"
                            isLast
                        />
                    </>
                ) : (
                    <>
                        {/* Spot Grid Specific Fields */}
                        <ConfigCell
                            label="Grid Type"
                            value={config.grid_type.charAt(0).toUpperCase() + config.grid_type.slice(1)}
                            icon="grid"
                            highlight
                        />
                        {/* Spacer to align Spacing to end if needed, or just let it flow. 
                            We have 6 items for Spot: Symbol, Range, Zones, Inv, Type, Spacing.
                            4 cols -> Row 1: Sym, Range, Zones, Inv. Row 2: Type, Spacing, Empty, Empty.
                            This looks fine.
                        */}
                        <ConfigCell
                            label="Spacing"
                            value={formatSpacing(gridSpacingPct)}
                            icon="percent"
                            highlight
                            highlightColor="var(--accent-primary)"
                            tooltip="Profit margin per roundtrip (before fees)"
                        />
                    </>
                )}
            </div>
        </div>
    );
};

const icons: Record<string, React.ReactNode> = {
    coin: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="10" />
            <path d="M12 6v12M9 9.5c0-.8.7-1.5 1.5-1.5h3c.8 0 1.5.7 1.5 1.5s-.7 1.5-1.5 1.5h-3c-.8 0-1.5.7-1.5 1.5s.7 1.5 1.5 1.5h3c.8 0 1.5-.7 1.5-1.5" />
        </svg>
    ),
    range: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M3 12h18M8 8l-5 4 5 4M16 8l5 4-5 4" />
        </svg>
    ),
    grid: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <rect x="3" y="3" width="7" height="7" />
            <rect x="14" y="3" width="7" height="7" />
            <rect x="14" y="14" width="7" height="7" />
            <rect x="3" y="14" width="7" height="7" />
        </svg>
    ),
    wallet: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M20 12V8H6a2 2 0 0 1-2-2c0-1.1.9-2 2-2h12v4" />
            <path d="M4 6v12c0 1.1.9 2 2 2h14v-4" />
            <path d="M18 12a2 2 0 0 0 0 4h4v-4h-4z" />
        </svg>
    ),
    leverage: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M12 2L2 7l10 5 10-5-10-5z" />
            <path d="M2 17l10 5 10-5" />
            <path d="M2 12l10 5 10-5" />
        </svg>
    ),
    trend: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M23 6l-9.5 9.5-5-5L1 18" />
            <path d="M17 6h6v6" />
        </svg>
    ),
    shield: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
        </svg>
    ),
    spacing: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M21 10H3M21 6H3M21 14H3M21 18H3" />
        </svg>
    ),
    percent: (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="6" cy="6" r="3" />
            <circle cx="18" cy="18" r="3" />
            <path d="M20 4L4 20" />
        </svg>
    )
};

const ConfigCell: React.FC<{
    label: string;
    value: string;
    subValue?: string;
    icon?: string;
    highlight?: boolean;
    highlightColor?: string;
    tooltip?: string;
    isLast?: boolean;
}> = ({ label, value, subValue, icon, highlight, highlightColor, tooltip, isLast }) => (
    <div style={{
        padding: '16px 18px',
        borderRight: isLast ? 'none' : '1px solid var(--border-color)',
        borderBottom: '1px solid var(--border-color)',
        background: highlight ? `${highlightColor || 'var(--accent-primary)'}08` : 'transparent',
        transition: 'background var(--transition-fast)'
    }}>
        <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
            fontSize: '10px',
            color: 'var(--text-tertiary)',
            marginBottom: '8px',
            textTransform: 'uppercase',
            letterSpacing: '0.5px',
            fontWeight: 500
        }}>
            {icon && <span style={{ opacity: 0.7 }}>{icons[icon]}</span>}
            {tooltip ? (
                <Tooltip content={tooltip}>
                    <span style={{ cursor: 'help', borderBottom: '1px dotted var(--text-tertiary)' }}>
                        {label}
                    </span>
                </Tooltip>
            ) : (
                label
            )}
        </div>
        <div style={{
            fontSize: '13px',
            fontWeight: highlight ? 600 : 500,
            color: highlight ? (highlightColor || 'var(--accent-primary)') : 'var(--text-primary)',
            fontFamily: 'var(--font-mono)',
            letterSpacing: '-0.01em'
        }}>
            {value}
        </div>
        {subValue && (
            <div style={{
                fontSize: '10px',
                color: 'var(--text-tertiary)',
                marginTop: '4px'
            }}>
                {subValue}
            </div>
        )}
    </div>
);

export default ConfigPanel;

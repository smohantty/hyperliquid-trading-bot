import React, { useState, useRef } from 'react';
import { createPortal } from 'react-dom';

interface TooltipProps {
    content: string;
    children: React.ReactNode;
}

const Tooltip: React.FC<TooltipProps> = ({ content, children }) => {
    const [isVisible, setIsVisible] = useState(false);
    const triggerRef = useRef<HTMLDivElement>(null);
    const [coords, setCoords] = useState({ top: 0, left: 0 });

    const handleMouseEnter = () => {
        if (triggerRef.current) {
            const rect = triggerRef.current.getBoundingClientRect();
            setCoords({
                top: rect.top - 10,
                left: rect.left + rect.width / 2
            });
            setIsVisible(true);
        }
    };

    return (
        <div
            ref={triggerRef}
            style={{ position: 'relative', display: 'inline-flex' }}
            onMouseEnter={handleMouseEnter}
            onMouseLeave={() => setIsVisible(false)}
        >
            {children}
            {isVisible && createPortal(
                <div style={{
                    position: 'fixed',
                    top: coords.top,
                    left: coords.left,
                    transform: 'translate(-50%, -100%)',
                    padding: '8px 12px',
                    background: 'rgba(15, 19, 24, 0.95)',
                    backdropFilter: 'blur(12px) saturate(180%)',
                    border: '1px solid var(--border-strong)',
                    borderRadius: 'var(--radius-sm)',
                    color: 'var(--text-primary)',
                    fontSize: '11px',
                    fontWeight: 500,
                    whiteSpace: 'nowrap',
                    zIndex: 9999,
                    boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4), 0 0 0 1px rgba(255, 255, 255, 0.03) inset',
                    pointerEvents: 'none',
                    animation: 'fadeIn 0.15s ease-out'
                }}>
                    {content}
                    {/* Arrow */}
                    <div style={{
                        position: 'absolute',
                        top: '100%',
                        left: '50%',
                        marginLeft: '-5px',
                        width: 0,
                        height: 0,
                        borderLeft: '5px solid transparent',
                        borderRight: '5px solid transparent',
                        borderTop: '5px solid var(--border-strong)'
                    }} />
                    <div style={{
                        position: 'absolute',
                        top: '100%',
                        left: '50%',
                        marginLeft: '-4px',
                        marginTop: '-1px',
                        width: 0,
                        height: 0,
                        borderLeft: '4px solid transparent',
                        borderRight: '4px solid transparent',
                        borderTop: '4px solid rgba(15, 19, 24, 0.95)'
                    }} />
                </div>,
                document.body
            )}
        </div>
    );
};

export default Tooltip;

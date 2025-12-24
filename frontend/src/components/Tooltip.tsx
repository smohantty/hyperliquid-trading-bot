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
                top: rect.top - 8, // 8px gap above element
                left: rect.left + rect.width / 2 // Center horizontally
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
                    padding: '6px 10px',
                    background: 'var(--bg-secondary)',
                    border: '1px solid var(--border-color)',
                    borderRadius: '4px',
                    color: 'var(--text-primary)',
                    fontSize: '11px',
                    whiteSpace: 'nowrap',
                    zIndex: 9999,
                    boxShadow: '0 4px 6px rgba(0, 0, 0, 0.3)',
                    pointerEvents: 'none'
                }}>
                    {content}
                    {/* Arrow */}
                    <div style={{
                        position: 'absolute',
                        top: '100%',
                        left: '50%',
                        marginLeft: '-4px',
                        borderWidth: '4px',
                        borderStyle: 'solid',
                        borderColor: 'var(--border-color) transparent transparent transparent'
                    }} />
                    <div style={{
                        position: 'absolute',
                        top: '100%',
                        left: '50%',
                        marginLeft: '-4px',
                        marginTop: '-1px',
                        borderWidth: '4px',
                        borderStyle: 'solid',
                        borderColor: 'var(--bg-secondary) transparent transparent transparent'
                    }} />
                </div>,
                document.body
            )}
        </div>
    );
};

export default Tooltip;

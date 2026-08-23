import { type ReactNode, useCallback } from 'react';
import { Switch } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

type ToggleProps = TailwindProps & {
    label: ReactNode;
    value?: boolean;
    color?: string;
    onChange?: (value: boolean) => void;
};

// Controlled, not seeded from a prop. Both call sites are backed by persisted store state, and a copy held in local
// state silently diverges from it the moment anything other than this switch writes to the store - leaving the switch
// showing one thing while the app does another.
export const Toggle = ({ label, value = false, color, onChange, className = '' }: ToggleProps) => {
    const enabled = value;

    const handleClick = useCallback(() => {
        onChange?.(!enabled);
    }, [enabled, onChange]);

    return (
        <div className={`flex justify-between items-center ${className}`}>
            {label}

            <Switch
                size='small'
                checked={enabled}
                slotProps={{
                    thumb: {
                        style: { backgroundColor: enabled ? color : undefined },
                    },
                    track: {
                        style: { backgroundColor: enabled ? color : undefined },
                    },
                }}
                onClick={handleClick}
            />
        </div>
    );
};

import { type ReactNode, useCallback, useState } from 'react';
import { Switch } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

type ToggleProps = TailwindProps & {
    label: ReactNode;
    initialValue?: boolean;
    color?: string;
    onChange?: (value: boolean) => void;
};

export const Toggle = ({ label, initialValue = false, color, onChange, className = '' }: ToggleProps) => {
    const [enabled, setEnabled] = useState(initialValue);

    const handleClick = useCallback(() => {
        onChange?.(!enabled);
        setEnabled(!enabled);
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

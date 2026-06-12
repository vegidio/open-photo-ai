import { useMemo } from 'react';
import { Tooltip } from '@mui/material';
import { Icon } from '@/components/atoms/Icon';

type ExportQueueStateProps = {
    state: string;
};

export const ExportQueueState = ({ state }: ExportQueueStateProps) => {
    const [msg, color] = useMemo(() => {
        switch (state) {
            case 'RUNNING':
                return ['Processing', 'text-[#009aff]'];
            case 'COMPLETED':
                return ['Completed', 'text-[#009aff]'];
            case 'ERROR':
            case 'ERROR_DOWNLOAD':
                return ['Error', 'text-[#ff5555]'];
            default:
                return ['<Invisible>', ''];
        }
    }, [state]);

    return (
        <div className={`flex flex-row items-center gap-1 ${color}`}>
            <span className={`${state === 'IDLE' ? 'invisible' : ''}`}>{msg}</span>
            {state.startsWith('ERROR') && (
                <Tooltip title={state === 'ERROR' ? 'Something went wrong...' : 'Failed to download AI model'}>
                    <span>
                        <Icon option='info' className='size-3.5' />
                    </span>
                </Tooltip>
            )}
        </div>
    );
};

import { useMemo } from 'react';
import { Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { Icon } from '@/components/atoms/Icon';

type ExportQueueStateProps = {
    state: string;
};

export const ExportQueueState = ({ state }: ExportQueueStateProps) => {
    const { t } = useTranslation();

    const [msg, color] = useMemo(() => {
        switch (state) {
            case 'RUNNING':
                return [t('export.queue.processing'), 'text-[#009aff]'];
            case 'COMPLETED':
                return [t('export.queue.completed'), 'text-[#009aff]'];
            case 'ERROR':
            case 'ERROR_DOWNLOAD':
                return [t('export.queue.error'), 'text-[#ff5555]'];
            default:
                return ['<Invisible>', ''];
        }
    }, [state, t]);

    return (
        <div className={`flex flex-row items-center gap-1 ${color}`}>
            <span className={`${state === 'IDLE' ? 'invisible' : ''}`}>{msg}</span>
            {state.startsWith('ERROR') && (
                <Tooltip
                    title={state === 'ERROR' ? t('export.queue.errorTooltip') : t('export.queue.errorDownloadTooltip')}
                >
                    <span>
                        <Icon option='info' className='size-3.5' />
                    </span>
                </Tooltip>
            )}
        </div>
    );
};

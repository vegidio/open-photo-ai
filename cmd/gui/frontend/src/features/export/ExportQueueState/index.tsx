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
                return [t('export.queue.processing'), 'text-brand'];
            case 'COMPLETED':
                return [t('export.queue.completed'), 'text-brand'];
            case 'ERROR':
            case 'ERROR_DOWNLOAD':
                return [t('export.queue.error'), 'text-danger'];
            default:
                // No message for this state. A non-breaking space rather than a literal keeps the row's height and
                // baseline while showing nothing - the placeholder that used to sit here was untranslated English,
                // and it was hidden only for IDLE, so any state this switch did not know about rendered "<Invisible>"
                // on screen.
                return ['\u00a0', ''];
        }
    }, [state, t]);

    return (
        <div className={`flex flex-row items-center gap-1 ${color}`}>
            <span className={`${msg === '\u00a0' ? 'invisible' : ''}`}>{msg}</span>
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

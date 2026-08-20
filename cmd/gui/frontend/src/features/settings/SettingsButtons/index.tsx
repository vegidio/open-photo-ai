import { useTranslation } from 'react-i18next';
import type { TailwindProps } from '@/utils/TailwindProps';
import { Button } from '@/components/atoms/Button';

type SettingsButtonsProps = TailwindProps & {
    onCancel: () => void;
    onSave: () => void;
};

export const SettingsButtons = ({ onCancel, onSave, className = '' }: SettingsButtonsProps) => {
    const { t } = useTranslation();

    return (
        <div className={`${className} flex gap-3`}>
            <Button option='secondary' className='w-20' onClick={onCancel}>
                {t('common.cancel')}
            </Button>
            <Button option='primary' className='w-20' onClick={onSave}>
                {t('common.save')}
            </Button>
        </div>
    );
};

import type { TailwindProps } from '@/utils/TailwindProps';
import { Button } from '@/components/atoms/Button';

type SettingsButtonsProps = TailwindProps & {
    onCancel: () => void;
    onSave: () => void;
};

export const SettingsButtons = ({ onCancel, onSave, className = '' }: SettingsButtonsProps) => {
    return (
        <div className={`${className} flex gap-3`}>
            <Button option='secondary' className='w-20' onClick={onCancel}>
                Cancel
            </Button>
            <Button option='primary' className='w-20' onClick={onSave}>
                Save
            </Button>
        </div>
    );
};

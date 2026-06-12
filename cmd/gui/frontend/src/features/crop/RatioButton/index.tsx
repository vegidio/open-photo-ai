import { Typography } from '@mui/material';
import type { IconType } from 'react-icons';

type RatioButtonProps = {
    label: string;
    icon: IconType;
    selected: boolean;
    onClick: () => void;
};

export const RatioButton = ({ label, icon: RatioIcon, selected, onClick }: RatioButtonProps) => {
    return (
        <button type='button' onClick={onClick} className='flex items-center gap-3 text-left'>
            <span
                className={`flex size-12 shrink-0 items-center justify-center rounded-full transition-colors ${
                    selected ? 'bg-[#009aff] text-[#f2f2f2]' : 'bg-neutral-700 text-neutral-200'
                }`}
            >
                <RatioIcon className='size-6' />
            </span>

            <Typography variant='subtitle2' className={selected ? 'text-white' : 'text-neutral-300'}>
                {label}
            </Typography>
        </button>
    );
};

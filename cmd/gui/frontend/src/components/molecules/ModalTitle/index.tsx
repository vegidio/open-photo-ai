import { Divider, Typography } from '@mui/material';
import { IconButton } from '@/components/atoms/IconButton';

type ModalTitleProps = {
    title: string;
    onClose?: () => void;
};

export const ModalTitle = ({ title, onClose }: ModalTitleProps) => {
    return (
        <div className='flex flex-col'>
            <div className='flex flex-row h-10 justify-between items-center'>
                <Typography className='text-xs font-medium ml-3 text-[#9e9e9e]'>{title}</Typography>
                <IconButton option='close' size='small' className='mr-1 text-[#9e9e9e]' onClick={onClose} />
            </div>

            <Divider />
        </div>
    );
};

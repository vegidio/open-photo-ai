import { Typography } from '@mui/material';
import { Button } from '@/components/atoms/Button';

type FaceSelectorProps = {
    onClick: () => void;
};

export const FaceSelector = ({ onClick }: FaceSelectorProps) => {
    return (
        <div className='flex flex-col gap-2'>
            <Typography align='center' className='text-[13px] text-[#b0b0b0]'>
                You can select individual faces
                <br />
                that you want to enhance.
            </Typography>

            <Button option='tertiary' onClick={onClick}>
                Select faces
            </Button>
        </div>
    );
};

import { Typography } from '@mui/material';
import { Button } from '@/components/atoms/Button';

type FaceSelectorProps = {
    selectedCount: number;
    onClick: () => void;
};

export const FaceSelector = ({ selectedCount, onClick }: FaceSelectorProps) => {
    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2'>Faces</Typography>

            <Typography align='center' className='text-[13px] text-[#b0b0b0]'>
                You can select individual
                <br />
                faces that you want to enhance.
            </Typography>

            <Button option='tertiary' onClick={onClick}>
                Select faces ({selectedCount})
            </Button>
        </div>
    );
};

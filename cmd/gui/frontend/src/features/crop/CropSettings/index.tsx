import { Button, Divider, Typography } from '@mui/material';
import type { IconType } from 'react-icons';
import { MdCropFree, MdCropLandscape, MdCropPortrait, MdCropSquare } from 'react-icons/md';
import { CropDimensions } from '@/features/crop/CropDimensions';
import { RatioButton } from '@/features/crop/RatioButton';

type RatioOption = {
    key: string;
    label: string;
    icon: IconType;
    value?: number;
};

const RATIOS: RatioOption[] = [
    { key: 'free', label: 'Free', icon: MdCropFree },
    { key: 'square', label: 'Square', icon: MdCropSquare, value: 1 },
    { key: '5:4', label: '5:4', icon: MdCropLandscape, value: 5 / 4 },
    { key: '4:5', label: '4:5', icon: MdCropPortrait, value: 4 / 5 },
    { key: '4:3', label: '4:3', icon: MdCropLandscape, value: 4 / 3 },
    { key: '3:4', label: '3:4', icon: MdCropPortrait, value: 3 / 4 },
    { key: '3:2', label: '3:2', icon: MdCropLandscape, value: 3 / 2 },
    { key: '2:3', label: '2:3', icon: MdCropPortrait, value: 2 / 3 },
    { key: '16:9', label: '16:9', icon: MdCropLandscape, value: 16 / 9 },
    { key: '9:16', label: '9:16', icon: MdCropPortrait, value: 9 / 16 },
];

type CropSettingsProps = {
    selected: string;
    onSelect: (key: string, aspectRatio?: number) => void;
    width: string;
    height: string;
    onWidthCommit: (value: number) => void;
    onHeightCommit: (value: number) => void;
    onSwap: () => void;
    onCancel: () => void;
    onApply: () => void;
};

export const CropSettings = ({
    selected,
    onSelect,
    width,
    height,
    onWidthCommit,
    onHeightCommit,
    onSwap,
    onCancel,
    onApply,
}: CropSettingsProps) => {
    return (
        <div className='flex flex-col w-64 shrink-0 overflow-y-auto bg-[#212121] p-4 gap-2'>
            <Typography variant='body2'>Aspect Ratio</Typography>

            <div className='grid grid-cols-2 my-1 gap-x-2 gap-y-4'>
                {RATIOS.map(({ key, label, icon, value }) => (
                    <RatioButton
                        key={key}
                        label={label}
                        icon={icon}
                        selected={key === selected}
                        onClick={() => onSelect(key, value)}
                    />
                ))}
            </div>

            <Divider className='my-2' />

            <Typography variant='body2'>Dimensions</Typography>

            <CropDimensions
                width={width}
                height={height}
                onWidthCommit={onWidthCommit}
                onHeightCommit={onHeightCommit}
                onSwap={onSwap}
            />

            <Typography variant='body2' className='mt-6 text-center text-[#b0b0b0]'>
                You can use the mouse wheel or the trackpad pinch to zoom in and out the image on the left.
            </Typography>

            <div className='flex-1' />

            <div className='flex gap-3'>
                <Button
                    variant='contained'
                    onClick={onCancel}
                    className='flex-1 bg-[#353535] hover:bg-[#171717] text-[#f2f2f2] normal-case font-normal'
                >
                    Cancel
                </Button>

                <Button
                    variant='contained'
                    onClick={onApply}
                    className='flex-1 bg-[#009aff] hover:bg-[#007eff] disabled:opacity-50 text-[#f2f2f2] normal-case font-normal'
                >
                    Apply
                </Button>
            </div>
        </div>
    );
};

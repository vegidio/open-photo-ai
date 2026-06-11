import { Divider, Typography } from '@mui/material';
import type { IconType } from 'react-icons';
import { MdCropFree, MdCropLandscape, MdCropPortrait, MdCropSquare } from 'react-icons/md';
import { CropDimensions } from '@/components/molecules/CropDimensions';
import { RatioButton } from '@/components/molecules/RatioButton';

type RatioOption = {
    key: string;
    label: string;
    icon: IconType;
    value?: number;
};

const RATIOS: RatioOption[] = [
    { key: 'free', label: 'Free', icon: MdCropFree },
    { key: 'square', label: 'Square', icon: MdCropSquare, value: 1 },
    { key: '16:9', label: '16:9', icon: MdCropLandscape, value: 16 / 9 },
    { key: '9:16', label: '9:16', icon: MdCropPortrait, value: 9 / 16 },
    { key: '3:2', label: '3:2', icon: MdCropLandscape, value: 3 / 2 },
    { key: '2:3', label: '2:3', icon: MdCropPortrait, value: 2 / 3 },
    { key: '4:3', label: '4:3', icon: MdCropLandscape, value: 4 / 3 },
    { key: '3:4', label: '3:4', icon: MdCropPortrait, value: 3 / 4 },
    { key: '5:4', label: '5:4', icon: MdCropLandscape, value: 5 / 4 },
    { key: '4:5', label: '4:5', icon: MdCropPortrait, value: 4 / 5 },
];

type AspectRatioSelectorProps = {
    selected: string;
    onSelect: (key: string, aspectRatio?: number) => void;
    width: string;
    height: string;
    onWidthCommit: (value: number) => void;
    onHeightCommit: (value: number) => void;
    onSwap: () => void;
};

export const AspectRatioSelector = ({
    selected,
    onSelect,
    width,
    height,
    onWidthCommit,
    onHeightCommit,
    onSwap,
}: AspectRatioSelectorProps) => {
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
        </div>
    );
};

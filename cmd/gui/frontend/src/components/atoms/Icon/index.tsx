import {
    MdBlurOn,
    MdChangeHistory,
    MdClose,
    MdCrop,
    MdCropLandscape,
    MdFlip,
    MdInfoOutline,
    MdOpenInFull,
    MdOutlineFaceRetouchingNatural,
    MdOutlineLightMode,
    MdOutlinePalette,
    MdRotate90DegreesCcw,
    MdSplitscreen,
} from 'react-icons/md';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

export type IconName =
    | 'close'
    | 'info'
    | 'denoise'
    | 'face_recovery'
    | 'light_adjustment'
    | 'color_balance'
    | 'upscale'
    | 'sharpen'
    | 'crop'
    | 'rotate'
    | 'flip_horizontal'
    | 'flip_vertical'
    | 'preview_full'
    | 'preview_side'
    | 'preview_split';

type IconProps = TailwindProps & {
    option: IconName;
};

export const Icon = ({ option, className = '' }: IconProps) => {
    switch (option) {
        case 'close':
            return <MdClose className={className} />;
        case 'info':
            return <MdInfoOutline className={className} />;
        case 'denoise':
            return <MdBlurOn className={className} />;
        case 'face_recovery':
            return <MdOutlineFaceRetouchingNatural className={className} />;
        case 'light_adjustment':
            return <MdOutlineLightMode className={className} />;
        case 'color_balance':
            return <MdOutlinePalette className={className} />;
        case 'upscale':
            return <MdOpenInFull className={className} />;
        case 'sharpen':
            return <MdChangeHistory className={className} />;
        case 'crop':
            return <MdCrop className={className} />;
        case 'rotate':
            return <MdRotate90DegreesCcw className={`-scale-x-100 ${className}`} />;
        case 'flip_horizontal':
            return <MdFlip className={className} />;
        case 'flip_vertical':
            return <MdFlip className={`rotate-90 ${className}`} />;
        case 'preview_full':
            return <MdCropLandscape className={className} />;
        case 'preview_side':
            return <MdSplitscreen className={`rotate-90 ${className}`} />;
        case 'preview_split':
            return <MdFlip className={`-scale-x-100 ${className}`} />;
    }
};

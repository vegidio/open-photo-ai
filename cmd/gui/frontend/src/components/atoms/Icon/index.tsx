import {
    MdClose,
    MdCropLandscape,
    MdFlip,
    MdInfoOutline,
    MdOpenInFull,
    MdOutlineFaceRetouchingNatural,
    MdSplitscreen,
} from 'react-icons/md';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

export type IconName =
    | 'close'
    | 'face_recovery'
    | 'info'
    | 'upscale'
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
        case 'face_recovery':
            return <MdOutlineFaceRetouchingNatural className={className} />;
        case 'info':
            return <MdInfoOutline className={className} />;
        case 'upscale':
            return <MdOpenInFull className={className} />;
        case 'preview_full':
            return <MdCropLandscape className={className} />;
        case 'preview_side':
            return <MdSplitscreen className={`rotate-90 ${className}`} />;
        case 'preview_split':
            return <MdFlip className={`transform -scale-x-100 ${className}`} />;
    }
};

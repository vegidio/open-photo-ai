import { MdClose, MdOpenInFull, MdOutlineFaceRetouchingNatural } from 'react-icons/md';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

type IconName = 'close' | 'face_recovery' | 'upscale';

type IconProps = TailwindProps & {
    option: IconName;
};

export const Icon = ({ option, className = '' }: IconProps) => {
    switch (option) {
        case 'close':
            return <MdClose className={className} />;
        case 'face_recovery':
            return <MdOutlineFaceRetouchingNatural className={className} />;
        case 'upscale':
            return <MdOpenInFull className={className} />;
    }
};

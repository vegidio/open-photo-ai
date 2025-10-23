import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { PreviewEmpty } from './PreviewEmpty';
import { PreviewImageSideBySide } from './PreviewImageSideBySide.tsx';
import { useFileStore } from '@/stores';

export const Preview = ({ className = '' }: TailwindProps) => {
    const files = useFileStore((state) => state.files);

    return (
        <div
            id='preview'
            className={`flex items-center justify-center bg-[#171717] [background-image:radial-gradient(#383838_1px,transparent_1px)] [background-size:3rem_3rem] ${className}`}
        >
            {files.length === 0 ? <PreviewEmpty /> : <PreviewImageSideBySide />}
        </div>
    );
};

import { PreviewEmpty } from './PreviewEmpty';
import { PreviewImageSideBySide } from './PreviewImageSideBySide.tsx';
import { useFileStore } from '@/stores';

interface PreviewProps {
    className?: string;
}

export const Preview = ({ className = '' }: PreviewProps) => {
    const filePaths = useFileStore((state) => state.filePaths);

    return (
        <div
            id='preview'
            className={`flex items-center justify-center bg-[#171717] [background-image:radial-gradient(#383838_1px,transparent_1px)] [background-size:3rem_3rem] ${className}`}
        >
            {filePaths.length === 0 ? <PreviewEmpty /> : <PreviewImageSideBySide />}
        </div>
    );
};

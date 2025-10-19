import { PreviewEmpty } from './PreviewEmpty';
import { PreviewImageSideBySide } from './PreviewImageSideBySide.tsx';

interface PreviewProps {
    className?: string;
}

export const Preview = ({ className = '' }: PreviewProps) => {
    return (
        <div
            id="preview"
            className={`size-full bg-[#171717] [background-image:radial-gradient(#383838_1px,transparent_1px)] [background-size:3rem_3rem] ${className}`}
        >
            {/*<PreviewEmpty />*/}

            <PreviewImageSideBySide />
        </div>
    );
};

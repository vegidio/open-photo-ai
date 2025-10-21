import { useState } from 'react';
import { ProcessImage } from '../../../bindings/gui/services/imageservice.ts';
import { useFileStore } from '../../stores/files.ts';

type SidebarExportProps = {
    className?: string;
};

export const SidebarExport = ({ className = '' }: SidebarExportProps) => {
    const filePaths = useFileStore((state) => state.filePaths);
    const selectedIndex = useFileStore((state) => state.selectedIndex);

    const [base64Image, setBase64Image] = useState<string>();

    const onExportClick = async () => {
        if (filePaths.length > 0) {
            const selectedPath = filePaths[selectedIndex];
            const base64 = await ProcessImage(selectedPath);
            setBase64Image(base64);
        }
    };

    return (
        <div>
            <button
                type="button"
                className={`${className} bg-[#009aff] hover:bg-[#007eff] text-[#f2f2f2] w-full h-12`}
                onClick={onExportClick}
            >
                Export
            </button>
        </div>
    );
};

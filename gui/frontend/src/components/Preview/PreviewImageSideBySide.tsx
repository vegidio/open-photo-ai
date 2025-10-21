import { useEffect, useState } from 'react';
import { ProcessImage } from '../../../bindings/gui/services/imageservice.ts';
import { useFileStore } from '../../stores/files.ts';
import { ImageBase64, ImagePath } from '../Image';

export const PreviewImageSideBySide = () => {
    const filePaths = useFileStore((state) => state.filePaths);
    const selectedIndex = useFileStore((state) => state.selectedIndex);

    const [base64Image, setBase64Image] = useState<string>();

    useEffect(() => {
        async function processImage() {
            if (filePaths.length > 0) {
                const selectedPath = filePaths[selectedIndex];
                const base64 = await ProcessImage(selectedPath);
                setBase64Image(base64);
            } else {
                setBase64Image(undefined);
            }
        }

        processImage();
    }, [filePaths, selectedIndex]);

    return (
        <div className="flex flex-row h-full w-full p-1 gap-1">
            <div className="flex-1 flex items-center justify-center">
                {filePaths.length > 0 && (
                    <ImagePath path={filePaths[selectedIndex]} className="w-full h-full object-contain" />
                )}
            </div>

            <div className="flex-1 flex items-center justify-center">
                {base64Image && <ImageBase64 base64={base64Image} className="w-full h-full object-contain" />}
            </div>
        </div>
    );
};

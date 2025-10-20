import { useEffect, useState } from 'react';
import { GetImageBytes } from '../../../bindings/gui/services/imageservice.ts';
import { useFileStore } from '../../stores/files.ts';

export const PreviewImageSideBySide = () => {
    const selectedFilePath = useFileStore((state) => state.filePaths[state.selectedFileIndex]);

    const [imageUrl, setImageUrl] = useState<string>();

    useEffect(() => {
        const loadImage = async () => {
            try {
                const image = await fetchImage(selectedFilePath);
                setImageUrl(image);
            } catch (e) {
                console.error(e);
            }
        };

        loadImage();
    }, [selectedFilePath]);

    return (
        <div className="flex flex-row h-full w-full p-2 gap-2">
            <div className="flex-1 flex items-center justify-center">
                {imageUrl && <img src={imageUrl} className="max-w-full max-h-full" />}
            </div>

            <div className="flex-1 flex items-center justify-center">
                <img src="https://picsum.photos/1920/1080" className="max-w-full max-h-full" />
            </div>
        </div>
    );
};

const fetchImage = async (path: string) => {
    try {
        const b64 = await GetImageBytes(path);
        const bytes = base64ToUint8Array(b64);
        const blob = new Blob([bytes as BlobPart], { type: 'image/jpeg' });
        return URL.createObjectURL(blob);
    } catch (e) {
        console.error(e);
    }
};

const base64ToUint8Array = (b64: string): Uint8Array => {
    const binary = atob(b64);
    const len = binary.length;
    const bytes = new Uint8Array(len);
    for (let i = 0; i < len; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
};

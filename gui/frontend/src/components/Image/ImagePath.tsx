import { useEffect, useState } from 'react';
import type { TailwindProps } from '@/utils';
import { GetImage } from '../../../bindings/gui/services/imageservice.ts';
import { ImageBase64 } from './ImageBase64';

type ImagePathProps = TailwindProps & {
    path: string;
};

export const ImagePath = ({ path, className = '' }: ImagePathProps) => {
    const [base64, setBase64] = useState<string>();

    useEffect(() => {
        async function loadFile() {
            try {
                const b64 = await GetImage(path);
                setBase64(b64);
            } catch (e) {
                setBase64(undefined);
                console.error(e);
            }
        }

        loadFile();
    }, [path]);

    return <>{base64 && <ImageBase64 base64={base64} className={className} />}</>;
};

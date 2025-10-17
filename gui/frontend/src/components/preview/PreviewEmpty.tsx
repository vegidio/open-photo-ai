import { Button } from 'flowbite-react';
import { MdFolderOpen } from 'react-icons/md';

export const PreviewEmpty = () => {
    return (
        <div className="flex h-full flex-col items-center justify-center">
            <MdFolderOpen className="size-20" />

            <div className="flex flex-col gap-1 pb-3">
                <p>
                    Drag and drop images or
                    <br />
                    folder to start editing
                </p>

                <p>or</p>
            </div>

            <Button>Browse images</Button>
        </div>
    );
};

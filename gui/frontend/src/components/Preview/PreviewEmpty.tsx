import { Button } from 'flowbite-react';
import { MdFolderOpen } from 'react-icons/md';
import { DialogService } from '../../../bindings/gui/services';

export const PreviewEmpty = () => {
    const onBrowseClick = () => {
        DialogService.OpenFileDialog();
    };

    return (
        <div className="flex h-full flex-col items-center justify-center">
            <MdFolderOpen className="size-20 bg-[#171717] text-[#009aff]" />

            <div className="flex bg-[#171717] flex-col text-center gap-3 mb-4">
                <p className="text-center text-[#f2f2f2]">
                    Drag and drop images or
                    <br />
                    folder to start editing
                </p>

                <p className="text-[#979797]">OR</p>
            </div>

            <Button className="bg-[#009aff] hover:bg-[#007eff] text-[#f2f2f2]" onClick={onBrowseClick}>
                Browse images
            </Button>
        </div>
    );
};

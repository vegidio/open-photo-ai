import { Dialog } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { CropSettings } from '@/features/crop/CropSettings';
import { ImageCropper } from '@/features/crop/ImageCropper';
import { RotateControls } from '@/features/crop/RotateControls';
import { useCropController } from '@/features/crop/useCropController.ts';
import { DOTTED_BACKGROUND } from '@/utils/constants.ts';

type CropRotateProps = {
    open: boolean;
    onClose: () => void;
};

export const CropRotate = ({ open, onClose }: CropRotateProps) => {
    const {
        cropperRef,
        baseImage,
        ratio,
        aspectRatio,
        fineRotation,
        cropWidth,
        cropHeight,
        onReady,
        syncDims,
        onRotationChange,
        onRotate90,
        onFlipHorizontal,
        onFlipVertical,
        onReset,
        onSelectRatio,
        onWidthCommit,
        onHeightCommit,
        onSwap,
        onApply,
    } = useCropController(open, onClose);

    if (!baseImage) return undefined;

    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick') onClose();
            }}
            slotProps={{
                paper: {
                    className:
                        'bg-[#212121] bg-none max-w-none max-h-none m-8 w-[calc(100vw-64px)] h-[calc(100vh-64px)] flex flex-col',
                },
            }}
        >
            <ModalTitle title='Crop/Rotate' onClose={onClose} />

            <div className={`flex-1 flex flex-row overflow-hidden ${DOTTED_BACKGROUND}`}>
                {/* Left */}
                <div className='flex-1 flex flex-col overflow-hidden p-8 gap-4'>
                    <ImageCropper
                        ref={cropperRef}
                        src={baseImage.url}
                        aspectRatio={aspectRatio}
                        onChange={syncDims}
                        onReady={onReady}
                    />

                    <RotateControls
                        rotation={fineRotation}
                        onRotationChange={onRotationChange}
                        onRotate90={onRotate90}
                        onFlipHorizontal={onFlipHorizontal}
                        onFlipVertical={onFlipVertical}
                        onReset={onReset}
                        className='w-1/2 self-center'
                    />
                </div>

                {/* Right */}
                <CropSettings
                    selected={ratio}
                    onSelect={onSelectRatio}
                    width={String(cropWidth)}
                    height={String(cropHeight)}
                    onWidthCommit={onWidthCommit}
                    onHeightCommit={onHeightCommit}
                    onSwap={onSwap}
                    onCancel={onClose}
                    onApply={onApply}
                />
            </div>
        </Dialog>
    );
};

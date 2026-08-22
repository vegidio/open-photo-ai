import { useState } from 'react';
import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { FaceSelector } from '@/features/enhancements/FaceSelector';
import { ModelSelector } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { FaceToggle } from '@/features/faces';
import { useCurrentFile, useFileDisabledFaces, useFileFaces, useModelOptions, useOptionEnhancement } from '@/hooks';
import { buildSelection } from '@/utils/enhancement';

type OptionsFaceRecoveryProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsFaceRecovery = ({ anchorEl, open, onClose }: OptionsFaceRecoveryProps) => {
    const { t } = useTranslation();

    const options = useModelOptions('fr');

    // Face recovery has no per-run amount, so only the model half of the hook is used. The file is still needed
    // directly, for the face toggle this popover opens.
    const { model, onModelChange } = useOptionEnhancement('fr', (nextModel) => buildSelection('fr', nextModel));

    const file = useCurrentFile();
    const selectedCount = useFileFaces(file).length - useFileDisabledFaces(file).size;
    const [facesOpen, setFacesOpen] = useState(false);

    if (!file) return undefined;

    return (
        <OptionsPopover
            title={t('enhancements.faceRecovery.name')}
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
            hideBackdrop={false}
        >
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <FaceSelector selectedCount={selectedCount} onClick={() => setFacesOpen(true)} />
            </div>

            <FaceToggle file={file} open={facesOpen} onClose={() => setFacesOpen(false)} />
        </OptionsPopover>
    );
};

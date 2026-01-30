import { useEffect } from 'react';
import { Dialog, Divider } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { SettingsButtons } from '@/components/molecules/SettingsButtons';
import { SettingsList } from '@/components/organisms/SettingsList';
import { SettingsMenu } from '@/components/organisms/SettingsMenu';
import { useSettingsStore } from '@/stores';

type SettingsProps = {
    section: 'application' | 'models';
    open: boolean;
    onClose: () => void;
};

export const Settings = ({ section, open, onClose }: SettingsProps) => {
    const saveSnapshot = useSettingsStore((state) => state.saveSnapshot);
    const restoreSnapshot = useSettingsStore((state) => state.restoreSnapshot);

    const onCancel = () => {
        restoreSnapshot();
        onClose();
    };

    // biome-ignore lint/correctness/useExhaustiveDependencies: N/A
    useEffect(() => saveSnapshot(), []);

    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick') onCancel();
            }}
            slotProps={{
                paper: {
                    className: 'bg-[#212121] w-[48rem] h-[40rem] max-w-full bg-none',
                },
            }}
        >
            <ModalTitle title='Settings' onClose={onCancel} />

            <div className='flex flex-row flex-1 overflow-hidden'>
                <SettingsMenu className='w-52 px-2 pt-2' />

                <Divider orientation='vertical' flexItem className='border-[#171717] my-0.5' />

                <div className='flex flex-col flex-1'>
                    <SettingsList className='flex-1 overflow-y-auto scrollbar-thin' />

                    <SettingsButtons onCancel={onCancel} onClose={onClose} className='p-3 justify-end' />
                </div>
            </div>
        </Dialog>
    );
};

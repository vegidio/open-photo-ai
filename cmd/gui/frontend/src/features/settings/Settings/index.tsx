import { useEffect, useRef } from 'react';
import { Dialog, Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { SetExecutionProvider } from '@/bindings/gui/services/appservice.ts';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { SettingsButtons } from '@/features/settings/SettingsButtons';
import { SettingsList, type SettingsListHandle } from '@/features/settings/SettingsList';
import { SettingsMenu } from '@/features/settings/SettingsMenu';
import i18n from '@/i18n';
import { useSettingsStore } from '@/stores';

type SettingsProps = {
    section: 'application' | 'models';
    open: boolean;
    onClose: () => void;
};

export const Settings = ({ section: _section, open, onClose }: SettingsProps) => {
    const { t } = useTranslation();
    const saveSnapshot = useSettingsStore((state) => state.saveSnapshot);
    const restoreSnapshot = useSettingsStore((state) => state.restoreSnapshot);
    const listRef = useRef<SettingsListHandle>(null);

    // The processor the dialog opened with, so Save can tell whether it actually changed.
    const initialEp = useRef(useSettingsStore.getState().executionProvider);

    const onCancel = () => {
        restoreSnapshot();
        onClose();
    };

    const onSave = () => {
        onClose();

        const { executionProvider, language } = useSettingsStore.getState();

        // Applying the language here rather than in the Select's onChange is what keeps Cancel a true no-op: until
        // Save runs, nothing outside the store has observed the new value, so restoreSnapshot() undoes it completely.
        // i18n.language is itself the authoritative "currently applied" value, so no extra ref is needed to compare.
        if (language !== i18n.language) void i18n.changeLanguage(language);

        // Only the processor needs telling: the other settings (the model of each enhancement) already produce a
        // different operation ID, so they miss the model cache on their own.
        if (executionProvider === initialEp.current) return;

        // Nothing is unloaded here. The backend keys loaded models by operation *and* processor, so the next
        // enhancement simply builds on the new one and whatever the old processor held ages out by itself. That means
        // this is safe mid-export: the running job keeps its models, and the next one picks up the change.
        void SetExecutionProvider();
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
            <ModalTitle title={t('settings.title')} onClose={onCancel} />

            <div className='flex flex-row flex-1 overflow-hidden'>
                <SettingsMenu
                    className='w-52 px-2 pt-2'
                    onItemClick={(itemId) => listRef.current?.scrollToSection(itemId)}
                />

                <Divider orientation='vertical' flexItem className='border-[#171717] my-0.5' />

                <div className='flex flex-col flex-1'>
                    <SettingsList ref={listRef} className='flex-1 overflow-y-auto scrollbar-thin' />

                    <SettingsButtons onCancel={onCancel} onSave={onSave} className='p-3 justify-end' />
                </div>
            </div>
        </Dialog>
    );
};

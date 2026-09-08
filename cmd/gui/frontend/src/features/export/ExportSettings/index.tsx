import { useMemo, useState } from 'react';
import { Divider, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import type { QualityChoices } from '@/utils/quality.ts';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { ExportSettingsButtons } from '@/features/export/ExportSettingsButtons';
import { ExportSettingsFilename } from '@/features/export/ExportSettingsFilename';
import { ExportSettingsFormat } from '@/features/export/ExportSettingsFormat';
import { ExportSettingsLocation } from '@/features/export/ExportSettingsLocation';
import { ExportSettingsQuality } from '@/features/export/ExportSettingsQuality';
import { useExportStore, useSettingsStore } from '@/stores';
import { resolveQualityFormat } from '@/utils/export.ts';

export type ExportSettingsProps = TailwindProps & {
    enhancements: Map<File, Operation[]>;
    onClose: () => void;
};

export const ExportSettings = ({ enhancements, onClose, className }: ExportSettingsProps) => {
    const { t } = useTranslation();
    const format = useExportStore((state) => state.format);

    // A draft of the persisted qualities, not the store itself: the setting is only committed once the user actually
    // exports (see ExportSettingsButtons), and closing the dialog has to discard whatever they were fiddling with.
    // The dialog is remounted on every open, so unmounting is all the reset this needs.
    //
    // Seeded with getState rather than a selector on purpose - subscribing would let the Settings dialog yank the
    // draft out from under a half-made edit. The whole record is held so switching format back and forth keeps each
    // format's in-session value.
    const [quality, setQuality] = useState<QualityChoices>(() => ({ ...useSettingsStore.getState().quality }));

    // Which format the slider would speak for, if any. Under "preserve" a mixed queue has no single answer, and the
    // slider is hidden rather than misrepresenting one - each file still gets its own stored quality on export.
    const qualityFormat = useMemo(() => resolveQualityFormat(enhancements.keys(), format), [enhancements, format]);

    return (
        <div className={`${className} p-3 flex flex-col gap-4`}>
            <Typography variant='subtitle2'>{t('export.settings.title')}</Typography>

            {/* Scrolls rather than pushing the buttons out of the dialog: the parent is a fixed-height, overflow-hidden
                row, so the Quality block appearing would otherwise clip Cancel and Save off the bottom edge. flex-1
                keeps the buttons pinned to the bottom when the settings are short, as a plain spacer used to; min-h-0
                is what lets this actually shrink, since a flex item defaults to min-height:auto. */}
            <div className='flex-1 min-h-0 flex flex-col gap-4 overflow-hidden'>
                <ExportSettingsFilename />

                <Divider />

                <ExportSettingsLocation />

                <Divider />

                <ExportSettingsFormat />

                {qualityFormat !== undefined && (
                    <ExportSettingsQuality
                        format={qualityFormat}
                        value={quality[qualityFormat]}
                        onChange={(value) => setQuality((current) => ({ ...current, [qualityFormat]: value }))}
                    />
                )}
            </div>

            <ExportSettingsButtons enhancements={enhancements} quality={quality} onClose={onClose} />
        </div>
    );
};

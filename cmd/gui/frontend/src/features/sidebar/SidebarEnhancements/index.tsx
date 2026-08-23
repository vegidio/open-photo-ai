import { useEffect, useState } from 'react';
import { List } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { File } from '@/bindings/gui/types';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { AnalyticsEvent, track } from '@/analytics';
import { ListItemEnhancement } from '@/features/enhancements/ListItemEnhancement';
import { ListItemAutopilot } from '@/features/sidebar/ListItemAutopilot';
import { useAddEnhancements, useCurrentFile, useFileOperations, useNotify } from '@/hooks';
import { useEnhancementStore, useSettingsStore } from '@/stores';
import { getEnhancementType, suggestEnhancement } from '@/utils/enhancement.ts';
import { getErrorMessage, userFriendlyErrorKey } from '@/utils/errors.ts';

export const SidebarEnhancements = ({ className = '' }: TailwindProps) => {
    const { t } = useTranslation();
    const { enqueueSnackbar } = useNotify();

    const file = useCurrentFile();
    const autopilot = useEnhancementStore((state) => state.autopilot);
    const hasEnhancement = useEnhancementStore((state) => (file ? state.enhancements.has(file.Path) : false));
    const operations = useFileOperations(file);
    const addEnhancements = useAddEnhancements();
    const models = useSettingsStore((state) => state.models);

    const [isAnalysing, setIsAnalysing] = useState(false);

    // addEnhancements/enqueueSnackbar are called imperatively and are unstable references; keying the effect on them
    // would re-run autopilot mid-flight (before hasEnhancement flips true) and add the suggestions twice. The model
    // selections are read at run time only. Autopilot must run exactly once per file.
    // biome-ignore lint/correctness/useExhaustiveDependencies: see above
    useEffect(() => {
        // Autopilot should run if all conditions are met:
        //   1. There's a file selected
        //   2. Autopilot is enabled
        //   3. The file never had any enhancements applied to it; if any enhancements were applied before, even if
        //      they were removed later, autopilot will _not_ run again, unless the file is removed and re-added.
        async function runAutopilot(currentFile: File) {
            setIsAnalysing(true);

            try {
                const suggestions = await suggestEnhancement(currentFile, models);

                await addEnhancements(currentFile, suggestions, 'autopilot');
                track(AnalyticsEvent.AutopilotRun, { count: suggestions.length });
            } catch (e) {
                console.error('Autopilot failed', e);
                track(AnalyticsEvent.AutopilotFailed, { reason: getErrorMessage(e) });
                enqueueSnackbar(t(userFriendlyErrorKey(e, 'errors.autopilotFailed')), { variant: 'error' });
            } finally {
                setIsAnalysing(false);
            }
        }

        if (file && autopilot && !hasEnhancement) runAutopilot(file);
    }, [autopilot, hasEnhancement, file]);

    return (
        <List className={`${className}`} dense>
            {isAnalysing ? (
                <ListItemAutopilot />
            ) : (
                operations.map((op) => {
                    const prefix = getEnhancementType(op.id);
                    return <ListItemEnhancement key={prefix} op={op} />;
                })
            )}
        </List>
    );
};

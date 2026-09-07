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
import { suggestEnhancement } from '@/utils/enhancement.ts';
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

    // The spinner belongs to a FILE, not to the component. Analysis is asynchronous and the selection can change
    // while it runs, so a bare boolean had no way to say WHICH file it was about: a run finishing for the previous
    // file switched off the spinner a newer run had just switched on. Storing the path being analysed makes every
    // ordering self-correcting - switching away hides it at once, switching back shows it again, and each run only
    // ever clears its own file.
    const [analysingPath, setAnalysingPath] = useState<string | null>(null);
    const isAnalysing = file != null && analysingPath === file.Path;

    // addEnhancements and enqueueSnackbar are stable references now, but they stay out of the list on purpose: both
    // are called imperatively, and keying the effect on them would re-run autopilot mid-flight (before hasEnhancement
    // flips true) and add every suggestion twice. The model selections are read at run time only, so listing them
    // would restart the analysis whenever a setting changed. Autopilot must run exactly once per file.
    // biome-ignore lint/correctness/useExhaustiveDependencies: see above
    useEffect(() => {
        // Autopilot should run if all conditions are met:
        //   1. There's a file selected
        //   2. Autopilot is enabled
        //   3. The file never had any enhancements applied to it; if any enhancements were applied before, even if
        //      they were removed later, autopilot will _not_ run again, unless the file is removed and re-added.
        async function runAutopilot(currentFile: File) {
            setAnalysingPath(currentFile.Path);

            try {
                const suggestions = await suggestEnhancement(currentFile, models);

                // Applied unconditionally, even if the user has moved on: the suggestions belong to currentFile, and
                // dropping them would leave a file the user already saw analysed with nothing to show for it.
                await addEnhancements(currentFile, suggestions, 'autopilot');
                track(AnalyticsEvent.AutopilotRun, { count: suggestions.length });
            } catch (e) {
                console.error('Autopilot failed', e);
                track(AnalyticsEvent.AutopilotFailed, { reason: getErrorMessage(e) });
                enqueueSnackbar(t(userFriendlyErrorKey(e, 'errors.autopilotFailed')), { variant: 'error' });
            } finally {
                // Clears this file's spinner and nothing else. Note that addEnhancements above flips hasEnhancement,
                // which re-runs this very effect - so anything that tied the reset to the effect's own lifetime (a
                // cleanup flag, say) would cancel the run that is still in flight and leave the spinner up forever.
                setAnalysingPath((current) => (current === currentFile.Path ? null : current));
            }
        }

        if (file && autopilot && !hasEnhancement) void runAutopilot(file);
    }, [autopilot, hasEnhancement, file]);

    return (
        <List className={`${className}`} dense>
            {isAnalysing ? (
                <ListItemAutopilot />
            ) : (
                // Keyed on the operation id, not on its enhancement type: getEnhancementType returns undefined for an
                // id that no longer maps to anything (a stale entry rehydrated from a previous version's persisted
                // state), and two of those would collide on key={undefined} and mis-reconcile. The id is unique per
                // row and stable across renders.
                operations.map((op) => <ListItemEnhancement key={op.id} op={op} />)
            )}
        </List>
    );
};

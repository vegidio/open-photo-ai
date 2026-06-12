import { useEffect, useState } from 'react';
import { List } from '@mui/material';
import type { File } from '@/bindings/gui/types';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { ListItemEnhancement } from '@/features/enhancements/ListItemEnhancement';
import { ListItemAutopilot } from '@/features/sidebar/ListItemAutopilot';
import { useAddEnhancements, useCurrentFile, useFileOperations, useNotify } from '@/hooks';
import { useEnhancementStore, useSettingsStore } from '@/stores';
import { suggestEnhancement } from '@/utils/enhancement.ts';
import { userFriendlyErrorMessage } from '@/utils/errors.ts';

export const SidebarEnhancements = ({ className = '' }: TailwindProps) => {
    const { enqueueSnackbar } = useNotify();

    const file = useCurrentFile();
    const autopilot = useEnhancementStore((state) => state.autopilot);
    const hasEnhancement = useEnhancementStore((state) => (file ? state.enhancements.has(file) : false));
    const operations = useFileOperations(file);
    const addEnhancements = useAddEnhancements();
    const dnModel = useSettingsStore((state) => state.dnModel);
    const frModel = useSettingsStore((state) => state.frModel);
    const laModel = useSettingsStore((state) => state.laModel);
    const cbModel = useSettingsStore((state) => state.cbModel);
    const upModel = useSettingsStore((state) => state.upModel);
    const shModel = useSettingsStore((state) => state.shModel);

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
                const suggestions = await suggestEnhancement(currentFile, {
                    dn: dnModel,
                    fr: frModel,
                    la: laModel,
                    cb: cbModel,
                    sh: shModel,
                    up: upModel,
                });

                await addEnhancements(currentFile, suggestions);
            } catch (e) {
                const msg = userFriendlyErrorMessage(e, 'Something went wrong. Failed to run autopilot.');
                enqueueSnackbar(msg, { variant: 'error' });
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
                    const prefix = op.id.slice(0, 2);
                    return <ListItemEnhancement key={prefix} op={op} />;
                })
            )}
        </List>
    );
};

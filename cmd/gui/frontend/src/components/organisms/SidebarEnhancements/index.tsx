import { useEffect, useState } from 'react';
import { List } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { ListItemAutopilot } from '@/components/molecules/ListItemAutopilot';
import { ListItemEnhancement } from '@/components/organisms/ListItemEnhancement';
import { useNotify } from '@/hooks/useNotify.ts';
import { useEnhancementStore, useFileStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';
import { suggestEnhancement } from '@/utils/enhancement.ts';

export const SidebarEnhancements = ({ className = '' }: TailwindProps) => {
    const { enqueueSnackbar } = useNotify();

    const file = useFileStore((state) => state.files[state.currentIndex]);
    const autopilot = useEnhancementStore((state) => state.autopilot);
    const hasEnhancement = useEnhancementStore((state) => state.enhancements.has(file));
    const operations = useEnhancementStore((state) => state.enhancements.get(file) ?? EMPTY_OPERATIONS);
    const addEnhancements = useEnhancementStore((state) => state.addEnhancements);

    const [isAnalysing, setIsAnalysing] = useState(false);

    // biome-ignore lint/correctness/useExhaustiveDependencies: enqueueSnackbar
    useEffect(() => {
        // Autopilot should run if all conditions are met:
        //   1. There's a file selected
        //   2. Autopilot is enabled
        //   3. The file never had any enhancements applied to it; if any enhancements were applied before, even if
        //      they were removed later, autopilot will _not_ run again, unless the file is removed and re-added.
        const shouldRunAutopilot = file && autopilot && !hasEnhancement;

        async function runAutopilot() {
            setIsAnalysing(true);

            try {
                const suggestions = await suggestEnhancement(file);
                addEnhancements(file, suggestions);
            } catch (e) {
                const msg = userFriendlyErrorMessage(e);
                enqueueSnackbar(msg, { variant: 'error' });
            } finally {
                setIsAnalysing(false);
            }
        }

        if (shouldRunAutopilot) runAutopilot();
    }, [autopilot, hasEnhancement, addEnhancements, file]);

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

const userFriendlyErrorMessage = (error: unknown) => {
    const msg = error instanceof Error ? error.message : String(error);

    switch (true) {
        case msg.includes('[download]'):
            return 'Failed to download AI model. Check your internet connection and try again.';
        default:
            return 'Something wrong wrong. Failed to run autopilot.';
    }
};

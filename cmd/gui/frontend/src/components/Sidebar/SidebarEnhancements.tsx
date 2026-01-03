import { useEffect, useState } from 'react';
import { List } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { AutopilotAnalysis } from '@/components/Enhancement/AutopilotAnalysis.tsx';
import { EnhancementListItem } from '@/components/molecules/EnhancementListItem';
import { useEnhancementStore, useFileStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';
import { suggestEnhancement } from '@/utils/enhancement.ts';

export const SidebarEnhancements = ({ className = '' }: TailwindProps) => {
    const file = useFileStore((state) => state.files[state.currentIndex]);
    const autopilot = useEnhancementStore((state) => state.autopilot);
    const hasEnhancement = useEnhancementStore((state) => state.enhancements.has(file));
    const operations = useEnhancementStore((state) => state.enhancements.get(file) ?? EMPTY_OPERATIONS);
    const addEnhancements = useEnhancementStore((state) => state.addEnhancements);

    const [isAnalysing, setIsAnalysing] = useState(false);

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
                const suggestions = await suggestEnhancement(file.Path);
                addEnhancements(file, suggestions);
            } catch (e) {
                console.error('Failed to run autopilot', e);
            } finally {
                setIsAnalysing(false);
            }
        }

        if (shouldRunAutopilot) runAutopilot();
    }, [autopilot, hasEnhancement, addEnhancements, file]);

    return (
        <List className={`${className}`} dense>
            {isAnalysing ? <AutopilotAnalysis /> : operations.map((op) => <EnhancementListItem key={op.id} op={op} />)}
        </List>
    );
};

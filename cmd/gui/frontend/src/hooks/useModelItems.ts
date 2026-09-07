import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SelectItem } from '@/components/atoms/Select';
import { modelLabel } from '@/i18n/format';
import { type EnhancementType, modelTiers } from '@/utils/enhancement';

/**
 * The model choices the Settings screen's default-model dropdown offers, built from the enhancement registry.
 *
 * Every model appears at both quality tiers — "Stockholm HD", "Stockholm SD" — because the tier is part of the
 * default: it decides the precision an enhancement is built at when it is added from the menu or by autopilot.
 *
 * The tiers of one model are drawn as a group, with a rule under the last of them. The rule is set here rather than
 * in the Select because only the catalog knows where one model ends and the next begins.
 *
 * Built inside the hook rather than at module scope because `t()` called at module-evaluation time would freeze the
 * labels in whatever language was active on the first import.
 */
export const useModelItems = (type: EnhancementType): SelectItem[] => {
    const { t } = useTranslation();

    return useMemo(() => {
        const tiers = modelTiers(type);

        return tiers.map(({ value, label, tier, model }, index) => {
            const next = tiers[index + 1];

            return {
                value,
                label: modelLabel(t, label, tier),
                // No rule under the last item: it would read as a break before nothing.
                divider: next !== undefined && next.model !== model,
            };
        });
    }, [t, type]);
};

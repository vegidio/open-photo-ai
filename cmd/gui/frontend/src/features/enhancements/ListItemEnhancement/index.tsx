import { type ComponentType, type MouseEvent, useState } from 'react';
import { IconButton, ListItem, ListItemButton, ListItemIcon, ListItemText } from '@mui/material';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { Operation } from '@/operations';
import { AnalyticsEvent, track } from '@/analytics';
import { Icon } from '@/components/atoms/Icon';
import { OptionsColorBalance } from '@/features/enhancements/OptionsColorBalance';
import { OptionsColorization } from '@/features/enhancements/OptionsColorization';
import { OptionsDenoise } from '@/features/enhancements/OptionsDenoise';
import { OptionsFaceRecovery } from '@/features/enhancements/OptionsFaceRecovery';
import { OptionsLightAdjustment } from '@/features/enhancements/OptionsLightAdjustment';
import { OptionsSharpen } from '@/features/enhancements/OptionsSharpen';
import { OptionsUpscale } from '@/features/enhancements/OptionsUpscale';
import { useCurrentFile, useFileDisabledFaces, useFileFaces } from '@/hooks';
import { qualityLabel } from '@/i18n/format';
import { useEnhancementStore } from '@/stores';
import { ENHANCEMENTS, type EnhancementType, getEnhancementType } from '@/utils/enhancement.ts';

type ListItemEnhancementProps = {
    op: Operation;
};

type OptionsProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

// The popover each enhancement opens. Kept here rather than in the ENHANCEMENTS registry so that `utils/enhancement`
// stays a plain data module the stores and hooks can import without pulling in the feature components.
const OPTIONS: Record<EnhancementType, ComponentType<OptionsProps>> = {
    dn: OptionsDenoise,
    fr: OptionsFaceRecovery,
    cl: OptionsColorization,
    la: OptionsLightAdjustment,
    cb: OptionsColorBalance,
    sh: OptionsSharpen,
    up: OptionsUpscale,
};

export const ListItemEnhancement = ({ op }: ListItemEnhancementProps) => {
    const { t } = useTranslation();
    const [isHovered, setIsHovered] = useState(false);
    const file = useCurrentFile();
    const faces = useFileFaces(file);
    const disabledFaces = useFileDisabledFaces(file);
    const removeEnhancement = useEnhancementStore((state) => state.removeEnhancement);

    const type = getEnhancementType(op.id);
    const enhancement = ENHANCEMENTS[type];
    const OptionsComponent = OPTIONS[type];

    const name = enhancement ? t(enhancement.nameKey) : '';
    const info = enhancement ? opInfo(t, op, type, facesLabel(t, faces.length, disabledFaces.size)) : '';

    const [anchorEl, setAnchorEl] = useState<HTMLElement | undefined>(undefined);
    const open = Boolean(anchorEl);

    const onMenuOpen = (event: MouseEvent<HTMLDivElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const onMenuClose = () => {
        setAnchorEl(undefined);
    };

    const onRemove = () => {
        if (file) {
            removeEnhancement(file, op.id);
            track(AnalyticsEvent.EnhancementRemoved, { type });
        }
    };

    return (
        <>
            <ListItem
                divider={true}
                disablePadding
                onMouseEnter={() => setIsHovered(true)}
                onMouseLeave={() => setIsHovered(false)}
                secondaryAction={
                    isHovered ? (
                        <IconButton disableRipple edge='end' onClick={onRemove}>
                            <Icon option='close' />
                        </IconButton>
                    ) : undefined
                }
            >
                <ListItemButton className='min-h-12' onClick={onMenuOpen}>
                    <ListItemIcon className='min-w-9 [&>svg]:size-5'>
                        {enhancement && <Icon option={enhancement.icon} />}
                    </ListItemIcon>
                    <ListItemText
                        primary={name}
                        secondary={info}
                        className='my-0'
                        slotProps={{
                            primary: {
                                className: 'text-[13px] text-white',
                            },
                            secondary: {
                                className: 'text-[13px] text-[#545454] italic',
                            },
                        }}
                    />
                </ListItemButton>
            </ListItem>

            {open && OptionsComponent && <OptionsComponent anchorEl={anchorEl} open={true} onClose={onMenuClose} />}
        </>
    );
};

// The secondary line under the enhancement's name. Face recovery and upscale each carry a figure of their own; every
// other enhancement is described by its intensity, so they share one key rather than repeating the same call six times.
const opInfo = (t: TFunction, op: Operation, type: EnhancementType, faceText: string): string => {
    const name = titleCase(op.options.name);
    const quality = qualityLabel(t, op.options.precision);

    switch (type) {
        case 'fr':
            return t('enhancements.infoFaces', { name, faces: faceText, quality });

        // Colorization has no intensity, so the default branch's "{{intensity}}%" would render "NaN%".
        case 'cl':
            return t('enhancements.infoModel', { name, quality });

        case 'up': {
            const scale = parseFloat(parseFloat(op.options.scale).toFixed(3));
            return t('enhancements.infoScale', { name, scale, quality });
        }

        default: {
            const intensity = parseFloat(op.options.intensity) * 100;
            return t('enhancements.info', { name, intensity, quality });
        }
    }
};

const titleCase = (input: string): string => {
    if (!input) return input;
    return input[0].toUpperCase() + input.slice(1);
};

// `count` drives i18next's CLDR plural selection and `context` picks the partial-selection wording; the two
// suffixes compose as key_context_plural, e.g. faces_partial_one. Pluralising on the total rather than the enabled
// count keeps the existing English behaviour, and a language with more plural forms only adds keys to its own
// catalog -- no code change here.
const facesLabel = (t: TFunction, total: number, disabledCount: number): string => {
    const enabled = Math.max(0, total - disabledCount);

    return t('enhancements.faces', {
        count: total,
        enabled,
        context: enabled === total ? undefined : 'partial',
    });
};

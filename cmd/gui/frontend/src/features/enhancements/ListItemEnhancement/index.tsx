import { type MouseEvent, type ReactNode, useState } from 'react';
import { IconButton, ListItem, ListItemButton, ListItemIcon, ListItemText } from '@mui/material';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { Operation } from '@/operations';
import { AnalyticsEvent, track } from '@/analytics';
import { Icon } from '@/components/atoms/Icon';
import { OptionsColorBalance } from '@/features/enhancements/OptionsColorBalance';
import { OptionsDenoise } from '@/features/enhancements/OptionsDenoise';
import { OptionsFaceRecovery } from '@/features/enhancements/OptionsFaceRecovery';
import { OptionsLightAdjustment } from '@/features/enhancements/OptionsLightAdjustment';
import { OptionsSharpen } from '@/features/enhancements/OptionsSharpen';
import { OptionsUpscale } from '@/features/enhancements/OptionsUpscale';
import { useCurrentFile, useFileDisabledFaces, useFileFaces } from '@/hooks';
import { useEnhancementStore } from '@/stores';
import { getEnhancementType } from '@/utils/enhancement.ts';

type ListItemEnhancementProps = {
    op: Operation;
};

export const ListItemEnhancement = ({ op }: ListItemEnhancementProps) => {
    const { t } = useTranslation();
    const [isHovered, setIsHovered] = useState(false);
    const file = useCurrentFile();
    const faces = useFileFaces(file);
    const disabledFaces = useFileDisabledFaces(file);
    const removeEnhancement = useEnhancementStore((state) => state.removeEnhancement);

    const { name, info, icon } = opToEnhancement(t, op, facesLabel(t, faces.length, disabledFaces.size));
    const OptionsComponent = selectOptionsComponent(op.id);

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
            track(AnalyticsEvent.EnhancementRemoved, { type: getEnhancementType(op.id) });
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
                    <ListItemIcon className='min-w-9 [&>svg]:size-5'>{icon}</ListItemIcon>
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

const selectOptionsComponent = (operationId: string) => {
    switch (true) {
        case operationId.startsWith('dn'):
            return OptionsDenoise;

        case operationId.startsWith('fr'):
            return OptionsFaceRecovery;

        case operationId.startsWith('la'):
            return OptionsLightAdjustment;

        case operationId.startsWith('cb'):
            return OptionsColorBalance;

        case operationId.startsWith('sh'):
            return OptionsSharpen;

        case operationId.startsWith('up'):
            return OptionsUpscale;
    }
};

const opToEnhancement = (
    t: TFunction,
    op: Operation,
    faceText: string,
): { name: string; info: string; icon: ReactNode } => {
    const quality = t(op.options.precision === 'fp32' ? 'models.quality.hd' : 'models.quality.md');

    switch (true) {
        // Denoise
        case op.id.startsWith('dn'): {
            const intensity = parseFloat(op.options.intensity) * 100;
            const info = t('enhancements.info', { name: titleCase(op.options.name), intensity, quality });
            return { name: t('enhancements.denoise.name'), info, icon: <Icon option='denoise' /> };
        }

        // Face Recovery
        case op.id.startsWith('fr'): {
            const info = t('enhancements.infoFaces', { name: titleCase(op.options.name), faces: faceText, quality });
            return { name: t('enhancements.faceRecovery.name'), info, icon: <Icon option='face_recovery' /> };
        }

        // Light Adjustment
        case op.id.startsWith('la'): {
            const intensity = parseFloat(op.options.intensity) * 100;
            const info = t('enhancements.info', { name: titleCase(op.options.name), intensity, quality });
            return { name: t('enhancements.lightAdjustment.name'), info, icon: <Icon option='light_adjustment' /> };
        }

        // Color Balance
        case op.id.startsWith('cb'): {
            const intensity = parseFloat(op.options.intensity) * 100;
            const info = t('enhancements.info', { name: titleCase(op.options.name), intensity, quality });
            return { name: t('enhancements.colorBalance.name'), info, icon: <Icon option='color_balance' /> };
        }

        // Upscale
        case op.id.startsWith('up'): {
            const scale = parseFloat(parseFloat(op.options.scale).toFixed(3));
            const info = t('enhancements.infoScale', { name: titleCase(op.options.name), scale, quality });
            return { name: t('enhancements.upscale.name'), info, icon: <Icon option='upscale' /> };
        }

        // Sharpen
        case op.id.startsWith('sh'): {
            const intensity = parseFloat(op.options.intensity) * 100;
            const info = t('enhancements.info', { name: titleCase(op.options.name), intensity, quality });
            return { name: t('enhancements.sharpen.name'), info, icon: <Icon option='sharpen' /> };
        }
    }

    return { name: '', info: '', icon: undefined };
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

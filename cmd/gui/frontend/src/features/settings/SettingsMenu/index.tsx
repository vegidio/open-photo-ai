import { SimpleTreeView, TreeItem } from '@mui/x-tree-view';
import { useTranslation } from 'react-i18next';
import type { TailwindProps } from '@/utils/TailwindProps';

type SettingsMenuProps = TailwindProps & {
    onItemClick?: (itemId: string) => void;
};

export const SettingsMenu = ({ className = '', onItemClick }: SettingsMenuProps) => {
    const { t } = useTranslation();

    return (
        <SimpleTreeView
            className={`${className}`}
            expandedItems={['app', 'performance', 'enhancements']}
            onItemClick={(_, itemId) => onItemClick?.(itemId)}
            sx={{
                '& .MuiTreeItem-label': {
                    fontSize: '0.875rem', // text-sm equivalent
                    color: '#b0b0b0',
                },
            }}
        >
            {/* Every label here reuses the key its SettingsList counterpart renders, so the two can't drift. The
                itemIds still have to match by hand — a mismatch makes scrollToSection silently do nothing. */}
            <TreeItem
                itemId='app'
                label={t('settings.sections.app')}
                slotProps={{
                    label: {
                        className: 'font-bold text-[#f2f2f2]',
                    },
                }}
            >
                <TreeItem itemId='app_language' label={t('settings.app.language.title')} />
                <TreeItem itemId='app_logs' label={t('settings.app.logs.title')} />
                <TreeItem itemId='app_analytics' label={t('settings.app.analytics.title')} />
            </TreeItem>

            <TreeItem
                itemId='performance'
                label={t('settings.sections.performance')}
                slotProps={{
                    label: {
                        className: 'font-bold text-[#f2f2f2]',
                    },
                }}
            >
                <TreeItem itemId='perf_processor' label={t('settings.performance.aiProcessor.title')} />
            </TreeItem>

            <TreeItem
                itemId='enhancements'
                label={t('settings.sections.enhancements')}
                slotProps={{
                    label: {
                        className: 'font-bold text-[#f2f2f2]',
                    },
                }}
            >
                <TreeItem itemId='enh_denoise' label={t('settings.enhancements.denoise.title')} />
                <TreeItem itemId='enh_face' label={t('settings.enhancements.faceRecovery.title')} />
                <TreeItem itemId='enh_light' label={t('settings.enhancements.lightAdjustment.title')} />
                <TreeItem itemId='enh_color' label={t('settings.enhancements.colorBalance.title')} />
                <TreeItem itemId='enh_sharpen' label={t('settings.enhancements.sharpen.title')} />
                <TreeItem itemId='enh_upscale' label={t('settings.enhancements.upscale.title')} />
            </TreeItem>
        </SimpleTreeView>
    );
};

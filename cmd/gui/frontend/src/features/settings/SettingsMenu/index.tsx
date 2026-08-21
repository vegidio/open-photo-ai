import { SimpleTreeView, TreeItem } from '@mui/x-tree-view';
import { useTranslation } from 'react-i18next';
import type { TailwindProps } from '@/utils/TailwindProps';
import { SETTINGS_SECTIONS } from '@/features/settings/sections';

type SettingsMenuProps = TailwindProps & {
    onItemClick?: (itemId: string) => void;
};

// Every section starts expanded, so this follows the structure rather than repeating its ids.
const EXPANDED_ITEMS = SETTINGS_SECTIONS.map((section) => section.id);

export const SettingsMenu = ({ className = '', onItemClick }: SettingsMenuProps) => {
    const { t } = useTranslation();

    return (
        <SimpleTreeView
            className={`${className}`}
            expandedItems={EXPANDED_ITEMS}
            onItemClick={(_, itemId) => onItemClick?.(itemId)}
            sx={{
                '& .MuiTreeItem-label': {
                    fontSize: '0.875rem',
                    color: '#b0b0b0',
                },
            }}
        >
            {SETTINGS_SECTIONS.map((section) => (
                <TreeItem
                    key={section.id}
                    itemId={section.id}
                    label={t(section.labelKey)}
                    slotProps={{
                        label: {
                            className: 'font-bold text-[#f2f2f2]',
                        },
                    }}
                >
                    {section.items.map((item) => (
                        <TreeItem key={item.id} itemId={item.id} label={t(item.labelKey)} />
                    ))}
                </TreeItem>
            ))}
        </SimpleTreeView>
    );
};

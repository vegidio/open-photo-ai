import { type MouseEvent, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FiPlus } from 'react-icons/fi';
import { Button } from '@/components/atoms/Button';
import { MenuAddEnhancement } from '@/features/enhancements/MenuAddEnhancement';

type AddEnhancementProps = {
    disabled?: boolean;
};

export const AddEnhancement = ({ disabled = false }: AddEnhancementProps) => {
    const { t } = useTranslation();
    const [anchorEl, setAnchorEl] = useState<HTMLElement | undefined>(undefined);
    const open = Boolean(anchorEl);

    const onMenuOpen = (event: MouseEvent<HTMLButtonElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const onMenuClose = () => {
        setAnchorEl(undefined);
    };

    return (
        <>
            <Button
                option='tertiary'
                disabled={disabled}
                startIcon={<FiPlus className='size-6 stroke-1' />}
                onClick={onMenuOpen}
            >
                {t('enhancements.add')}
            </Button>

            {open && <MenuAddEnhancement anchorEl={anchorEl} open={true} onMenuClose={onMenuClose} />}
        </>
    );
};

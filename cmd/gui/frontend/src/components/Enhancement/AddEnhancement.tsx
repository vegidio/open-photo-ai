import { type MouseEvent, useState } from 'react';
import { FiPlus } from 'react-icons/fi';
import { Button } from '@/components/atoms/Button';
import { EnhancementAddMenu } from '@/components/molecules/EnhancementAddMenu';

type AddEnhancementProps = {
    disabled?: boolean;
};

export const AddEnhancement = ({ disabled = false }: AddEnhancementProps) => {
    const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
    const open = Boolean(anchorEl);

    const onMenuOpen = (event: MouseEvent<HTMLButtonElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const onMenuClose = () => {
        setAnchorEl(null);
    };

    return (
        <>
            <Button
                option='secondary'
                disabled={disabled}
                startIcon={<FiPlus className='size-6 stroke-1' />}
                onClick={onMenuOpen}
            >
                Add enhancement
            </Button>

            <EnhancementAddMenu anchorEl={anchorEl} open={open} onMenuClose={onMenuClose} />
        </>
    );
};

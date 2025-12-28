import { Button } from '@mui/material';
import { Browser } from '@wailsio/runtime';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

export const NavbarUpdate = ({ className }: TailwindProps) => {
    const onUpdateClick = () => {
        Browser.OpenURL('https://github.com/vegidio/open-photo-ai/releases');
    };

    return (
        <Button
            variant='contained'
            size='small'
            onClick={onUpdateClick}
            className={`${className} bg-[#009aff] hover:bg-[#007eff] text-[#f2f2f2] normal-case font-normal animate-pulse`}
        >
            Update Available
        </Button>
    );
};

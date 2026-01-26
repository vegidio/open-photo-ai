import { type MouseEvent, useState } from 'react';
import { Divider, IconButton, Typography } from '@mui/material';
import { basename } from 'pathe';
import { IoIosMore } from 'react-icons/io';
import type { File } from '@/bindings/gui/types';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { MenuFileOptions } from '@/components/molecules/MenuFileOptions';

type NavbarCurrentFileProps = TailwindProps & {
    file: File;
};

export const NavbarCurrentFile = ({ file, className = '' }: NavbarCurrentFileProps) => {
    const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
    const open = Boolean(anchorEl);

    const onMenuOpen = (event: MouseEvent<HTMLButtonElement>) => {
        // Prevent click from bubbling to the parent button
        event.stopPropagation();
        setAnchorEl(event.currentTarget);
    };

    const onMenuClose = () => {
        setAnchorEl(null);
    };

    return (
        <>
            <div className={`${className} flex flex-row h-full items-center`}>
                <Divider orientation='vertical' variant='middle' flexItem />

                <Typography variant='caption' className='ml-4 mr-2 text-[#b0b0b0]'>
                    {basename(file.Path)}
                </Typography>

                <IconButton type='button' onClick={onMenuOpen}>
                    <IoIosMore className='size-4 text-[#b0b0b0]' />
                </IconButton>
            </div>

            <MenuFileOptions
                file={file}
                anchorEl={anchorEl}
                anchorOrigin={{
                    vertical: 'bottom',
                    horizontal: 'center',
                }}
                transformOrigin={{
                    vertical: 'top',
                    horizontal: 'center',
                }}
                open={open}
                onMenuClose={onMenuClose}
            />
        </>
    );
};

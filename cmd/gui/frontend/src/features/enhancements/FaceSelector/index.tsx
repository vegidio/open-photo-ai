import { Typography } from '@mui/material';
import { Trans, useTranslation } from 'react-i18next';
import { Button } from '@/components/atoms/Button';

type FaceSelectorProps = {
    selectedCount: number;
    onClick: () => void;
};

export const FaceSelector = ({ selectedCount, onClick }: FaceSelectorProps) => {
    const { t } = useTranslation();

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2'>{t('enhancements.faceSelector.title')}</Typography>

            {/* The line break is part of the centred two-line layout, so it sits in the catalog where a translator
                can move or drop it. <br/> is one of i18next's default kept nodes, so no components prop is needed. */}
            <Typography align='center' className='text-[13px] text-[#b0b0b0]'>
                <Trans i18nKey='enhancements.faceSelector.help' />
            </Typography>

            <Button option='tertiary' onClick={onClick}>
                {t('enhancements.faceSelector.button', { count: selectedCount })}
            </Button>
        </div>
    );
};

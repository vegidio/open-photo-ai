import { useMemo } from 'react';
import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ValueSlider } from '@/components/atoms/ValueSlider';
import { DEFAULT_QUALITY, MAX_QUALITY, MIN_QUALITY, type QualityFormat } from '@/utils/quality';

type ExportSettingsQualityProps = {
    format: QualityFormat;
    value: number;
    onChange: (value: number) => void;
};

export const ExportSettingsQuality = ({ format, value, onChange }: ExportSettingsQualityProps) => {
    const { t } = useTranslation();

    // The mark shows where this format's default sits, so a user who has moved the slider can find their way back to
    // it. It moves with the format because the encoders' scales are not comparable - see DEFAULT_QUALITY.
    const marks = useMemo(() => [{ value: DEFAULT_QUALITY[format], label: String(DEFAULT_QUALITY[format]) }], [format]);

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2' className='text-[#b0b0b0]'>
                {t('export.settings.quality.title')}
            </Typography>

            <ValueSlider
                min={MIN_QUALITY}
                max={MAX_QUALITY}
                step={1}
                marks={marks}
                value={value}
                onChange={onChange}
                className='mx-3'
            />
        </div>
    );
};

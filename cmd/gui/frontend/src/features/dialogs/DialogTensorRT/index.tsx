import { Typography } from '@mui/material';
import { Trans, useTranslation } from 'react-i18next';
import logo_tensorrt from '@/assets/logo_tensorrt.avif';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { Button } from '@/components/atoms/Button';
import { DialogGeneral } from '@/components/molecules/DialogGeneral';
import { useSettingsStore } from '@/stores';

type DialogTensorRTProps = {
    open: boolean;
    onClose?: () => void;
};

export const DialogTensorRT = ({ open, onClose }: DialogTensorRTProps) => {
    const { t } = useTranslation();
    const setIsFirstTensorRT = useSettingsStore((state) => state.setIsFirstTensorRT);
    const setExecutionProvider = useSettingsStore((state) => state.setExecutionProvider);

    const onNo = () => {
        // The user answered no to using TensorRT; so we switch to the next best option, CUDA.
        setExecutionProvider(ExecutionProvider.ExecutionProviderCUDA);

        setIsFirstTensorRT(false);
        onClose?.();
    };

    const onYes = () => {
        setIsFirstTensorRT(false);
        onClose?.();
    };

    return (
        <DialogGeneral title={t('dialogs.tensorRT.title')} open={open} className='w-[32rem]'>
            <div className='flex flex-col p-6 gap-6 items-center'>
                <img src={logo_tensorrt} alt={t('dialogs.tensorRT.logoAlt')} className='aspect-auto h-24' />

                <div className='flex flex-col text-center text-[#f2f2f2] gap-3'>
                    <Typography variant='body2'>{t('dialogs.tensorRT.intro')}</Typography>

                    {/* The emphasis is inside the sentence, so it has to travel with it. The object form of
                        `components` is used rather than positional <0>/<1> indices, so a translator sees a
                        meaningful tag name and can move it to wherever the emphasis belongs in their language. */}
                    <Typography variant='body2'>
                        <Trans
                            i18nKey='dialogs.tensorRT.optimization'
                            components={{ b: <span className='font-bold text-white' /> }}
                        />
                    </Typography>

                    <Typography variant='body2'>{t('dialogs.tensorRT.question')}</Typography>
                </div>

                <div className={`flex gap-3`}>
                    <Button option='secondary' className='w-36' onClick={onNo}>
                        {t('common.no')}
                    </Button>
                    <Button option='primary' className='w-36' onClick={onYes}>
                        {t('common.yes')}
                    </Button>
                </div>

                {/* The manual <br/> that used to balance these two lines is gone: it was tuned to the English
                    wording, and a translator has no way to know where the break belongs. Natural wrapping is right. */}
                <Typography variant='caption' className='text-[#9e9e9e] text-center'>
                    <Trans
                        i18nKey='dialogs.tensorRT.footer'
                        components={{ b: <span className='font-bold' />, u: <span className='underline' /> }}
                    />
                </Typography>
            </div>
        </DialogGeneral>
    );
};

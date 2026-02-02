import { Typography } from '@mui/material';
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
        <DialogGeneral title='TensorRT Detected' open={open} className='w-[32rem]'>
            <div className='flex flex-col p-6 gap-6 items-center'>
                <img src={logo_tensorrt} alt='TensorRT' className='aspect-auto h-24' />

                <div className='flex flex-col text-center text-[#f2f2f2] gap-3'>
                    <Typography variant='body2'>
                        We detected a GPU that supports TensorRT. TensorRT is an NVIDIA technology that can run AI
                        models significantly faster than CUDA.
                    </Typography>
                    <Typography variant='body2'>
                        When TensorRT is enabled, it must first optimize the model graph into a format it understands.{' '}
                        <span className='font-bold text-white'>
                            This optimization step can take a few minutes the first time it runs
                        </span>
                        ; subsequent runs will be much faster.
                    </Typography>
                    <Typography variant='body2'>Would you like to enable TensorRT?</Typography>
                </div>

                <div className={`flex gap-3`}>
                    <Button option='secondary' className='w-36' onClick={onNo}>
                        No
                    </Button>
                    <Button option='primary' className='w-36' onClick={onYes}>
                        Yes
                    </Button>
                </div>

                <Typography variant='caption' className='text-[#9e9e9e] text-center'>
                    Regardless of what you choose now, you can update your
                    <br />
                    choice at any time in the <span className='font-bold'>Settings</span> dialog, under{' '}
                    <span className='underline'>AI processor</span>.
                </Typography>
            </div>
        </DialogGeneral>
    );
};

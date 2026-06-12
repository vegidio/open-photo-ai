import type { CropInfo, File } from '@/bindings/gui/types';
import { useCropStore } from '@/stores';

export const useFileCrop = (file: File | undefined): CropInfo | undefined =>
    useCropStore((state) => (file ? state.crops.get(file) : undefined));

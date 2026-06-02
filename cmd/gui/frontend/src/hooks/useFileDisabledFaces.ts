import type { File } from '@/bindings/gui/types';
import { useEnhancementStore } from '@/stores';
import { EMPTY_DISABLED } from '@/utils/constants.ts';

export const useFileDisabledFaces = (file: File | undefined) =>
    useEnhancementStore((state) => (file ? (state.disabledFaces.get(file) ?? EMPTY_DISABLED) : EMPTY_DISABLED));

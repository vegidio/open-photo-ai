import type { File } from '@/bindings/gui/types';
import { useEnhancementStore } from '@/stores';
import { EMPTY_FACES } from '@/utils/constants.ts';

export const useFileFaces = (file: File | undefined) =>
    useEnhancementStore((state) => (file ? (state.faces.get(file) ?? EMPTY_FACES) : EMPTY_FACES));

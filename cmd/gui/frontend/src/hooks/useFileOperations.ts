import type { File } from '@/bindings/gui/types';
import { useEnhancementStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';

export const useFileOperations = (file: File | undefined) =>
    useEnhancementStore((state) => (file ? (state.enhancements.get(file) ?? EMPTY_OPERATIONS) : EMPTY_OPERATIONS));

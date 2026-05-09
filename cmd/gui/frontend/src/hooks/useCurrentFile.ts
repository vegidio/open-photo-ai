import { useFileStore } from '@/stores';

export const useCurrentFile = () => useFileStore((state) => state.files.at(state.currentIndex));

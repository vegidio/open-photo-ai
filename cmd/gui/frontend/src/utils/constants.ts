import type { Face } from '@/bindings/github.com/vegidio/open-photo-ai/models/detection';
import type { Operation } from '@/operations';
import { Version } from '@/bindings/gui/services/appservice.ts';
import { GetArch, GetOS } from '@/bindings/gui/services/osservice.ts';
import { CropInfo } from '@/bindings/gui/types';

export const version = await Version();
export const os = await GetOS();
export const arch = await GetArch();

export const EMPTY_OPERATIONS: Operation[] = [];
export const EMPTY_FACES: Face[] = [];
export const EMPTY_DISABLED: ReadonlySet<number> = new Set();
// Zero-value crop sent across the Wails boundary to mean "no crop" (a value-type CropInfo, never null/undefined).
export const EMPTY_CROP = new CropInfo({});

// Dark canvas with a dotted grid, shared by the Preview and the Crop/Rotate modal.
export const DOTTED_BACKGROUND = 'bg-[#171717] bg-[radial-gradient(#383838_1px,transparent_1px)] bg-size-[3rem_3rem]';

// Smallest allowed crop side (px), shared by the Crop/Rotate clamp and the dimension fields.
export const MIN_CROP_SIZE = 16;

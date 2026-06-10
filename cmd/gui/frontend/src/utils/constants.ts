import type { Face } from '@/bindings/github.com/vegidio/open-photo-ai/models/detection';
import type { Operation } from '@/operations';
import { Version } from '@/bindings/gui/services/appservice.ts';
import { GetArch, GetOS } from '@/bindings/gui/services/osservice.ts';

export const version = await Version();
export const os = await GetOS();
export const arch = await GetArch();

export const EMPTY_OPERATIONS: Operation[] = [];
export const EMPTY_FACES: Face[] = [];
export const EMPTY_DISABLED: ReadonlySet<number> = new Set();

// Dark canvas with a dotted grid, shared by the Preview and the Crop/Rotate modal.
export const DOTTED_BACKGROUND = 'bg-[#171717] bg-[radial-gradient(#383838_1px,transparent_1px)] bg-size-[3rem_3rem]';

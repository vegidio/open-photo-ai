import type { Operation } from '@/operations';
import { Version } from '@/bindings/gui/services/appservice.ts';
import { GetArch, GetOS } from '@/bindings/gui/services/osservice.ts';

export const version = await Version();
export const os = await GetOS();
export const arch = await GetArch();

export const EMPTY_OPERATIONS: Operation[] = [];

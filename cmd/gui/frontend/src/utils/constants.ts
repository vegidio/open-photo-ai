import { Version } from '../../bindings/gui/services/appservice.ts';
import { GetArch, GetOS } from '../../bindings/gui/services/environmentservice.ts';

export const version = await Version();
export const os = await GetOS();
export const arch = await GetArch();

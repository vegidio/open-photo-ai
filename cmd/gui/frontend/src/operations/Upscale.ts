import type { Operation } from './Operation';

export class Upscale implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(mode: 'general' | 'cartoon', scale: number, precision: string) {
        this.id = `upscale_${mode}_${scale}_${precision}`;
        this.options = { mode, scale: scale.toString(), precision };
    }
}

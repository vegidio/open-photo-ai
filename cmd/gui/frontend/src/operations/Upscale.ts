import type { Operation } from './Operation';

export class Kyoto implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(mode: 'general' | 'cartoon', scale: number, precision: string) {
        this.id = `up_kyoto_${mode}_${scale}_${precision}`;
        this.options = { mode, scale: scale.toString(), precision };
    }
}

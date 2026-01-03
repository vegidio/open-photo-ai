import type { Operation } from './Operation';

export class Tokyo implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(scale: number, precision: string) {
        this.id = `up_tokyo_${scale}x_${precision}`;
        this.options = { name: 'tokyo', scale: scale.toString(), precision };
    }
}

export class Kyoto implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(mode: 'general' | 'cartoon', scale: number, precision: string) {
        this.id = `up_kyoto_${mode}_${scale}x_${precision}`;
        this.options = { name: 'kyoto', mode, scale: scale.toString(), precision };
    }
}

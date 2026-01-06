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

    constructor(scale: number, precision: string) {
        this.id = `up_kyoto_${scale}x_${precision}`;
        this.options = { name: 'kyoto', scale: scale.toString(), precision };
    }
}

export class Saitama implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(scale: number, precision: string) {
        this.id = `up_saitama_${scale}x_${precision}`;
        this.options = { name: 'saitama', scale: scale.toString(), precision };
    }
}

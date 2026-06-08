import type { Operation } from './Operation';

export class Stockholm implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(intensity: number, precision: string) {
        this.id = `dn_stockholm_${intensity}_${precision}`;
        this.options = { name: 'stockholm', intensity: intensity.toString(), precision };
    }
}

export class Malmo implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(intensity: number, precision: string) {
        this.id = `dn_malmo_${intensity}_${precision}`;
        this.options = { name: 'malmo', intensity: intensity.toString(), precision };
    }
}

export class Gothenburg implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(intensity: number, precision: string) {
        this.id = `dn_gothenburg_${intensity}_${precision}`;
        this.options = { name: 'gothenburg', intensity: intensity.toString(), precision };
    }
}

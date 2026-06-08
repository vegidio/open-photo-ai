import type { Operation } from './Operation';

export class Moscow implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(intensity: number, precision: string) {
        this.id = `sh_moscow_${intensity}_${precision}`;
        this.options = { name: 'moscow', intensity: intensity.toString(), precision };
    }
}

export class Novgorod implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(intensity: number, precision: string) {
        this.id = `sh_novgorod_${intensity}_${precision}`;
        this.options = { name: 'novgorod', intensity: intensity.toString(), precision };
    }
}

export class Petersburg implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(intensity: number, precision: string) {
        this.id = `sh_petersburg_${intensity}_${precision}`;
        this.options = { name: 'petersburg', intensity: intensity.toString(), precision };
    }
}

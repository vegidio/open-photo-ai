import type { Operation } from './Operation';

export class Moscow implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(strength: number, precision: string) {
        this.id = `sh_moscow_${strength}_${precision}`;
        this.options = { name: 'moscow', strength: strength.toString(), precision };
    }
}

export class Novgorod implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(strength: number, precision: string) {
        this.id = `sh_novgorod_${strength}_${precision}`;
        this.options = { name: 'novgorod', strength: strength.toString(), precision };
    }
}

export class Petersburg implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(strength: number, precision: string) {
        this.id = `sh_petersburg_${strength}_${precision}`;
        this.options = { name: 'petersburg', strength: strength.toString(), precision };
    }
}

import type { Operation } from './Operation';

export class Stockholm implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(strength: number, precision: string) {
        this.id = `dn_stockholm_${strength}_${precision}`;
        this.options = { name: 'stockholm', strength: strength.toString(), precision };
    }
}

export class Malmo implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(strength: number, precision: string) {
        this.id = `dn_malmo_${strength}_${precision}`;
        this.options = { name: 'malmo', strength: strength.toString(), precision };
    }
}

export class Gothenburg implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(strength: number, precision: string) {
        this.id = `dn_gothenburg_${strength}_${precision}`;
        this.options = { name: 'gothenburg', strength: strength.toString(), precision };
    }
}

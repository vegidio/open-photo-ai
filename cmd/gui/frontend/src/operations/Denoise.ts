import type { Operation } from './Operation';

export class Stockholm implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(precision: string) {
        this.id = `dn_stockholm_${precision}`;
        this.options = { name: 'stockholm', precision };
    }
}

export class Malmo implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(precision: string) {
        this.id = `dn_malmo_${precision}`;
        this.options = { name: 'malmo', precision };
    }
}

import type { Operation } from './Operation';

export class Moscow implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(precision: string) {
        this.id = `sh_moscow_${precision}`;
        this.options = { name: 'moscow', precision };
    }
}

export class Novgorod implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(precision: string) {
        this.id = `sh_novgorod_${precision}`;
        this.options = { name: 'novgorod', precision };
    }
}

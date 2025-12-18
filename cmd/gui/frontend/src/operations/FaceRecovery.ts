import type { Operation } from './Operation';

export class Athens implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(precision: string) {
        this.id = `fr_athens_${precision}`;
        this.options = { precision };
    }
}

export class Santorini implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(precision: string) {
        this.id = `fr_santorini_${precision}`;
        this.options = { precision };
    }
}

import type { Operation } from './Operation';

export class Rio implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(intensity: number, precision: string) {
        this.id = `cb_rio_${intensity}_${precision}`;
        this.options = { name: 'rio', intensity: intensity.toString(), precision };
    }
}

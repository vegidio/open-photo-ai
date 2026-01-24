import type { Operation } from './Operation';

export class Paris implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(intensity: number, precision: string) {
        this.id = `la_paris_${intensity}_${precision}`;
        this.options = { name: 'paris', intensity: intensity.toString(), precision };
    }
}

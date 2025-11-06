import type { Operation } from './Operation';

export class FaceRecovery implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(precision: string) {
        this.id = `face_${precision}`;
        this.options = { precision };
    }
}

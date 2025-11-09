import type { Operation } from './Operation';

export class FaceRecovery implements Operation {
    id = '';
    options: Record<string, string> = {};

    constructor(mode: 'realistic' | 'creative', precision: string) {
        this.id = `face-recovery_${mode}_${precision}`;
        this.options = { mode, precision };
    }
}

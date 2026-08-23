import { describe, expect, it } from 'vitest';
import type { Operation } from '@/operations';
import {
    buildSelection,
    ENHANCEMENT_ORDER,
    ENHANCEMENTS,
    getEnhancementType,
    getOp,
    upscaleFactor,
} from './enhancement.ts';

const op = (id: string, options: Record<string, string> = {}): Operation => ({ id, options });

describe('getEnhancementType', () => {
    it('reads the type from the id prefix', () => {
        expect(getEnhancementType('dn_stockholm_fp32')).toBe('dn');
        expect(getEnhancementType('up_osaka_fp16')).toBe('up');
    });

    // The cast this replaced made ENHANCEMENTS[type] look total. A stale persisted operation, or one from a newer
    // backend, has to come back as undefined rather than indexing the catalog with a key that isn't in it.
    it('returns undefined for a prefix that names no enhancement', () => {
        expect(getEnhancementType('xx_whatever_fp32')).toBeUndefined();
        expect(getEnhancementType('')).toBeUndefined();
    });

    it('recognises every type the catalog declares', () => {
        for (const type of ENHANCEMENT_ORDER) {
            expect(getEnhancementType(`${type}_something_fp32`)).toBe(type);
        }
    });
});

describe('upscaleFactor', () => {
    it('is 1 when nothing upscales', () => {
        expect(upscaleFactor([])).toBe(1);
        expect(upscaleFactor([op('dn_stockholm_fp32', { intensity: '0.5' })])).toBe(1);
    });

    // The bug this guards: parseInt turned a 1.5x upscale into 1x, so the export queue and the navbar disagreed
    // about the output dimensions of the very same job.
    it('keeps fractional scales', () => {
        expect(upscaleFactor([op('up_tokyo_fp32', { scale: '1.5' })])).toBe(1.5);
        expect(upscaleFactor([op('up_tokyo_fp32', { scale: '2' })])).toBe(2);
    });

    it('falls back to 1 for a scale that is missing or unusable', () => {
        expect(upscaleFactor([op('up_tokyo_fp32')])).toBe(1);
        expect(upscaleFactor([op('up_tokyo_fp32', { scale: 'abc' })])).toBe(1);
        expect(upscaleFactor([op('up_tokyo_fp32', { scale: '0' })])).toBe(1);
    });
});

describe('the enhancement catalog', () => {
    // ENHANCEMENT_ORDER is both the add menu's order and the pipeline's order; the two agreeing is the whole point
    // of it being one list, so a type present in only one of the two would be a silent inconsistency.
    it('orders exactly the types it defines', () => {
        expect([...ENHANCEMENT_ORDER].sort()).toEqual(Object.keys(ENHANCEMENTS).sort());
    });

    it('gives every enhancement a default model that it actually offers', () => {
        for (const type of ENHANCEMENT_ORDER) {
            const info = ENHANCEMENTS[type];
            expect(info.models.map((m) => m.id)).toContain(info.defaultModel);
        }
    });
});

describe('getOp', () => {
    it('builds an operation whose id carries the requested model', () => {
        for (const type of ENHANCEMENT_ORDER) {
            const info = ENHANCEMENTS[type];
            const built = getOp(type, info.defaultModel);

            expect(built.id.startsWith(`${type}_${info.defaultModel}_`)).toBe(true);
        }
    });

    // A stored setting naming a model that was since renamed or removed must not produce a broken id.
    it('falls back to a known model for an unknown one', () => {
        const built = getOp('up', 'not-a-real-model');
        expect(getEnhancementType(built.id)).toBe('up');
    });
});

describe('buildSelection', () => {
    it('builds from a <model>_<precision> selection', () => {
        // Upscale ids carry the scale between the model and the precision (`up_kyoto_1x_fp32`), so this asserts the
        // ends of the id rather than the whole of it.
        const info = ENHANCEMENTS.up;
        const built = buildSelection('up', `${info.defaultModel}_fp32`);

        expect(built?.id.startsWith(`up_${info.defaultModel}_`)).toBe(true);
        expect(built?.id.endsWith('_fp32')).toBe(true);
    });

    it('honours the precision named in the selection', () => {
        const info = ENHANCEMENTS.up;
        expect(buildSelection('up', `${info.defaultModel}_fp16`)?.id.endsWith('_fp16')).toBe(true);
    });

    // useOptionEnhancement reads undefined as "leave the enhancement alone", which is what keeps a half-typed or
    // stale selection from cancelling inference that is already running.
    it('returns undefined when the selection names nothing known', () => {
        expect(buildSelection('up', 'nope_fp32')).toBeUndefined();
        expect(buildSelection('up', 'tokyo')).toBeUndefined();
        expect(buildSelection('up', '')).toBeUndefined();
    });
});

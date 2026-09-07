import { describe, expect, it } from 'vitest';
import type { Operation } from '@/operations';
import {
    buildSelection,
    DEFAULT_MODELS,
    ENHANCEMENT_ORDER,
    ENHANCEMENTS,
    getEnhancementType,
    getOp,
    modelPrecisions,
    modelTiers,
    normalizeModels,
    qualityTier,
    upscaleFactor,
} from './enhancement.ts';

const op = (id: string, options: Record<string, string> = {}): Operation => ({ id, options });

describe('getEnhancementType', () => {
    it('reads the type from the id prefix', () => {
        expect(getEnhancementType('dn_stockholm_fp32')).toBe('dn');
        expect(getEnhancementType('up_osaka_fp16')).toBe('up');
        expect(getEnhancementType('up_osaka_int8')).toBe('up');
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

    // Both `getOp` and `buildSelection` read a `<model>_<precision>` selection by splitting on the underscore, so a
    // codename containing one would silently resolve to the wrong model rather than fail.
    it('gives no model an id containing an underscore', () => {
        for (const type of ENHANCEMENT_ORDER) {
            for (const model of ENHANCEMENTS[type].models) {
                expect(model.id).not.toContain('_');
            }
        }
    });
});

describe('modelTiers', () => {
    it('lists every model at both tiers, HD first', () => {
        for (const type of ENHANCEMENT_ORDER) {
            const tiers = modelTiers(type);
            const models = ENHANCEMENTS[type].models;

            expect(tiers).toHaveLength(models.length * 2);
            expect(tiers.map((entry) => entry.tier)).toEqual(models.flatMap(() => ['hd', 'md']));
            expect(tiers.map((entry) => entry.model)).toEqual(models.flatMap((m) => [m.id, m.id]));
        }
    });

    it('carries the precision each tier is backed by', () => {
        expect(
            modelTiers('up')
                .filter((entry) => entry.model === 'kyoto')
                .map((entry) => entry.value),
        ).toEqual(['kyoto_fp32', 'kyoto_fp16']);

        // The reason the tier cannot be assumed to be a precision: Osaka has no fp32 build at all.
        expect(
            modelTiers('up')
                .filter((entry) => entry.model === 'osaka')
                .map((entry) => entry.value),
        ).toEqual(['osaka_fp16', 'osaka_int8']);
    });
});

describe('DEFAULT_MODELS', () => {
    // DEFAULT_MODELS is built at module scope from modelSelection, which is built from modelPrecisions. Declared in
    // the wrong order those are a temporal dead zone error thrown on import - a blank app rather than a red test - so
    // this asserts the values as well as their shape.
    it('is a valid HD selection for every enhancement', () => {
        for (const type of ENHANCEMENT_ORDER) {
            const info = ENHANCEMENTS[type];
            const [model, precision] = DEFAULT_MODELS[type].split('_');

            expect(model).toBe(info.defaultModel);
            expect(precision).toBe(modelPrecisions(type, info.defaultModel).hd);
        }
    });
});

describe('normalizeModels', () => {
    it('fills in a missing or unusable record', () => {
        expect(normalizeModels(undefined)).toEqual(DEFAULT_MODELS);
        expect(normalizeModels({})).toEqual(DEFAULT_MODELS);
        expect(normalizeModels('nonsense')).toEqual(DEFAULT_MODELS);
        expect(normalizeModels({ up: 42 })).toEqual(DEFAULT_MODELS);
    });

    // The upgrade path: everything persisted before the quality tier was selectable is a bare codename, and it meant
    // HD. Getting this wrong would move every existing user to the SD build of their chosen model.
    it('upgrades a bare codename to its HD selection', () => {
        expect(normalizeModels({ ...DEFAULT_MODELS, dn: 'malmo', up: 'osaka' })).toMatchObject({
            dn: 'malmo_fp32',
            up: 'osaka_fp16',
        });
    });

    it('keeps a selection that already names a real build', () => {
        expect(normalizeModels({ ...DEFAULT_MODELS, up: 'osaka_int8' }).up).toBe('osaka_int8');
        expect(normalizeModels({ ...DEFAULT_MODELS, dn: 'malmo_fp16' }).dn).toBe('malmo_fp16');
    });

    it('repairs a precision the named model does not publish', () => {
        expect(normalizeModels({ ...DEFAULT_MODELS, up: 'osaka_fp32' }).up).toBe('osaka_fp16');
        expect(normalizeModels({ ...DEFAULT_MODELS, up: 'kyoto_int8' }).up).toBe('kyoto_fp32');
    });

    it('falls back to a known model but keeps the tier asked for', () => {
        expect(normalizeModels({ ...DEFAULT_MODELS, up: 'nope_fp16' }).up).toBe('kyoto_fp16');
    });

    it('covers every enhancement whatever it is given', () => {
        expect(Object.keys(normalizeModels({ dn: 'malmo' })).sort()).toEqual([...ENHANCEMENT_ORDER].sort());
    });
});

describe('modelPrecisions / qualityTier', () => {
    it('defaults to fp32 for HD and fp16 for SD', () => {
        expect(modelPrecisions('up', 'kyoto')).toEqual({ hd: 'fp32', md: 'fp16' });
        expect(qualityTier('up', 'kyoto', 'fp32')).toBe('hd');
        expect(qualityTier('up', 'kyoto', 'fp16')).toBe('md');
    });

    // Osaka is the reason the tier stopped being derivable from the precision alone: no fp32 build of it exists, so
    // its fp16 build is the HD tier and an int8 one is the SD tier. Reading the tier off `precision === 'fp32'` would
    // label both of its entries "SD".
    it('follows the model when its tiers are not the convention', () => {
        expect(modelPrecisions('up', 'osaka')).toEqual({ hd: 'fp16', md: 'int8' });
        expect(qualityTier('up', 'osaka', 'fp16')).toBe('hd');
        expect(qualityTier('up', 'osaka', 'int8')).toBe('md');
    });

    // Same rehydration case getEnhancementType guards: an id from a persisted setting or an older cache entry.
    it('falls back to the convention for an unknown model', () => {
        expect(modelPrecisions('up', 'not-a-real-model')).toEqual({ hd: 'fp32', md: 'fp16' });
        expect(qualityTier('up', 'not-a-real-model', 'fp32')).toBe('hd');
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

    // Also the migration case: a selection with no precision is what every build before the quality tier was
    // selectable persisted, and it has to keep meaning HD.
    it("builds at the model's HD precision when the selection names none", () => {
        expect(getOp('up', 'kyoto').id.endsWith('_fp32')).toBe(true);
        expect(getOp('up', 'osaka').id.endsWith('_fp16')).toBe(true);
    });

    it('builds at the precision the selection names', () => {
        expect(getOp('up', 'kyoto_fp16').id.endsWith('_fp16')).toBe(true);
        expect(getOp('up', 'osaka_int8').id.endsWith('_int8')).toBe(true);
        expect(getOp('dn', 'malmo_fp16').id.endsWith('_fp16')).toBe(true);
    });

    // Asking for a build that was never published would send the backend after a model file that does not exist, so
    // an impossible precision collapses onto the tier it was closest to.
    it('repairs a precision the model does not publish', () => {
        expect(getOp('up', 'osaka_fp32').id.endsWith('_fp16')).toBe(true);
        expect(getOp('up', 'kyoto_int8').id.endsWith('_fp32')).toBe(true);
    });

    // A stored setting naming a model that was since renamed or removed must not produce a broken id.
    it('falls back to a known model for an unknown one', () => {
        const built = getOp('up', 'not-a-real-model');
        expect(getEnhancementType(built.id)).toBe('up');
    });

    // The tier is the user's statement about quality, and it outlives the model they picked it on.
    it('keeps the tier when it falls back to another model', () => {
        expect(getOp('up', 'not-a-real-model_fp16').id.endsWith('_fp16')).toBe(true);
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

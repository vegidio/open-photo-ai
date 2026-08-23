import { describe, expect, it } from 'vitest';
import type { CropInfo } from '@/bindings/gui/types';
import { cropToken } from './crop.ts';

const crop = (overrides: Partial<CropInfo> = {}): CropInfo =>
    ({
        Rotation: 0,
        FlipH: false,
        FlipV: false,
        Left: 0,
        Top: 0,
        Width: 100,
        Height: 50,
        ...overrides,
    }) as CropInfo;

describe('cropToken', () => {
    // An uncropped image has to key exactly as it did before crops existed, or every cached entry is orphaned.
    it('is empty when there is no crop', () => {
        expect(cropToken(undefined)).toBe('');
    });

    it('encodes every field that changes the pixels', () => {
        expect(cropToken(crop())).toBe('_c0-00-0-0-100-50');
        expect(cropToken(crop({ Rotation: 90 }))).toBe('_c90-00-0-0-100-50');
        expect(cropToken(crop({ FlipH: true }))).toBe('_c0-10-0-0-100-50');
        expect(cropToken(crop({ FlipV: true }))).toBe('_c0-01-0-0-100-50');
    });

    // The point of the token is that two different crops never collide in the cache.
    it('distinguishes crops that differ in only one field', () => {
        const tokens = new Set([
            cropToken(crop()),
            cropToken(crop({ Left: 1 })),
            cropToken(crop({ Top: 1 })),
            cropToken(crop({ Width: 101 })),
            cropToken(crop({ Height: 51 })),
            cropToken(crop({ Rotation: 180 })),
        ]);

        expect(tokens.size).toBe(6);
    });
});

import { describe, expect, it } from 'vitest';
import { clampQuality, DEFAULT_QUALITY, normalizeQuality } from './quality.ts';
import { ImageFormat } from '@/bindings/github.com/vegidio/open-photo-ai/types';

describe('clampQuality', () => {
    it('keeps a value in range', () => {
        expect(clampQuality(1)).toBe(1);
        expect(clampQuality(55)).toBe(55);
        expect(clampQuality(100)).toBe(100);
    });

    // 0 is the dangerous one: the native encoders take it as a real request rather than "use your default", and
    // write out a garbage image.
    it('clamps out-of-range values', () => {
        expect(clampQuality(0)).toBe(1);
        expect(clampQuality(-20)).toBe(1);
        expect(clampQuality(250)).toBe(100);
    });

    it('rounds fractional values', () => {
        expect(clampQuality(60.4)).toBe(60);
        expect(clampQuality(60.6)).toBe(61);
    });
});

describe('normalizeQuality', () => {
    it('falls back to the defaults when nothing was stored', () => {
        expect(normalizeQuality(undefined)).toEqual(DEFAULT_QUALITY);
        expect(normalizeQuality({})).toEqual(DEFAULT_QUALITY);
    });

    it('keeps the stored values it can use', () => {
        expect(normalizeQuality({ [ImageFormat.FormatJpeg]: 42 })).toEqual({
            ...DEFAULT_QUALITY,
            [ImageFormat.FormatJpeg]: 42,
        });
    });

    // A record written by an older build, or hand-edited in localStorage, must not reach the encoders as-is.
    it('replaces unusable values with the format default', () => {
        for (const stored of [0, -1, Number.NaN, 'nonsense', null]) {
            expect(normalizeQuality({ [ImageFormat.FormatWebp]: stored })[ImageFormat.FormatWebp]).toBe(
                DEFAULT_QUALITY[ImageFormat.FormatWebp],
            );
        }
    });

    it('clamps a stored value that is out of range', () => {
        expect(normalizeQuality({ [ImageFormat.FormatAvif]: 900 })[ImageFormat.FormatAvif]).toBe(100);
    });

    it('drops keys that are not quality formats', () => {
        expect(normalizeQuality({ [ImageFormat.FormatPng]: 10, nonsense: 5 })).toEqual(DEFAULT_QUALITY);
    });
});

import { describe, expect, it } from 'vitest';
import { formatList, MP_BUCKETS, mpBucket } from './buckets';

describe('mpBucket', () => {
    it('places common camera sizes in the expected band', () => {
        expect(mpBucket(1280, 720)).toBe('<2'); // 0.9 MP - a thumbnail or a screenshot
        expect(mpBucket(1920, 1080)).toBe('2-6'); // 2.1 MP - 1080p
        expect(mpBucket(3840, 2160)).toBe('6-12'); // 8.3 MP - 4K
        expect(mpBucket(5472, 3648)).toBe('12-24'); // 20.0 MP - APS-C
        expect(mpBucket(8256, 5504)).toBe('24-50'); // 45.4 MP - high-megapixel full frame
        expect(mpBucket(11648, 8736)).toBe('50+'); // 101.8 MP - medium format
    });

    // Each boundary belongs to the band above it, so a batch of identically sized images cannot straddle two buckets.
    // 6000x4000 - the most common full-frame sensor there is - lands exactly on 24.0 MP, so this rule decides where a
    // very large share of real photos is counted.
    it('puts each boundary in the upper band', () => {
        expect(mpBucket(2000, 1000)).toBe('2-6'); // exactly 2.0 MP
        expect(mpBucket(3000, 2000)).toBe('6-12'); // exactly 6.0 MP
        expect(mpBucket(4000, 3000)).toBe('12-24'); // exactly 12.0 MP
        expect(mpBucket(6000, 4000)).toBe('24-50'); // exactly 24.0 MP
        expect(mpBucket(10000, 5000)).toBe('50+'); // exactly 50.0 MP
    });

    // This runs on the analytics path, which must never be able to throw into the call site that reports an event.
    it('degrades to the smallest band rather than throwing', () => {
        for (const [w, h] of [
            [0, 0],
            [-100, 100],
            [100, -100],
            [Number.NaN, 100],
            [Number.POSITIVE_INFINITY, 100],
        ]) {
            expect(mpBucket(w, h)).toBe('<2');
        }
    });

    it('only ever returns a declared bucket', () => {
        for (const size of [1, 500, 1080, 4000, 12000, 40000]) {
            expect(MP_BUCKETS).toContain(mpBucket(size, size));
        }
    });
});

describe('formatList', () => {
    it('normalises, de-duplicates and sorts', () => {
        expect(formatList(['.JPG', 'jpg', '.png'])).toBe('jpg,png');
    });

    // Order must not matter: the same mix of files dragged in a different order has to be one value, not two.
    it('is order-independent', () => {
        expect(formatList(['.cr2', '.jpg'])).toBe(formatList(['.jpg', '.cr2']));
    });

    it('drops empty extensions rather than emitting a stray separator', () => {
        expect(formatList(['', '.jpg', '.'])).toBe('jpg');
        expect(formatList([])).toBe('');
    });

    // Only ever extensions - a file name here would be user content on a telemetry payload.
    it('keeps a dotted name to its final extension only', () => {
        expect(formatList(['.tar.gz'])).toBe('tar.gz');
    });
});

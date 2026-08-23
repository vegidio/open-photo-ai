import { describe, expect, it } from 'vitest';
import { formatBytes } from './format.ts';

describe('formatBytes', () => {
    it('uses KB below one megabyte', () => {
        expect(formatBytes(0)).toBe('0.00 KB');
        expect(formatBytes(1_500)).toBe('1.50 KB');
        expect(formatBytes(999_999)).toBe('1000.00 KB');
    });

    it('switches to MB at exactly one million bytes', () => {
        expect(formatBytes(1_000_000)).toBe('1.00 MB');
        expect(formatBytes(2_500_000)).toBe('2.50 MB');
    });
});

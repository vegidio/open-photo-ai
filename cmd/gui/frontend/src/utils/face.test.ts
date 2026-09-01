import { describe, expect, it, vi } from 'vitest';
import { Precision } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { Athens, Santorini } from '@/operations';

// face.ts reaches utils/constants.ts, which top-level-awaits Version() and GetOS() over the Wails bridge — so
// importing it under `environment: 'node'` throws on the missing `window` before a single test runs. Only these two
// calls are stubbed, rather than the whole constants module: the real module graph then loads, so this exercises
// face.ts as it actually ships instead of a stand-in for it.
vi.mock('@/bindings/gui/services/appservice.ts', () => ({ Version: () => Promise.resolve('0.0.0-test') }));
vi.mock('@/bindings/gui/services/osservice.ts', () => ({ GetOS: () => Promise.resolve('darwin') }));

const { faceRecoveryPrecision, hasFaceRecovery } = await import('./face.ts');

// The detection pass runs at whatever precision the face-recovery operation was built at, so this function is the
// only thing deciding which of the two New York graphs gets downloaded and run. Getting it wrong is quiet: the
// wrong-precision model still detects the same faces, it just means shipping a second 88 MB graph to feed a 44 MB one.
describe('faceRecoveryPrecision', () => {
    // Built through the operation classes rather than from literals, so a change to the id template in
    // operations/factory.ts breaks this test instead of silently changing which segment the precision is read from.
    it('reads the precision off the face-recovery operation', () => {
        expect(faceRecoveryPrecision([new Athens('fp32').id])).toBe(Precision.PrecisionFp32);
        expect(faceRecoveryPrecision([new Athens('fp16').id])).toBe(Precision.PrecisionFp16);
        expect(faceRecoveryPrecision([new Santorini('fp16').id])).toBe(Precision.PrecisionFp16);
    });

    it('finds the face-recovery operation among others', () => {
        const ops = ['dn_stockholm_1_fp32', new Santorini('fp16').id, 'up_kyoto_4x_fp32'];
        expect(faceRecoveryPrecision(ops)).toBe(Precision.PrecisionFp16);
    });

    // Undefined is the "nothing to detect" signal both callers branch on, so it has to be undefined and not a
    // default: a default would run a detection pass for a pipeline that has no use for faces.
    it('is undefined when no face-recovery operation is present', () => {
        expect(faceRecoveryPrecision([])).toBeUndefined();
        expect(faceRecoveryPrecision(['dn_stockholm_1_fp32', 'up_kyoto_4x_fp32'])).toBeUndefined();
    });

    // The prefix test must not match an operation that merely contains "fr_", only one that starts with it.
    it('agrees with hasFaceRecovery about whether faces are needed', () => {
        for (const ops of [[], ['cl_delhi_fp32'], [new Athens('fp32').id], ['sh_moscow_1_fp16', new Santorini('fp32').id]]) {
            expect(faceRecoveryPrecision(ops) !== undefined).toBe(hasFaceRecovery(ops));
        }
    });
});

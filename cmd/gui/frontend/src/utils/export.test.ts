import { describe, expect, it, vi } from 'vitest';
import type { File } from '@/bindings/gui/types';
import { getExportInfo } from './export.ts';

// getExportInfo is pure, but its module graph reaches utils/constants.ts, whose top-level await asks the Wails bridge
// for the app version and platform before anything importing it can evaluate. There is no bridge in a unit test, so
// those two services are stubbed - which is also a fair demonstration of how far that one await reaches.
vi.mock('@/bindings/gui/services/appservice.ts', () => ({
    Version: () => Promise.resolve('0.0.0-test'),
}));

vi.mock('@/bindings/gui/services/osservice.ts', () => ({
    GetOS: () => Promise.resolve('darwin'),
}));

const file = (path: string, extension: string): File => ({ Path: path, Extension: extension }) as File;

describe('getExportInfo', () => {
    it('applies the prefix and suffix around the base name', () => {
        const { fileName } = getExportInfo(file('/photos/beach.jpg', 'jpg'), 'png', 'new_', '_edited');
        expect(fileName).toBe('new_beach_edited.png');
    });

    it('writes beside the source when no location is given', () => {
        const { filePath } = getExportInfo(file('/photos/beach.jpg', 'jpg'), 'png', '', '');
        expect(filePath).toBe('/photos/beach.png');
    });

    it('writes into the chosen location when there is one', () => {
        const { filePath } = getExportInfo(file('/photos/beach.jpg', 'jpg'), 'png', '', '', '/exports');
        expect(filePath).toBe('/exports/beach.png');
    });

    it('keeps the source extension for "preserve"', () => {
        expect(getExportInfo(file('/photos/beach.jpg', 'jpg'), 'preserve', '', '').ext).toBe('jpg');
    });

    // RAW is decodable but not encodable, so "preserve" on a RAW file has to land on something writable rather than
    // producing a .cr2 the encoder will refuse.
    it('falls back to tiff when the source format cannot be written', () => {
        expect(getExportInfo(file('/photos/shot.cr2', 'cr2'), 'preserve', '', '').ext).toBe('tiff');
        expect(getExportInfo(file('/photos/shot.arw', 'arw'), 'preserve', '', '').ext).toBe('tiff');
    });

    it('strips only the final extension from the base name', () => {
        const { fileName } = getExportInfo(file('/photos/my.holiday.photo.jpg', 'jpg'), 'png', '', '');
        expect(fileName).toBe('my.holiday.photo.png');
    });
});

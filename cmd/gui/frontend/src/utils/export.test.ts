import { describe, expect, it, vi } from 'vitest';
import { ImageFormat } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import type { File } from '@/bindings/gui/types';
import { getExportInfo, getQualityFormat, resolveQualityFormat } from './export.ts';

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

describe('getQualityFormat', () => {
    it('resolves the four formats whose encoders take a quality', () => {
        expect(getQualityFormat(file('/photos/a.png', 'png'), 'avif')).toBe(ImageFormat.FormatAvif);
        expect(getQualityFormat(file('/photos/a.png', 'png'), 'heic')).toBe(ImageFormat.FormatHeic);
        expect(getQualityFormat(file('/photos/a.png', 'png'), 'jpg')).toBe(ImageFormat.FormatJpeg);
        expect(getQualityFormat(file('/photos/a.png', 'png'), 'webp')).toBe(ImageFormat.FormatWebp);
    });

    // The whole reason the stored quality is keyed by ImageFormat rather than by extension: a queue of .jpg and .jpeg
    // files is one format with one remembered value, not two.
    it('collapses the extension aliases onto one format', () => {
        expect(getQualityFormat(file('/photos/a.jpeg', 'jpeg'), 'preserve')).toBe(
            getQualityFormat(file('/photos/a.jpg', 'jpg'), 'preserve'),
        );
        expect(getQualityFormat(file('/photos/a.heif', 'heif'), 'preserve')).toBe(
            getQualityFormat(file('/photos/a.heic', 'heic'), 'preserve'),
        );
    });

    it('has no quality for the lossless formats', () => {
        for (const ext of ['png', 'tiff', 'tif', 'bmp', 'gif']) {
            expect(getQualityFormat(file(`/photos/a.${ext}`, ext), ext)).toBeUndefined();
        }
    });

    // A RAW input resolves to tiff, which is lossless - so no slider, whatever the queue.
    it('has no quality for a RAW source under "preserve"', () => {
        expect(getQualityFormat(file('/photos/shot.cr2', 'cr2'), 'preserve')).toBeUndefined();
    });
});

describe('resolveQualityFormat', () => {
    const jpg = file('/photos/a.jpg', 'jpg');
    const jpeg = file('/photos/b.jpeg', 'jpeg');
    const webp = file('/photos/c.webp', 'webp');
    const png = file('/photos/d.png', 'png');
    const raw = file('/photos/e.cr2', 'cr2');

    it('ignores the sources when an explicit format is chosen', () => {
        expect(resolveQualityFormat([jpg, png, raw], 'webp')).toBe(ImageFormat.FormatWebp);
        expect(resolveQualityFormat([jpg, png, raw], 'png')).toBeUndefined();
    });

    it('resolves a "preserve" queue that agrees on one format', () => {
        expect(resolveQualityFormat([jpg, jpeg], 'preserve')).toBe(ImageFormat.FormatJpeg);
    });

    // One slider cannot honestly stand for two formats at once, so it is hidden and each file falls back to its own
    // stored quality at export time.
    it('resolves nothing for a mixed "preserve" queue', () => {
        expect(resolveQualityFormat([jpg, webp], 'preserve')).toBeUndefined();
        expect(resolveQualityFormat([jpg, png], 'preserve')).toBeUndefined();
        expect(resolveQualityFormat([jpg, raw], 'preserve')).toBeUndefined();
    });

    it('resolves nothing for an empty queue', () => {
        expect(resolveQualityFormat([], 'jpg')).toBeUndefined();
    });
});

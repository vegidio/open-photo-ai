import { beforeEach, describe, expect, it, vi } from 'vitest';

const trackEvent = vi.fn((_event: string, _params?: unknown) => Promise.resolve());
const init = vi.fn((_key: string, _options?: unknown) => undefined);

vi.mock('@aptabase/web', () => ({
    init: (key: string, options?: unknown) => init(key, options),
    trackEvent: (event: string, params?: unknown) => trackEvent(event, params),
}));

// The real module reads the app version through a top-level await on the Wails bindings, which has no backend to
// answer it here.
vi.mock('@/utils/constants.ts', () => ({ version: '0.0.0-test' }));

describe('track', () => {
    beforeEach(() => {
        vi.resetModules();
        trackEvent.mockClear();
        init.mockClear();
    });

    // The key ships as the literal `<aptabase_key>` and is only substituted in CI, so every local and dev build hits
    // this path. If the guard regressed, developers would silently write events into the production project.
    it('sends nothing while the app key is an unsubstituted placeholder', async () => {
        vi.doMock('./config.ts', () => ({ aptabaseAppKey: '<aptabase_key>' }));

        const { AnalyticsEvent, initAnalytics, track } = await import('./index.ts');

        initAnalytics(true);
        track(AnalyticsEvent.AppOpen);

        expect(init).not.toHaveBeenCalled();
        expect(trackEvent).not.toHaveBeenCalled();
    });

    it('sends nothing when the user has opted out', async () => {
        vi.doMock('./config.ts', () => ({ aptabaseAppKey: 'A-EU-1234567890' }));

        const { AnalyticsEvent, initAnalytics, track } = await import('./index.ts');

        initAnalytics(false);
        track(AnalyticsEvent.AppOpen);

        expect(trackEvent).not.toHaveBeenCalled();
    });

    it('honours the opt-out flipping at runtime, in both directions', async () => {
        vi.doMock('./config.ts', () => ({ aptabaseAppKey: 'A-EU-1234567890' }));

        const { AnalyticsEvent, initAnalytics, setAnalyticsEnabled, track } = await import('./index.ts');

        initAnalytics(true);
        track(AnalyticsEvent.AppOpen);
        expect(trackEvent).toHaveBeenCalledTimes(1);

        setAnalyticsEnabled(false);
        track(AnalyticsEvent.AppOpen);
        expect(trackEvent).toHaveBeenCalledTimes(1);

        setAnalyticsEnabled(true);
        track(AnalyticsEvent.AppOpen);
        expect(trackEvent).toHaveBeenCalledTimes(2);
    });

    it('passes the event name and properties through', async () => {
        vi.doMock('./config.ts', () => ({ aptabaseAppKey: 'A-EU-1234567890' }));

        const { AnalyticsEvent, initAnalytics, track } = await import('./index.ts');

        initAnalytics(true);
        track(AnalyticsEvent.ExportCompleted, { format: 'jpeg', duration_ms: 1200 });

        expect(trackEvent).toHaveBeenCalledWith('export_completed', { format: 'jpeg', duration_ms: 1200 });
    });

    // Analytics must never be able to break the call site that reports an event.
    it('swallows a rejecting transport', async () => {
        vi.doMock('./config.ts', () => ({ aptabaseAppKey: 'A-EU-1234567890' }));
        trackEvent.mockImplementationOnce(() => Promise.reject(new Error('offline')));

        const { AnalyticsEvent, initAnalytics, track } = await import('./index.ts');

        initAnalytics(true);
        expect(() => track(AnalyticsEvent.AppOpen)).not.toThrow();
    });
});

describe('AnalyticsEvent', () => {
    // Aptabase has no schema migration, so a name is a permanent contract. Two events sharing one name would silently
    // merge on the dashboard.
    it('has no duplicate event names', async () => {
        vi.doMock('./config.ts', () => ({ aptabaseAppKey: 'A-EU-1234567890' }));

        const { AnalyticsEvent } = await import('./index.ts');
        const names = Object.values(AnalyticsEvent);

        expect(new Set(names).size).toBe(names.length);
    });

    // The two renamed events must stay renamed: leaving the old names live would keep charting the old, wrong
    // quantities alongside the new ones.
    it('no longer declares the names that were renamed', async () => {
        vi.doMock('./config.ts', () => ({ aptabaseAppKey: 'A-EU-1234567890' }));

        const { AnalyticsEvent } = await import('./index.ts');
        const names: string[] = Object.values(AnalyticsEvent);

        expect(names).not.toContain('image_processed');
        expect(names).not.toContain('export_started');
    });
});

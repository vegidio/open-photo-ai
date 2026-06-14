import { measurementApiSecret, measurementId } from './config.ts';

// Single source of truth for analytics event names. GA4 constraints: snake_case, ≤ 40 chars, ≤ 25 params per event,
// and avoid the reserved names. Add new events here so every `track()` call site stays type-checked.
export const AnalyticsEvent = {
    AppOpen: 'app_open',
    AppInitialized: 'app_initialized',
    InitFailed: 'init_failed',
    FilesAdded: 'files_added',
    FileRemoved: 'file_removed',
    FilesCleared: 'files_cleared',
    EnhancementAdded: 'enhancement_added',
    EnhancementRemoved: 'enhancement_removed',
    AutopilotRun: 'autopilot_run',
    ImageProcessed: 'image_processed',
    ProcessFailed: 'process_failed',
    CropApplied: 'crop_applied',
    ExportStarted: 'export_started',
    ExportCompleted: 'export_completed',
    ExportFailed: 'export_failed',
    ExecutionProviderChanged: 'execution_provider_changed',
} as const;

export type AnalyticsEvent = (typeof AnalyticsEvent)[keyof typeof AnalyticsEvent];

type AnalyticsParams = Record<string, string | number | boolean>;

// We send events via the GA4 Measurement Protocol instead of the Firebase Analytics web SDK: the SDK's gtag transport
// silently fails to transmit from Wails' custom-scheme (wails://wails.localhost) webview, whereas a direct POST works.
// The destination is the same GA4 property behind the Firebase project, so events still land in Firebase Analytics.
const MP_ENDPOINT = 'https://www.google-analytics.com/mp/collect';

const CLIENT_ID_KEY = 'analytics-client-id';

// Module state: `enabled` mirrors the persisted opt-out; `clientId` is a stable per-install id GA4 requires.
let enabled = true;
let clientId = '';

// A stable client id, persisted across launches. Uses getRandomValues (works in any context) rather than
// crypto.randomUUID() (which needs a secure context the wails:// origin may not provide).
const loadClientId = (): string => {
    try {
        const existing = localStorage.getItem(CLIENT_ID_KEY);
        if (existing) return existing;

        const bytes = new Uint8Array(16);
        crypto.getRandomValues(bytes);
        const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
        const id = `${hex.slice(0, 16)}.${hex.slice(16)}`;
        localStorage.setItem(CLIENT_ID_KEY, id);
        return id;
    } catch {
        // localStorage unavailable — fall back to an ephemeral id (per-session, still valid for GA4).
        return `${Date.now()}.${Math.floor(Math.random() * 1e9)}`;
    }
};

/** Initializes analytics. Honors `collectionEnabled` so a persisted opt-out is respected from the first event. */
export const initAnalytics = (collectionEnabled = true): void => {
    enabled = collectionEnabled;
    clientId = loadClientId();
};

/** Toggles whether analytics events are collected. Wired to the Settings opt-out switch. */
export const setAnalyticsEnabled = (value: boolean): void => {
    enabled = value;
};

/** Logs a single analytics event. No-ops when collection is disabled or analytics isn't configured. */
export const track = (event: AnalyticsEvent, params?: AnalyticsParams): void => {
    if (!enabled) return;
    if (!measurementId || !measurementApiSecret || measurementApiSecret.startsWith('<')) return;

    const body = JSON.stringify({ client_id: clientId, events: [{ name: event, params }] });
    const url = `${MP_ENDPOINT}?measurement_id=${measurementId}&api_secret=${measurementApiSecret}`;

    // no-cors: the MP endpoint sends no CORS headers and we don't read the response. keepalive lets in-flight events
    // survive an app/window close. Fire-and-forget — analytics must never throw into the app.
    fetch(url, { method: 'POST', mode: 'no-cors', body, keepalive: true }).catch(() => {});
};

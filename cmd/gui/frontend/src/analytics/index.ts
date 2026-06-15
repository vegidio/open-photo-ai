import { init, trackEvent } from '@aptabase/web';
import { version } from '@/utils/constants.ts';
import { aptabaseAppKey } from './config.ts';

// Single source of truth for analytics event names. Add new events here so every `track()` call site stays type-checked.
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

// Module state: `enabled` mirrors the persisted opt-out. Aptabase has no built-in opt-out, so collection is gated here.
let enabled = true;

// The App Key literal ships as a placeholder until set; treat that as "not configured" so we never init/track with it.
const keyConfigured = (): boolean => !!aptabaseAppKey && !aptabaseAppKey.startsWith('A-XX');

/** Initializes analytics. Honors `collectionEnabled` so a persisted opt-out is respected from the first event. */
export const initAnalytics = (collectionEnabled = true): void => {
    enabled = collectionEnabled;
    if (keyConfigured()) init(aptabaseAppKey, { appVersion: version });
};

/** Toggles whether analytics events are collected. Wired to the Settings opt-out switch. */
export const setAnalyticsEnabled = (value: boolean): void => {
    enabled = value;
};

/** Logs a single analytics event. No-ops when collection is disabled or analytics isn't configured. */
export const track = (event: AnalyticsEvent, params?: AnalyticsParams): void => {
    if (!enabled || !keyConfigured()) return;

    // Fire-and-forget — analytics must never throw into the app. The SDK already swallows network errors; the extra
    // catch guards against init-not-called or other synchronous rejections.
    void trackEvent(event, params).catch(() => {});
};

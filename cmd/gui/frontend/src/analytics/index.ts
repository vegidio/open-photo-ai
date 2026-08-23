import { init, trackEvent } from '@aptabase/web';
import { aptabaseAppKey } from './config.ts';
import { version } from '@/utils/constants.ts';

// Single source of truth for analytics event names. Add new events here so every `track()` call site stays type-checked.
export const AnalyticsEvent = {
    AppOpen: 'app_open',
    AppInitialized: 'app_initialized',
    InitFailed: 'init_failed',

    FilesAdded: 'files_added',
    FileRemoved: 'file_removed',
    FilesCleared: 'files_cleared',

    // `type` falls back to 'unknown' when an operation id no longer maps to an enhancement type. That means a stale id
    // was rehydrated from a previous version's persisted state, not that anything crashed - so read a spike here as a
    // migration problem rather than a bug in the enhancement pipeline.
    EnhancementAdded: 'enhancement_added',
    EnhancementRemoved: 'enhancement_removed',

    AutopilotRun: 'autopilot_run',
    AutopilotFailed: 'autopilot_failed',

    // Renamed from `image_processed`, which was never what its name claimed. It fires from the preview effect, whose
    // dependencies include the crop, the disabled faces and the execution provider, and it fires on a frontend cache
    // hit too - so it counted preview renders, several per edit, not images enhanced. The rename is deliberate: the
    // old name's dashboards should break rather than quietly keep charting a different quantity.
    //
    // `export_completed` is the honest "the user enhanced an image and kept it" signal.
    PreviewRendered: 'preview_rendered',
    ProcessFailed: 'process_failed',

    CropApplied: 'crop_applied',
    CropCancelled: 'crop_cancelled',

    // Batch and per-file events are named apart because they can never be compared one to one: a batch covers many
    // files. Counting exports means counting `export_completed`.
    ExportBatchStarted: 'export_batch_started',
    ExportBatchFinished: 'export_batch_finished',
    ExportCompleted: 'export_completed',
    ExportFailed: 'export_failed',

    ExecutionProviderChanged: 'execution_provider_changed',
    ProviderFallback: 'provider_fallback',
    ModelChanged: 'model_changed',
    QualityChanged: 'quality_changed',
    LanguageChanged: 'language_changed',
    PreviewModeChanged: 'preview_mode_changed',

    FacesDetected: 'faces_detected',
    TensorRtPromptAnswered: 'tensorrt_prompt_answered',
    SettingsOpened: 'settings_opened',
    UpdateAvailable: 'update_available',

    RenderCrashed: 'render_crashed',
} as const;

export type AnalyticsEvent = (typeof AnalyticsEvent)[keyof typeof AnalyticsEvent];

type AnalyticsParams = Record<string, string | number | boolean>;

// Module state: `enabled` mirrors the persisted opt-out. Aptabase has no built-in opt-out, so collection is gated here.
let enabled = true;

// The App Key literal ships as a placeholder until set; treat that as "not configured" so we never init/track with it.
// The placeholder is the `<aptabase_key>` literal that CI replaces (see .github/workflows/build.yml), so the angle
// bracket is what identifies an un-substituted build — matching on the key's own prefix would let the placeholder pass.
const keyConfigured = (): boolean => !!aptabaseAppKey && !aptabaseAppKey.startsWith('<');

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

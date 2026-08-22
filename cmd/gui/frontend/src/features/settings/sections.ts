import type { ParseKeys } from 'i18next';

type SettingsItem = {
    // Doubles as the anchor id in SettingsList and the tree node id in SettingsMenu. Defining it once here is what
    // stops the two from drifting: a mismatch used to compile fine and just make the nav entry scroll nowhere.
    id: string;
    labelKey: ParseKeys;
};

type SettingsSection = SettingsItem & {
    items: SettingsItem[];
};

// The structure of the Settings dialog, read by both panes. SettingsMenu renders it as a tree and derives its
// expanded ids from it; SettingsList renders the subheaders and looks each row's component up by id.
export const SETTINGS_SECTIONS: SettingsSection[] = [
    {
        id: 'app',
        labelKey: 'settings.sections.app',
        items: [
            { id: 'app_language', labelKey: 'settings.app.language.title' },
            { id: 'app_logs', labelKey: 'settings.app.logs.title' },
            { id: 'app_analytics', labelKey: 'settings.app.analytics.title' },
        ],
    },
    {
        id: 'performance',
        labelKey: 'settings.sections.performance',
        items: [{ id: 'perf_processor', labelKey: 'settings.performance.aiProcessor.title' }],
    },
    {
        id: 'enhancements',
        labelKey: 'settings.sections.enhancements',
        // These reuse the same name keys the enhancement list and add menu render, so the dialog can't disagree with
        // the rest of the app about what an enhancement is called.
        items: [
            { id: 'enh_denoise', labelKey: 'enhancements.denoise.name' },
            { id: 'enh_face', labelKey: 'enhancements.faceRecovery.name' },
            { id: 'enh_colorization', labelKey: 'enhancements.colorization.name' },
            { id: 'enh_light', labelKey: 'enhancements.lightAdjustment.name' },
            { id: 'enh_color', labelKey: 'enhancements.colorBalance.name' },
            { id: 'enh_sharpen', labelKey: 'enhancements.sharpen.name' },
            { id: 'enh_upscale', labelKey: 'enhancements.upscale.name' },
        ],
    },
];

import { type ComponentType, type Ref, useImperativeHandle, useMemo, useRef } from 'react';
import { List, ListSubheader } from '@mui/material';
import type { ParseKeys } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { TailwindProps } from '@/utils/TailwindProps';
import { AnalyticsEvent, track } from '@/analytics';
import { ExecutionProvider, ImageFormat } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { GetLogsPath } from '@/bindings/gui/services/appservice.ts';
import { RevealInFileManager } from '@/bindings/gui/services/osservice.ts';
import { SettingsItemButton } from '@/features/settings/SettingsItemButton';
import { SettingsItemSelect } from '@/features/settings/SettingsItemSelect';
import { SettingsItemSlider } from '@/features/settings/SettingsItemSlider';
import { SettingsItemSwitch } from '@/features/settings/SettingsItemSwitch';
import { SETTINGS_SECTIONS } from '@/features/settings/sections';
import { useNotify } from '@/hooks/useNotify.ts';
import { LANGUAGE_NAMES, SUPPORTED_LANGUAGES, type SupportedLanguage } from '@/i18n/languages';
import { useSettingsStore } from '@/stores';
import { ENHANCEMENTS, type EnhancementType, modelItems } from '@/utils/enhancement';
import { DEFAULT_QUALITY, MAX_QUALITY, MIN_QUALITY, type QualityFormat } from '@/utils/quality';

export type SettingsListHandle = {
    scrollToSection: (itemId: string) => void;
};

type SettingsListProps = TailwindProps & {
    ref?: Ref<SettingsListHandle>;
};

// Every row takes the anchor id its entry in SETTINGS_SECTIONS defines.
type SettingsRowProps = {
    id?: string;
};

export const SettingsList = ({ className = '', ref }: SettingsListProps) => {
    const { t } = useTranslation();
    const containerRef = useRef<HTMLUListElement>(null);

    useImperativeHandle(ref, () => ({
        scrollToSection: (itemId: string) => {
            const target = containerRef.current?.querySelector(`#${CSS.escape(itemId)}`);
            target?.scrollIntoView({ behavior: 'smooth', block: 'start' });
        },
    }));

    return (
        <List ref={containerRef} className={`${className} py-0 w-full scroll-pt-12 overflow-x-hidden`}>
            {SETTINGS_SECTIONS.map((section) => (
                <div key={section.id}>
                    <ListSubheader id={section.id} className='bg-[#2b2b2b] text-[#f2f2f2]'>
                        {t(section.labelKey)}
                    </ListSubheader>

                    {section.items.map((item) => {
                        // Every enhancement row renders the same component, so they fall through to it rather than
                        // being listed one by one in ROWS — a third copy of the same seven ids.
                        const Row =
                            ROWS[item.id] ??
                            (item.id in ENHANCEMENT_ROWS
                                ? ItemEnhancement
                                : item.id in EXPORT_ROWS
                                  ? ItemExportQuality
                                  : undefined);
                        return Row ? <Row key={item.id} id={item.id} /> : null;
                    })}
                </div>
            ))}
        </List>
    );
};

// Fixed locale rather than the active one, so the picker keeps the same order in every language. See ItemLanguage.
const LANGUAGE_COLLATOR = new Intl.Collator('en');

const ItemLanguage = ({ id }: SettingsRowProps) => {
    const { t } = useTranslation();
    const language = useSettingsStore((state) => state.language);
    const setLanguage = useSettingsStore((state) => state.setLanguage);

    // Endonyms rather than catalog entries, so the list reads the same whatever language the app is currently in —
    // see i18n/languages.ts. Sorted by endonym because that is the only label a user can see; SUPPORTED_LANGUAGES is
    // ordered by catalog id, which would look arbitrary here. Only writes the store; Save is what actually switches
    // the app over.
    const items = useMemo(
        () =>
            SUPPORTED_LANGUAGES.map((lng) => ({ value: lng, label: LANGUAGE_NAMES[lng] })).sort((a, b) =>
                LANGUAGE_COLLATOR.compare(a.label, b.label),
            ),
        [],
    );

    return (
        <SettingsItemSelect
            id={id}
            title={t('settings.app.language.title')}
            description={t('settings.app.language.description')}
            items={items}
            selected={language}
            onSelect={(value) => setLanguage(value as SupportedLanguage)}
        />
    );
};

const ItemLogs = ({ id }: SettingsRowProps) => {
    const { t } = useTranslation();
    const { enqueueSnackbar } = useNotify();

    // Guarded rather than left as a floating promise. This is the button whose whole purpose is to let a user attach
    // their log to a bug report, so when it fails silently - a config directory that can't be resolved, a file manager
    // that won't open - the one path we have for diagnosing anything else is the path that gives no feedback at all.
    const onShowLogs = async () => {
        try {
            const path = await GetLogsPath();
            await RevealInFileManager(path);
        } catch (e) {
            console.error('Failed to reveal the log file', e);
            enqueueSnackbar(t('errors.showLogsFailed'), { variant: 'error' });
        }
    };

    return (
        <SettingsItemButton
            id={id}
            title={t('settings.app.logs.title')}
            description={t('settings.app.logs.description')}
            button={t('settings.app.logs.button')}
            onClick={onShowLogs}
        />
    );
};

const ItemAnalytics = ({ id }: SettingsRowProps) => {
    const { t } = useTranslation();
    const analyticsEnabled = useSettingsStore((state) => state.analyticsEnabled);
    const setAnalyticsEnabled = useSettingsStore((state) => state.setAnalyticsEnabled);

    // No `track` call here: opting out must not itself be reported.
    return (
        <SettingsItemSwitch
            id={id}
            title={t('settings.app.analytics.title')}
            description={t('settings.app.analytics.description')}
            checked={analyticsEnabled}
            onChange={setAnalyticsEnabled}
        />
    );
};

const ItemAiProcessor = ({ id }: SettingsRowProps) => {
    const { t } = useTranslation();
    const processorOptions = useSettingsStore((state) => state.processorOptions);
    const executionProvider = useSettingsStore((state) => state.executionProvider);
    const setExecutionProvider = useSettingsStore((state) => state.setExecutionProvider);

    // The provider's own name is the i18next context, which resolves `description_tensorrt`, `description_cuda` and
    // so on, falling back to the bare `description` for a provider that has no sentence of its own. Each branch is a
    // whole sentence in the catalog rather than a shared prefix plus a clause: a translator has to be free to
    // reorder, merge or re-punctuate the two halves, which string concatenation would prevent.
    //
    // No lock while work is running: loaded models are keyed by processor as well as operation, so switching is an
    // ordinary cache miss. Work already in flight keeps the models it holds and finishes on the old processor; the
    // next run picks up the new one.
    const description = t('settings.performance.aiProcessor.description', {
        context: executionProvider.toLowerCase(),
    });

    // Built here rather than in the store: the store is persisted, so a label written into it would be frozen in
    // whatever language was active at the time. "Auto" is the only real word — the rest are product names.
    const items = useMemo(
        () =>
            processorOptions.map((ep) => ({
                value: ep,
                label: ep === ExecutionProvider.ExecutionProviderAuto ? t('settings.performance.aiProcessor.auto') : ep,
            })),
        [processorOptions, t],
    );

    return (
        <SettingsItemSelect
            id={id}
            title={t('settings.performance.aiProcessor.title')}
            description={description}
            items={items}
            selected={executionProvider}
            onSelect={(value) => {
                setExecutionProvider(value as ExecutionProvider);
                track(AnalyticsEvent.ExecutionProviderChanged, { provider: value });
            }}
        />
    );
};

// The default-model rows. They differ only in which enhancement they belong to and the description they show; the
// models on offer and the row title both come from the ENHANCEMENTS registry, so a new model appears here without an
// edit. Model names are proper nouns and stay out of the catalogs.
type EnhancementRow = {
    type: EnhancementType;
    descriptionKey: ParseKeys;
};

const ENHANCEMENT_ROWS: Record<string, EnhancementRow> = {
    enh_denoise: { type: 'dn', descriptionKey: 'settings.enhancements.denoise.description' },
    enh_face: { type: 'fr', descriptionKey: 'settings.enhancements.faceRecovery.description' },
    enh_colorization: { type: 'cl', descriptionKey: 'settings.enhancements.colorization.description' },
    enh_light: { type: 'la', descriptionKey: 'settings.enhancements.lightAdjustment.description' },
    enh_color: { type: 'cb', descriptionKey: 'settings.enhancements.colorBalance.description' },
    enh_sharpen: { type: 'sh', descriptionKey: 'settings.enhancements.sharpen.description' },
    enh_upscale: { type: 'up', descriptionKey: 'settings.enhancements.upscale.description' },
};

const ItemEnhancement = ({ id }: SettingsRowProps) => {
    const { t } = useTranslation();
    const row = ENHANCEMENT_ROWS[id ?? ''];
    const selected = useSettingsStore((state) => state.models[row.type]);
    const setModel = useSettingsStore((state) => state.setModel);

    return (
        <SettingsItemSelect
            id={id}
            title={t(ENHANCEMENTS[row.type].nameKey)}
            description={t(row.descriptionKey)}
            items={modelItems(row.type)}
            selected={selected}
            onSelect={(value) => {
                setModel(row.type, value);

                // The most interesting signal the app has no data on: each enhancement family ships three models, and
                // nothing so far said which one anyone actually settles on.
                track(AnalyticsEvent.ModelChanged, { type: row.type, model: value });
            }}
        />
    );
};

// The export-quality rows, following the same one-component-many-rows shape as the enhancement rows above: they
// differ only in which format they set, and the format's own name is its title.
const EXPORT_ROWS: Record<string, QualityFormat> = {
    export_avif: ImageFormat.FormatAvif,
    export_heic: ImageFormat.FormatHeic,
    export_jpeg: ImageFormat.FormatJpeg,
    export_webp: ImageFormat.FormatWebp,
};

const EXPORT_DESCRIPTION_KEYS: Record<QualityFormat, ParseKeys> = {
    [ImageFormat.FormatAvif]: 'settings.export.avif.description',
    [ImageFormat.FormatHeic]: 'settings.export.heic.description',
    [ImageFormat.FormatJpeg]: 'settings.export.jpeg.description',
    [ImageFormat.FormatWebp]: 'settings.export.webp.description',
};

const EXPORT_TITLE_KEYS: Record<QualityFormat, ParseKeys> = {
    [ImageFormat.FormatAvif]: 'settings.export.avif.title',
    [ImageFormat.FormatHeic]: 'settings.export.heic.title',
    [ImageFormat.FormatJpeg]: 'settings.export.jpeg.title',
    [ImageFormat.FormatWebp]: 'settings.export.webp.title',
};

// Writes the store as the user drags, unlike the Export dialog's slider, which holds a draft until Export is pressed.
// The difference is deliberate: this dialog already has Save/Cancel around it, and 'quality' is in SNAPSHOT_KEYS, so
// Cancel reverts these the same way it reverts every other setting.
const ItemExportQuality = ({ id }: SettingsRowProps) => {
    const { t } = useTranslation();
    const format = EXPORT_ROWS[id ?? ''];
    const value = useSettingsStore((state) => state.quality[format]);
    const setQuality = useSettingsStore((state) => state.setQuality);

    const marks = useMemo(() => [{ value: DEFAULT_QUALITY[format], label: String(DEFAULT_QUALITY[format]) }], [format]);

    return (
        <SettingsItemSlider
            id={id}
            title={t(EXPORT_TITLE_KEYS[format])}
            description={t(EXPORT_DESCRIPTION_KEYS[format])}
            value={value}
            min={MIN_QUALITY}
            max={MAX_QUALITY}
            step={1}
            marks={marks}
            onChange={(next) => {
                setQuality(format, next);

                // Once per adjustment, not once per drag tick: ValueSlider wires this to MUI's onChangeCommitted.
                track(AnalyticsEvent.QualityChanged, { format, value: next });
            }}
        />
    );
};

// Binds each id in SETTINGS_SECTIONS to the row that renders it. Kept next to the rows rather than in `sections.ts`
// so that structure stays a plain data module.
const ROWS: Record<string, ComponentType<SettingsRowProps>> = {
    app_language: ItemLanguage,
    app_logs: ItemLogs,
    app_analytics: ItemAnalytics,
    perf_processor: ItemAiProcessor,
};

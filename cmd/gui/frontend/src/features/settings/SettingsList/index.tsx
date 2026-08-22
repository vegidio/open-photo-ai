import { type ComponentType, type Ref, useImperativeHandle, useMemo, useRef } from 'react';
import { List, ListSubheader } from '@mui/material';
import type { ParseKeys } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { TailwindProps } from '@/utils/TailwindProps';
import { AnalyticsEvent, track } from '@/analytics';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { GetLogsPath } from '@/bindings/gui/services/appservice.ts';
import { RevealInFileManager } from '@/bindings/gui/services/osservice.ts';
import type { SelectItem } from '@/components/atoms/Select';
import { SettingsItemButton } from '@/features/settings/SettingsItemButton';
import { SettingsItemSelect } from '@/features/settings/SettingsItemSelect';
import { SettingsItemSwitch } from '@/features/settings/SettingsItemSwitch';
import { SETTINGS_SECTIONS } from '@/features/settings/sections';
import { LANGUAGE_NAMES, SUPPORTED_LANGUAGES, type SupportedLanguage } from '@/i18n/languages';
import { useSettingsStore } from '@/stores';
import { ENHANCEMENTS, type EnhancementType } from '@/utils/enhancement';

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
        <List ref={containerRef} className={`${className} py-0 w-full scroll-pt-12`}>
            {SETTINGS_SECTIONS.map((section) => (
                <div key={section.id}>
                    <ListSubheader id={section.id} className='bg-[#2b2b2b] text-[#f2f2f2]'>
                        {t(section.labelKey)}
                    </ListSubheader>

                    {section.items.map((item) => {
                        const Row = ROWS[item.id];
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

    const onShowLogs = async () => {
        const path = await GetLogsPath();
        await RevealInFileManager(path);
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

// The default-model rows. They differ only in which enhancement they belong to, which store field they read and the
// models on offer, so they are described rather than written out six times. Model names are proper nouns and stay
// out of the catalogs; the row's title comes from ENHANCEMENTS so it matches the rest of the app.
type EnhancementRow = {
    type: EnhancementType;
    descriptionKey: ParseKeys;
    field: 'dnModel' | 'frModel' | 'clModel' | 'laModel' | 'cbModel' | 'shModel' | 'upModel';
    setter: 'setDnModel' | 'setFrModel' | 'setClModel' | 'setLaModel' | 'setCbModel' | 'setShModel' | 'setUpModel';
    models: SelectItem[];
};

const ENHANCEMENT_ROWS: Record<string, EnhancementRow> = {
    enh_denoise: {
        type: 'dn',
        descriptionKey: 'settings.enhancements.denoise.description',
        field: 'dnModel',
        setter: 'setDnModel',
        models: [
            { value: 'stockholm', label: 'Stockholm' },
            { value: 'malmo', label: 'Malmö' },
            { value: 'gothenburg', label: 'Gothenburg' },
        ],
    },
    enh_face: {
        type: 'fr',
        descriptionKey: 'settings.enhancements.faceRecovery.description',
        field: 'frModel',
        setter: 'setFrModel',
        models: [
            { value: 'athens', label: 'Athens' },
            { value: 'santorini', label: 'Santorini' },
        ],
    },
    enh_colorization: {
        type: 'cl',
        descriptionKey: 'settings.enhancements.colorization.description',
        field: 'clModel',
        setter: 'setClModel',
        models: [
            { value: 'delhi', label: 'Delhi' },
            { value: 'mumbai', label: 'Mumbai' },
            { value: 'jaipur', label: 'Jaipur' },
        ],
    },
    enh_light: {
        type: 'la',
        descriptionKey: 'settings.enhancements.lightAdjustment.description',
        field: 'laModel',
        setter: 'setLaModel',
        models: [{ value: 'paris', label: 'Paris' }],
    },
    enh_color: {
        type: 'cb',
        descriptionKey: 'settings.enhancements.colorBalance.description',
        field: 'cbModel',
        setter: 'setCbModel',
        models: [{ value: 'rio', label: 'Rio' }],
    },
    enh_sharpen: {
        type: 'sh',
        descriptionKey: 'settings.enhancements.sharpen.description',
        field: 'shModel',
        setter: 'setShModel',
        models: [
            { value: 'moscow', label: 'Moscow' },
            { value: 'petersburg', label: 'St. Petersburg' },
            { value: 'novgorod', label: 'Novgorod' },
        ],
    },
    enh_upscale: {
        type: 'up',
        descriptionKey: 'settings.enhancements.upscale.description',
        field: 'upModel',
        setter: 'setUpModel',
        models: [
            { value: 'tokyo', label: 'Tokyo' },
            { value: 'kyoto', label: 'Kyoto' },
            { value: 'saitama', label: 'Saitama' },
            { value: 'osaka', label: 'Osaka' },
        ],
    },
};

const ItemEnhancement = ({ id }: SettingsRowProps) => {
    const { t } = useTranslation();
    const row = ENHANCEMENT_ROWS[id ?? ''];
    const selected = useSettingsStore((state) => state[row.field]);
    const setModel = useSettingsStore((state) => state[row.setter]);

    return (
        <SettingsItemSelect
            id={id}
            title={t(ENHANCEMENTS[row.type].nameKey)}
            description={t(row.descriptionKey)}
            items={row.models}
            selected={selected}
            onSelect={(value) => setModel(value)}
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
    enh_denoise: ItemEnhancement,
    enh_face: ItemEnhancement,
    enh_colorization: ItemEnhancement,
    enh_light: ItemEnhancement,
    enh_color: ItemEnhancement,
    enh_sharpen: ItemEnhancement,
    enh_upscale: ItemEnhancement,
};

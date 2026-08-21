import { type Ref, useImperativeHandle, useMemo, useRef } from 'react';
import { List, ListSubheader } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { TailwindProps } from '@/utils/TailwindProps';
import { AnalyticsEvent, track } from '@/analytics';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { GetLogsPath } from '@/bindings/gui/services/appservice.ts';
import { RevealInFileManager } from '@/bindings/gui/services/osservice.ts';
import { SettingsItemButton } from '@/features/settings/SettingsItemButton';
import { SettingsItemSelect } from '@/features/settings/SettingsItemSelect';
import { SettingsItemSwitch } from '@/features/settings/SettingsItemSwitch';
import { LANGUAGE_NAMES, SUPPORTED_LANGUAGES, type SupportedLanguage } from '@/i18n/languages';
import { useSettingsStore } from '@/stores';

export type SettingsListHandle = {
    scrollToSection: (itemId: string) => void;
};

// React 19 passes ref through as an ordinary prop, so the imperative handle no longer needs forwardRef to reach it.
type SettingsListProps = TailwindProps & {
    ref?: Ref<SettingsListHandle>;
};

export const SettingsList = ({ className = '', ref }: SettingsListProps) => {
    const { t } = useTranslation();
    const containerRef = useRef<HTMLUListElement>(null);

    const dnModel = useSettingsStore((state) => state.dnModel);
    const setDnModel = useSettingsStore((state) => state.setDnModel);
    const frModel = useSettingsStore((state) => state.frModel);
    const setFrModel = useSettingsStore((state) => state.setFrModel);
    const laModel = useSettingsStore((state) => state.laModel);
    const setLaModel = useSettingsStore((state) => state.setLaModel);
    const cbModel = useSettingsStore((state) => state.cbModel);
    const setCbModel = useSettingsStore((state) => state.setCbModel);
    const upModel = useSettingsStore((state) => state.upModel);
    const setUpModel = useSettingsStore((state) => state.setUpModel);
    const shModel = useSettingsStore((state) => state.shModel);
    const setShModel = useSettingsStore((state) => state.setShModel);

    useImperativeHandle(ref, () => ({
        scrollToSection: (itemId: string) => {
            const target = containerRef.current?.querySelector(`#${CSS.escape(itemId)}`);
            target?.scrollIntoView({ behavior: 'smooth', block: 'start' });
        },
    }));

    return (
        <List ref={containerRef} className={`${className} py-0 w-full scroll-pt-12`}>
            <ListSubheader id='app' className='bg-[#2b2b2b] text-[#f2f2f2]'>
                {t('settings.sections.app')}
            </ListSubheader>

            <ItemLanguage id='app_language' />
            <ItemLogs id='app_logs' />
            <ItemAnalytics id='app_analytics' />

            <ListSubheader id='performance' className='bg-[#2b2b2b] text-[#f2f2f2]'>
                {t('settings.sections.performance')}
            </ListSubheader>

            <ItemAiProcessor id='perf_processor' />

            <ListSubheader id='enhancements' className='bg-[#2b2b2b] text-[#f2f2f2]'>
                {t('settings.sections.enhancements')}
            </ListSubheader>

            <SettingsItemSelect
                id='enh_denoise'
                title={t('settings.enhancements.denoise.title')}
                description={t('settings.enhancements.denoise.description')}
                items={[
                    {
                        value: 'stockholm',
                        label: 'Stockholm',
                    },
                    {
                        value: 'malmo',
                        label: 'Malmö',
                    },
                    {
                        value: 'gothenburg',
                        label: 'Gothenburg',
                    },
                ]}
                selected={dnModel}
                onSelect={(value) => setDnModel(value)}
            />

            <SettingsItemSelect
                id='enh_face'
                title={t('settings.enhancements.faceRecovery.title')}
                description={t('settings.enhancements.faceRecovery.description')}
                items={[
                    {
                        value: 'athens',
                        label: 'Athens',
                    },
                    {
                        value: 'santorini',
                        label: 'Santorini',
                    },
                ]}
                selected={frModel}
                onSelect={(value) => setFrModel(value)}
            />

            <SettingsItemSelect
                id='enh_light'
                title={t('settings.enhancements.lightAdjustment.title')}
                description={t('settings.enhancements.lightAdjustment.description')}
                items={[
                    {
                        value: 'paris',
                        label: 'Paris',
                    },
                ]}
                selected={laModel}
                onSelect={(value) => setLaModel(value)}
            />

            <SettingsItemSelect
                id='enh_color'
                title={t('settings.enhancements.colorBalance.title')}
                description={t('settings.enhancements.colorBalance.description')}
                items={[
                    {
                        value: 'rio',
                        label: 'Rio',
                    },
                ]}
                selected={cbModel}
                onSelect={(value) => setCbModel(value)}
            />

            <SettingsItemSelect
                id='enh_sharpen'
                title={t('settings.enhancements.sharpen.title')}
                description={t('settings.enhancements.sharpen.description')}
                items={[
                    {
                        value: 'moscow',
                        label: 'Moscow',
                    },
                    {
                        value: 'petersburg',
                        label: 'St. Petersburg',
                    },
                    {
                        value: 'novgorod',
                        label: 'Novgorod',
                    },
                ]}
                selected={shModel}
                onSelect={(value) => setShModel(value)}
            />

            <SettingsItemSelect
                id='enh_upscale'
                title={t('settings.enhancements.upscale.title')}
                description={t('settings.enhancements.upscale.description')}
                items={[
                    {
                        value: 'tokyo',
                        label: 'Tokyo',
                    },
                    {
                        value: 'kyoto',
                        label: 'Kyoto',
                    },
                    {
                        value: 'saitama',
                        label: 'Saitama',
                    },
                    {
                        value: 'osaka',
                        label: 'Osaka',
                    },
                ]}
                selected={upModel}
                onSelect={(value) => setUpModel(value)}
            />
        </List>
    );
};

type ItemLanguageProps = {
    id?: string;
};

const ItemLanguage = ({ id }: ItemLanguageProps) => {
    const { t } = useTranslation();
    const language = useSettingsStore((state) => state.language);
    const setLanguage = useSettingsStore((state) => state.setLanguage);

    // Endonyms rather than catalog entries, so the list reads the same whatever language the app is currently in —
    // see i18n/languages.ts. Only writes the store; Save is what actually switches the app over.
    const items = useMemo(() => SUPPORTED_LANGUAGES.map((lng) => ({ value: lng, label: LANGUAGE_NAMES[lng] })), []);

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

type ItemLogsProps = {
    id?: string;
};

const ItemLogs = ({ id }: ItemLogsProps) => {
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

type ItemAnalyticsProps = {
    id?: string;
};

const ItemAnalytics = ({ id }: ItemAnalyticsProps) => {
    const { t } = useTranslation();
    const analyticsEnabled = useSettingsStore((state) => state.analyticsEnabled);
    const setAnalyticsEnabled = useSettingsStore((state) => state.setAnalyticsEnabled);

    // Only writes the store; a subscription in main.tsx mirrors the flag into Firebase, so the store stays authoritative.
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

type ItemAiProcessorProps = {
    id?: string;
};

const ItemAiProcessor = ({ id }: ItemAiProcessorProps) => {
    const { t } = useTranslation();
    const processorOptions = useSettingsStore((state) => state.processorOptions);
    const executionProvider = useSettingsStore((state) => state.executionProvider);
    const setExecutionProvider = useSettingsStore((state) => state.setExecutionProvider);

    // Each branch is a whole sentence in the catalog rather than a shared prefix plus a clause: a translator has to be
    // free to reorder, merge or re-punctuate the two halves, which string concatenation would prevent.
    //
    // No lock while work is running: loaded models are keyed by processor as well as operation, so switching is an
    // ordinary cache miss. Work already in flight keeps the models it holds and finishes on the old processor; the
    // next run picks up the new one.
    const description = useMemo(() => {
        switch (executionProvider) {
            case ExecutionProvider.ExecutionProviderAuto:
                return t('settings.performance.aiProcessor.description_auto');
            case ExecutionProvider.ExecutionProviderTensorRT:
                return t('settings.performance.aiProcessor.description_tensorrt');
            case ExecutionProvider.ExecutionProviderCUDA:
                return t('settings.performance.aiProcessor.description_cuda');
            case ExecutionProvider.ExecutionProviderDirectML:
                return t('settings.performance.aiProcessor.description_directml');
            case ExecutionProvider.ExecutionProviderCoreML:
                return t('settings.performance.aiProcessor.description_coreml');
            case ExecutionProvider.ExecutionProviderCPU:
                return t('settings.performance.aiProcessor.description_cpu');
            default:
                return t('settings.performance.aiProcessor.description');
        }
    }, [executionProvider, t]);

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

import { CancellablePromise } from '@wailsio/runtime';
import type { ParseKeys } from 'i18next';
import type { File } from '@/bindings/gui/types';
import type { IconName } from '@/components/atoms/Icon';
import { ModelType } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { SuggestEnhancements } from '@/bindings/gui/services/imageservice.ts';
import {
    Athens,
    Delhi,
    Gothenburg,
    Jaipur,
    Kyoto,
    Malmo,
    Moscow,
    Mumbai,
    Novgorod,
    type Operation,
    Osaka,
    Paris,
    Petersburg,
    Rio,
    Saitama,
    Santorini,
    Stockholm,
    Tokyo,
} from '@/operations';

export type EnhancementType = 'dn' | 'fr' | 'cl' | 'la' | 'cb' | 'sh' | 'up';

// The user's chosen default model for each enhancement, as held by the settings store.
export type ModelChoices = Record<EnhancementType, string>;

// The first two letters of an operation ID are its enhancement type, e.g. `dn`, `fr`, `up`.
//
// The prefix is narrowed rather than asserted: an id from the backend, a persisted setting or a stale cache entry can
// carry any two characters, and casting made `ENHANCEMENTS[type]` look total when it is not. Returning undefined is
// what the call sites already assume - they all guard on the lookup - so this makes their guards type-driven.
//
// The analytics call sites report the undefined case as type='unknown'. That means an operation id was rehydrated
// from a previous version's persisted state and no longer maps to anything, so read a spike there as a migration
// problem rather than as a fault in the enhancement pipeline.
export const getEnhancementType = (opId: string): EnhancementType | undefined => {
    const prefix = opId.slice(0, 2);
    return ENHANCEMENT_ORDER.includes(prefix as EnhancementType) ? (prefix as EnhancementType) : undefined;
};

// The scale an operation list will produce, as a factor of the source dimensions. Fractional scales are supported, so
// this parses as a float: reading it with parseInt silently truncated 1.5x to 1x.
export const upscaleFactor = (operations: Operation[]): number => {
    const scale = operations.find((op) => op.id.startsWith('up'))?.options?.scale;
    const parsed = parseFloat(scale ?? '1');

    return Number.isFinite(parsed) && parsed > 0 ? parsed : 1;
};

// The order enhancements are presented in and applied in. It is a single list because the two must agree: the add
// menu offering them in one order while the pipeline ran them in another would be a silent inconsistency.
export const ENHANCEMENT_ORDER: readonly EnhancementType[] = ['dn', 'fr', 'cl', 'la', 'cb', 'sh', 'up'];

// The two quality tiers the model selector offers, as the user sees them. `md` is the catalog key behind the label
// "SD"; renaming it would touch all thirteen catalogs, so the key and the string differ on purpose.
export type QualityTier = 'hd' | 'md';

// Which precision backs each tier for one model.
type Precisions = Record<QualityTier, string>;

// The convention every model but Osaka follows.
const DEFAULT_PRECISIONS: Precisions = { hd: 'fp32', md: 'fp16' };

// One model a user can pick for an enhancement. Model names are proper nouns and stay out of the catalogs; only the
// description is translated.
type ModelInfo = {
    // The codename, as it appears in the operation ID: `delhi` in `cl_delhi_fp32`.
    id: string;

    label: string;

    descriptionKey: ParseKeys;

    // The precision behind each quality tier, when it is not the app-wide convention of fp32 for HD and fp16 for SD.
    //
    // Osaka is the only model that overrides it, and it needs to: no fp32 build of it was ever published, and its
    // diffusion transformer is now also published quantised to int8. So its two tiers are fp16 and int8 — the tier a
    // user picks is a statement about quality, not about a number of bits, and the two stopped agreeing here.
    precisions?: Precisions;

    // Builds the operation. `amount` is the intensity or scale where the enhancement has one, and ignored otherwise.
    build: (precision: string, amount: number) => Operation;
};

type EnhancementInfo = {
    // The catalog key for the enhancement's full name, as shown in lists and menus.
    nameKey: ParseKeys;

    // A separate, shorter name for the progress badge. Several languages need one that the full name is too long
    // for -- ru shortens "Восстановление лиц" to "Лица" -- so this is its own key rather than a truncation.
    shortNameKey: ParseKeys;

    // The catalog key for the secondary line under the enhancement's name, which differs by what the enhancement has
    // to report: an intensity, a scale, a face count, or nothing but the model. Declaring it here rather than
    // switching on the type at the call site means a new enhancement cannot be added without choosing one.
    infoKey: ParseKeys;

    icon: IconName;

    // The intensity or scale a freshly added enhancement gets. Ignored by enhancements that have neither.
    defaultAmount: number;

    // The model used when the user has expressed no preference, and the fallback for a stored preference
    // naming a model that no longer exists. Stated rather than implied by list position, because
    // presentation order and the sensible default are not the same thing: upscale lists Tokyo first but
    // defaults to Kyoto.
    defaultModel: string;

    // Every model on offer, in the order they are presented. This is the single catalog: the settings picker, the
    // options popovers and the operation builders all read it, so a model is added or renamed in exactly one place.
    models: ModelInfo[];
};

// The one place that knows what the two-letter operation prefixes mean, and which models sit behind each of them.
// Every consumer -- the enhancement list, the add menu, the options popovers, the settings picker, the progress
// badge -- reads its labels, icon and models from here, so adding an enhancement or a model is one entry rather than
// a matching branch in four separate switches.
export const ENHANCEMENTS: Record<EnhancementType, EnhancementInfo> = {
    dn: {
        nameKey: 'enhancements.denoise.name',
        shortNameKey: 'preview.progress.denoise',
        infoKey: 'enhancements.info',
        icon: 'denoise',
        defaultAmount: 1,
        defaultModel: 'stockholm',
        models: [
            {
                id: 'stockholm',
                label: 'Stockholm',
                descriptionKey: 'enhancements.denoise.models.stockholm',
                build: (precision, amount) => new Stockholm(amount, precision),
            },
            {
                id: 'gothenburg',
                label: 'Gothenburg',
                descriptionKey: 'enhancements.denoise.models.gothenburg',
                build: (precision, amount) => new Gothenburg(amount, precision),
            },
            {
                id: 'malmo',
                label: 'Malmö',
                descriptionKey: 'enhancements.denoise.models.malmo',
                build: (precision, amount) => new Malmo(amount, precision),
            },
        ],
    },
    fr: {
        nameKey: 'enhancements.faceRecovery.name',
        shortNameKey: 'preview.progress.faceRecovery',
        infoKey: 'enhancements.infoFaces',
        icon: 'face_recovery',
        defaultAmount: 0,
        defaultModel: 'athens',
        models: [
            {
                id: 'athens',
                label: 'Athens',
                descriptionKey: 'enhancements.faceRecovery.models.athens',
                build: (precision) => new Athens(precision),
            },
            {
                id: 'santorini',
                label: 'Santorini',
                descriptionKey: 'enhancements.faceRecovery.models.santorini',
                build: (precision) => new Santorini(precision),
            },
        ],
    },
    cl: {
        nameKey: 'enhancements.colorization.name',
        shortNameKey: 'preview.progress.colorization',
        infoKey: 'enhancements.infoModel',
        icon: 'colorization',
        defaultAmount: 0,
        defaultModel: 'delhi',
        models: [
            {
                id: 'delhi',
                label: 'Delhi',
                descriptionKey: 'enhancements.colorization.models.delhi',
                build: (precision) => new Delhi(precision),
            },
            {
                id: 'mumbai',
                label: 'Mumbai',
                descriptionKey: 'enhancements.colorization.models.mumbai',
                build: (precision) => new Mumbai(precision),
            },
            {
                id: 'jaipur',
                label: 'Jaipur',
                descriptionKey: 'enhancements.colorization.models.jaipur',
                build: (precision) => new Jaipur(precision),
            },
        ],
    },
    la: {
        nameKey: 'enhancements.lightAdjustment.name',
        shortNameKey: 'preview.progress.lightAdjustment',
        infoKey: 'enhancements.info',
        icon: 'light_adjustment',
        defaultAmount: 0.5,
        defaultModel: 'paris',
        models: [
            {
                id: 'paris',
                label: 'Paris',
                descriptionKey: 'enhancements.lightAdjustment.models.paris',
                build: (precision, amount) => new Paris(amount, precision),
            },
        ],
    },
    cb: {
        nameKey: 'enhancements.colorBalance.name',
        shortNameKey: 'preview.progress.colorBalance',
        infoKey: 'enhancements.info',
        icon: 'color_balance',
        defaultAmount: 0.5,
        defaultModel: 'rio',
        models: [
            {
                id: 'rio',
                label: 'Rio',
                descriptionKey: 'enhancements.colorBalance.models.rio',
                build: (precision, amount) => new Rio(amount, precision),
            },
        ],
    },
    sh: {
        nameKey: 'enhancements.sharpen.name',
        shortNameKey: 'preview.progress.sharpen',
        infoKey: 'enhancements.info',
        icon: 'sharpen',
        defaultAmount: 1,
        defaultModel: 'moscow',
        models: [
            {
                id: 'moscow',
                label: 'Moscow',
                descriptionKey: 'enhancements.sharpen.models.moscow',
                build: (precision, amount) => new Moscow(amount, precision),
            },
            {
                id: 'petersburg',
                label: 'St. Petersburg',
                descriptionKey: 'enhancements.sharpen.models.petersburg',
                build: (precision, amount) => new Petersburg(amount, precision),
            },
            {
                id: 'novgorod',
                label: 'Novgorod',
                descriptionKey: 'enhancements.sharpen.models.novgorod',
                build: (precision, amount) => new Novgorod(amount, precision),
            },
        ],
    },
    up: {
        nameKey: 'enhancements.upscale.name',
        shortNameKey: 'preview.progress.upscale',
        infoKey: 'enhancements.infoScale',
        icon: 'upscale',
        defaultAmount: 1,
        defaultModel: 'kyoto',
        models: [
            {
                id: 'tokyo',
                label: 'Tokyo',
                descriptionKey: 'enhancements.upscale.models.tokyo',
                build: (precision, amount) => new Tokyo(amount, precision),
            },
            {
                id: 'kyoto',
                label: 'Kyoto',
                descriptionKey: 'enhancements.upscale.models.kyoto',
                build: (precision, amount) => new Kyoto(amount, precision),
            },
            {
                id: 'saitama',
                label: 'Saitama',
                descriptionKey: 'enhancements.upscale.models.saitama',
                build: (precision, amount) => new Saitama(amount, precision),
            },
            {
                id: 'osaka',
                label: 'Osaka',
                descriptionKey: 'enhancements.upscale.models.osaka',
                precisions: { hd: 'fp16', md: 'int8' },
                build: (precision, amount) => new Osaka(amount, precision),
            },
        ],
    },
};

// The model each enhancement starts on, as the settings store's initial value. Derived from the registry so the
// defaults are stated once.
export const DEFAULT_MODELS: ModelChoices = Object.fromEntries(
    ENHANCEMENT_ORDER.map((type) => [type, ENHANCEMENTS[type].defaultModel]),
) as ModelChoices;

// The models an enhancement offers, as plain select items for the settings picker.
export const modelItems = (type: EnhancementType) =>
    ENHANCEMENTS[type].models.map(({ id, label }) => ({ value: id, label }));

/**
 * The precision behind each quality tier for one model, defaulting to the app-wide convention.
 *
 * An unknown model falls back to the convention rather than throwing: the id can come from a persisted setting or a
 * cached operation written by an older build, the same rehydration case `getEnhancementType` guards against.
 */
export const modelPrecisions = (type: EnhancementType, model: string): Precisions =>
    ENHANCEMENTS[type].models.find((m) => m.id === model)?.precisions ?? DEFAULT_PRECISIONS;

/**
 * The tier a precision represents, for a given model. The inverse of `modelPrecisions`, and the only thing that should
 * decide whether a label reads "HD" or "SD".
 *
 * It takes the model because the answer depends on it — Osaka's fp16 build is its HD tier, every other model's fp16 is
 * its SD one — and that is exactly what makes a bare `precision === 'fp32'` test wrong now.
 */
export const qualityTier = (type: EnhancementType, model: string, precision: string): QualityTier =>
    modelPrecisions(type, model).hd === precision ? 'hd' : 'md';

/**
 * Builds the operation for an enhancement at the user's chosen model, at the precision that model defaults to.
 *
 * The fallback chain is one rule, not two: an unknown model falls back to the enhancement's default, and a default
 * that is itself unknown falls back to its first. Both arms are what a stored setting from an older build, or a
 * renamed model, lands on.
 */
export const getOp = (type: EnhancementType, model: string, amount?: number): Operation => {
    const { models, defaultModel, defaultAmount } = ENHANCEMENTS[type];
    const chosen = models.find((m) => m.id === model) ?? models.find((m) => m.id === defaultModel) ?? models[0];

    return chosen.build(modelPrecisions(type, chosen.id).hd, amount ?? defaultAmount);
};

/**
 * Builds the operation for a `<model>_<precision>` selection, the value the options popovers' model selector carries.
 *
 * Returns `undefined` for a selection that names no known model, which is what `useOptionEnhancement` treats as
 * "leave the enhancement alone".
 */
export const buildSelection = (type: EnhancementType, selection: string, amount?: number): Operation | undefined => {
    const [id, precision] = selection.split('_');
    const chosen = ENHANCEMENTS[type].models.find((m) => m.id === id);

    if (!chosen || !precision) return undefined;

    return chosen.build(precision, amount ?? ENHANCEMENTS[type].defaultAmount);
};

export const suggestEnhancement = (file: File, models: ModelChoices) => {
    let p: CancellablePromise<ModelType[]>;

    return new CancellablePromise<Operation[]>(
        async (resolve, reject) => {
            p = SuggestEnhancements(file.Path);

            try {
                const opIds = await p;
                resolve(modelTypesToOps(opIds, file, models));
            } catch (e) {
                reject(e);
            }
        },
        () => p.cancel(),
    );
};

// The scale a freshly added upscale gets, chosen so the result lands in a sensible range for the source resolution.
export const defaultUpscaleScale = (file: File | undefined): number => {
    const [width, height] = file?.Dimensions ?? [0, 0];
    const mp = width * height;

    return mp <= 1_048_576 ? 4 : mp <= 4_194_304 ? 2 : 1;
};

const modelTypesToOps = (modelTypes: ModelType[], file: File, models: ModelChoices): Operation[] => {
    const operations: Operation[] = [];

    for (const modelType of modelTypes) {
        switch (modelType) {
            case ModelType.ModelTypeFaceRecovery:
                operations.push(getOp('fr', models.fr));
                break;

            case ModelType.ModelTypeLightAdjustment:
                operations.push(getOp('la', models.la));
                break;

            case ModelType.ModelTypeColorBalance:
                operations.push(getOp('cb', models.cb));
                break;

            case ModelType.ModelTypeUpscale:
                operations.push(getOp('up', models.up, defaultUpscaleScale(file)));
                break;
        }
    }

    return operations;
};

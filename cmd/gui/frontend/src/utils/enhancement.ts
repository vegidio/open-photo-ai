import { CancellablePromise } from '@wailsio/runtime';
import type { File } from '@/bindings/gui/types';
import { ModelType } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { SuggestEnhancements } from '@/bindings/gui/services/imageservice.ts';
import {
    Athens,
    Gothenburg,
    Kyoto,
    Malmo,
    Moscow,
    Novgorod,
    type Operation,
    Paris,
    Petersburg,
    Rio,
    Saitama,
    Santorini,
    Stockholm,
    Tokyo,
} from '@/operations';

export type ModelChoices = {
    dn: string;
    fr: string;
    la: string;
    cb: string;
    up: string;
    sh: string;
};

// The first two letters of an operation ID are its enhancement type, e.g. `dn`, `fr`, `up`.
export const getEnhancementType = (opId: string): string => opId.slice(0, 2);

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

export const getDnOp = (model: string) => {
    switch (model) {
        case 'malmo':
            return new Malmo(1, 'fp32');
        case 'gothenburg':
            return new Gothenburg(1, 'fp32');
        default:
            return new Stockholm(1, 'fp32');
    }
};

export const getShOp = (model: string) => {
    switch (model) {
        case 'novgorod':
            return new Novgorod(1, 'fp32');
        case 'petersburg':
            return new Petersburg(1, 'fp32');
        default:
            return new Moscow(1, 'fp32');
    }
};

export const getFrOp = (model: string) => {
    switch (model) {
        case 'santorini':
            return new Santorini('fp32');
        default:
            return new Athens('fp32');
    }
};

export const getLaOp = (model: string) => {
    switch (model) {
        default:
            return new Paris(0.5, 'fp32');
    }
};

export const getCbOp = (model: string) => {
    switch (model) {
        default:
            return new Rio(0.5, 'fp32');
    }
};

export const getUpOp = (model: string, scale: number) => {
    switch (model) {
        case 'tokyo':
            return new Tokyo(scale, 'fp32');
        case 'saitama':
            return new Saitama(scale, 'fp32');
        default:
            return new Kyoto(scale, 'fp32');
    }
};

const modelTypesToOps = (modelTypes: ModelType[], file: File, models: ModelChoices): Operation[] => {
    const operations: Operation[] = [];

    for (const modelType of modelTypes) {
        switch (modelType) {
            case ModelType.ModelTypeFaceRecovery:
                operations.push(getFrOp(models.fr));
                break;

            case ModelType.ModelTypeLightAdjustment:
                operations.push(getLaOp(models.la));
                break;

            case ModelType.ModelTypeColorBalance:
                operations.push(getCbOp(models.cb));
                break;

            case ModelType.ModelTypeUpscale: {
                const [width, height] = file.Dimensions;
                const mp = width * height;
                const scale = mp <= 1_048_576 ? 4 : mp <= 4_194_304 ? 2 : 1;

                operations.push(getUpOp(models.up, scale));
                break;
            }
        }
    }

    return operations;
};

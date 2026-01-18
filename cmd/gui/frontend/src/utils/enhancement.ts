import { CancellablePromise } from '@wailsio/runtime';
import { SuggestEnhancements } from '@/bindings/gui/services/imageservice.ts';
import { Athens, Kyoto, type Operation, Saitama, Santorini, Tokyo } from '@/operations';

export const suggestEnhancement = (filePath: string) => {
    let p: CancellablePromise<string[]>;

    return new CancellablePromise<Operation[]>(
        async (resolve, reject) => {
            p = SuggestEnhancements(filePath);

            try {
                const opIds = await p;
                resolve(idsToOperations(opIds));
            } catch (e) {
                reject(e);
            }
        },
        () => p.cancel(),
    );
};

const idsToOperations = (opIds: string[]): Operation[] => {
    const operations: Operation[] = [];

    for (const opId of opIds) {
        const values = opId.split('_');
        const name = values[1];

        switch (name) {
            // Face Recovery
            case 'athens':
                operations.push(new Athens(values[2]));
                break;

            case 'santorini':
                operations.push(new Santorini(values[2]));
                break;

            // Upscale
            case 'tokyo': {
                const scale = parseFloat(values[2].replace('x', ''));
                operations.push(new Tokyo(scale, values[3]));
                break;
            }

            case 'kyoto': {
                const scale = parseFloat(values[2].replace('x', ''));
                operations.push(new Kyoto(scale, values[3]));
                break;
            }

            case 'saitama': {
                const scale = parseFloat(values[2].replace('x', ''));
                operations.push(new Saitama(scale, values[3]));
                break;
            }
        }
    }

    return operations;
};

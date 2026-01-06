import { SuggestEnhancements } from '../../bindings/gui/services/imageservice.ts';
import { Athens, Kyoto, type Operation, Saitama, Santorini, Tokyo } from '@/operations';

export const suggestEnhancement = async (filePath: string) => {
    const opIds = await SuggestEnhancements(filePath);
    return idsToOperations(opIds);
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

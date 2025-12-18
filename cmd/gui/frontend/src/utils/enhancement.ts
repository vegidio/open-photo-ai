import { SuggestEnhancements } from '../../bindings/gui/services/imageservice.ts';
import { Athens, Kyoto, type Operation, Santorini, Tokyo } from '@/operations';

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
                const scale = parseInt(values[2].replace('x', ''), 10);
                operations.push(new Tokyo(scale, values[3]));
                break;
            }

            case 'kyoto': {
                const mode = values[2] as 'general' | 'cartoon';
                const scale = parseInt(values[3].replace('x', ''), 10);
                operations.push(new Kyoto(mode, scale, values[4]));
                break;
            }
        }
    }

    return operations;
};

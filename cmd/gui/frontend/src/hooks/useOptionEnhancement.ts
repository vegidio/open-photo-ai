import { useState } from 'react';
import type { Operation } from '@/operations';
import { useCurrentFile } from './useCurrentFile.ts';
import { useFileOperations } from './useFileOperations.ts';
import { useEnhancementStore } from '@/stores';

type OptionEnhancement = {
    model: string;
    amount: string;
    onModelChange: (value: string) => void;
    onAmountChange: (value: string) => void;
};

/**
 * Shared wiring for the `Options…` popovers (sharpen, denoise, upscale, light adjustment, color balance). It holds the
 * local `model`/`amount` input state and writes the enhancement back to the store only when the user actually changes a
 * value — never on mount, so opening a popover never cancels in-flight inference.
 *
 * @param prefix the operation id prefix used to find the current op (e.g. `sh`, `dn`, `up`).
 * @param initialAmount seeds the `amount` input from the current op (intensity or scale).
 * @param build turns the current `model`/`amount` into an `Operation`; return `undefined` to skip the write
 * (e.g. transient/empty input or an unknown model).
 */
export const useOptionEnhancement = (
    prefix: string,
    initialAmount: (op: Operation | undefined) => string,
    build: (model: string, amount: string) => Operation | undefined,
): OptionEnhancement => {
    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith(prefix));
    const [model, setModel] = useState(`${currentOp?.options.name}_${currentOp?.options.precision}`);
    const [amount, setAmount] = useState(initialAmount(currentOp));

    const apply = (nextModel: string, nextAmount: string) => {
        if (!file) return;
        const operation = build(nextModel, nextAmount);
        if (operation) replaceEnhancement(file, operation);
    };

    const onModelChange = (value: string) => {
        setModel(value);
        apply(value, amount);
    };

    const onAmountChange = (value: string) => {
        setAmount(value);
        apply(model, value);
    };

    return { model, amount, onModelChange, onAmountChange };
};

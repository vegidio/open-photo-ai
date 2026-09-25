import { useTranslation } from "react-i18next";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import type { SupportedProviders } from "@/ipc/setup";
import { cn } from "@/lib/utils";
import { PROCESSORS, type Processor } from "@/stores/settings";
import { useSetupStore } from "@/stores/setup";
import { useSettingsDraft } from "./draft";
import { RADIO_FILL } from "./rows.tsx";

// One of the two things this surface adds of its own; the other is the order `PROCESSORS` pins. The
// frontend still carries no list of provider names of its own in the sense that matters: **which**
// providers exist is the report's answer, and a provider the report names that this map does not is
// one that is not offered - exactly as `ipc/setup.ts` documents for the `#[non_exhaustive]` struct
// behind it. What this adds is how four names the frontend already spells are shown, which is
// presentation.
/** The product names, which are not translatable. */
const PROCESSOR_LABELS: Record<Exclude<Processor, "auto">, string> = {
    tensorrt: "TensorRT",
    cuda: "CUDA",
    coreml: "CoreML",
    cpu: "CPU",
};

// Walks `PROCESSORS` and asks the report about each, rather than walking the report: that is what
// fixes the order without a sort, and what makes an unknown fifth field on the report a provider that
// is simply not offered until someone teaches this file the name. It takes the report rather than
// reading the store, so it is answerable without a mounted setup.
/**
 * What this machine is offered: the automatic choice, then the providers the report says it has, in
 * {@link PROCESSORS}' order. A provider this file has no name for is not offered, so a machine with no
 * supported adapter is offered the automatic choice and its CPU, and nothing else.
 */
export const offeredProcessors = (providers: SupportedProviders | undefined): Processor[] =>
    PROCESSORS.filter((processor) => processor === "auto" || providers?.[processor]);

// Resolved where it is drawn rather than repaired on rehydrate, for the reason `useSettingsStore`'s
// docs give. A driver that was uninstalled, or a settings file carried to another machine, therefore
// reads as the automatic choice - which is the one the requirement names as the fallback, and the
// only one that is always honourable.
/** The stored processor, or the automatic choice where this machine no longer offers it. */
const resolveProcessor = (stored: Processor, offered: Processor[]): Processor =>
    offered.includes(stored) ? stored : "auto";

/**
 * Screen 16b: which processor the models run on, as one card per processor this machine offers - so
 * what picking each one means can be read before choosing, rather than only after.
 *
 * **Choosing one records the choice and nothing more.**
 */
export const PerformancePage = () => {
    const { t } = useTranslation();
    const providers = useSetupStore((state) => state.providers);
    const { values, update } = useSettingsDraft();

    // Nothing here rebuilds a session or releases memory: every run is handed the processor in force,
    // and `useEnhancementRun` starts a new one when Save changes it.

    const offered = offeredProcessors(providers);
    const selected = resolveProcessor(values.processor, offered);

    return (
        <RadioGroup
            className="flex flex-col gap-2.5"
            value={selected}
            // A group announces its options and not what the choice is *about*, and this page has no
            // row label to borrow - the page title says "Performance", not what is being chosen.
            aria-label={t("settings.performance.aiProcessor.title")}
            // Narrowed by the options rather than cast: these are the only values this rendered.
            onValueChange={(next) => {
                const chosen = offered.find((processor) => processor === next);
                if (chosen) update({ processor: chosen });
            }}
        >
            {offered.map((processor) => {
                const id = `settings_processor_${processor}`;
                const chosen = processor === selected;

                return (
                    /*
                     * The whole card is the radio's label, so a click anywhere on it chooses it. The
                     * radio names itself by the name span alone and is described by the sentence: as a
                     * label, the card would otherwise make the badge and the sentence part of its name.
                     */
                    <label
                        key={processor}
                        htmlFor={id}
                        data-slot="processor-card"
                        className={cn(
                            "flex cursor-pointer items-start gap-3 rounded-lg border px-4 py-3.5 transition-colors",
                            chosen ? "border-primary bg-primary/6" : "border-border",
                        )}
                    >
                        <RadioGroupItem
                            id={id}
                            value={processor}
                            className={cn(RADIO_FILL, "mt-0.5")}
                            aria-labelledby={`${id}_name`}
                            aria-describedby={`${id}_description`}
                        />

                        <div className="flex min-w-0 flex-col gap-1">
                            <span className="flex items-center gap-2 text-sm">
                                <span id={`${id}_name`}>
                                    {processor === "auto"
                                        ? t("settings.performance.aiProcessor.auto")
                                        : PROCESSOR_LABELS[processor]}
                                </span>

                                {/*
                                 * `#7cc7ff` is the design's own and has no token: a lighter step of the
                                 * accent that stays legible on the accent's own 14% tint.
                                 */}
                                {processor === "auto" && (
                                    <span className="flex h-5 items-center rounded-full bg-primary/14 px-1.75 font-medium text-[#7cc7ff] text-[11px]">
                                        {t("settings.performance.recommended")}
                                    </span>
                                )}
                            </span>

                            <p
                                id={`${id}_description`}
                                className="m-0 text-pretty text-foreground-dim text-xs leading-[1.55]"
                            >
                                {t(`settings.performance.processor.${processor}.description`)}
                            </p>
                        </div>
                    </label>
                );
            })}
        </RadioGroup>
    );
};

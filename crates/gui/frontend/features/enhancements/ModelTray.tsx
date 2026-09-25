import type { ParseKeys } from "i18next";
import { Info } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { FamilyEntry } from "@/ipc/catalogue";
import { type Enhancement, modelChoice, modelLabel } from "@/lib/enhancements";
import { cn } from "@/lib/utils";

type ModelTrayProps = {
    /** The enhancement whose models these are, which is where their descriptions are keyed from. */
    enhancement: Enhancement;
    /** What the catalogue publishes for that family. */
    entry: FamilyEntry | undefined;
    /** The `<codename>_<precision>` the enhancement currently carries. */
    value: string;
    /** Called with another option's value when the user picks one. */
    onChange: (value: string) => void;
};

/**
 * Where a model's description lives in the catalogues, derived from the enhancement's own name key.
 *
 * Composed at runtime, so it cannot be checked against the catalogue's shape and may resolve to
 * nothing.
 */
const descriptionKey = (enhancement: Enhancement, codename: string) =>
    // `enhancements.upscale.name` and `enhancements.upscale.models.kyoto` are siblings in every one of
    // the thirteen catalogues, so the family's segment is already spelled once - in `nameKey` - and
    // deriving from it is what keeps a second table of seven camel-cased family names out of this
    // file. The cast is the price.
    enhancement.nameKey.replace(/\.name$/, `.models.${codename}`) as ParseKeys;

/**
 * Every model this enhancement publishes, at every precision it publishes them at, with the one in
 * use shown as chosen.
 *
 * **Two columns, a row per model** - which is what reading the options positionally gives for free:
 * a model published at two precisions contributes two consecutive options, so the grid lays each
 * model's pair out as one row.
 *
 * **The description sits on a model's first precision and the second is left bare**, shown in a
 * tooltip off its marker rather than off the whole option.
 */
export const ModelTray = ({ enhancement, entry, value, onChange }: ModelTrayProps) => {
    const { t } = useTranslation();

    const { options } = modelChoice(entry, value);

    return (
        <div className="flex flex-col gap-2">
            <span className="text-[13px] text-foreground">{t("enhancements.modelSelector")}</span>

            <ToggleGroup
                type="single"
                value={value}
                onValueChange={(next: string) => {
                    // Radix reports an empty string when the option already chosen is pressed again.
                    // An enhancement always runs *some* model, so that deselection is ignored rather
                    // than written as "no model".
                    if (next) onChange(next);
                }}
                spacing={1}
                className="grid w-full grid-cols-2 rounded-lg bg-background p-1"
                data-slot="model-tray"
            >
                {options.map((option) => {
                    /*
                     * The marker goes on the first option of each model, which is what the design
                     * draws (screen 11) and what the reference does: the explanation is about the
                     * *model*, and repeating it would put two markers on every row. The option carries
                     * which one is first - the settings rows draw their separator off the same field,
                     * so the two cannot disagree about where one model ends.
                     *
                     * An empty default rather than the key itself, which is what an absent catalogue
                     * entry would otherwise draw: a model the library publishes and the catalogues
                     * have no sentence for gets no marker instead of a marker naming a missing key.
                     */
                    const description = option.first
                        ? t(descriptionKey(enhancement, option.codename), { defaultValue: "" })
                        : "";

                    return (
                        <ToggleGroupItem
                            key={option.value}
                            value={option.value}
                            /*
                             * The selection is painted off `aria-checked` rather than the generated
                             * `data-[state=on]`, in `bg-input` and with the `aria-checked:hover:*`
                             * pair, for the reasons `DrawerHeader`'s preview-mode group gives.
                             *
                             * Hovering fills the option rather than just brightening its label, with
                             * the *selected* colour at 40% rather than a palette tier of its own. Over
                             * the tray's `bg-background` that lands near #1f1f23 - a clear step up
                             * from the tray, and unmistakably below the chosen pill's full-strength
                             * #3f3f46. `bg-accent`, the obvious tier, is #27272a and sits close
                             * enough to the chosen pill in this ramp that pointing at an option reads
                             * as having picked it. Deriving the hover from `--input` also keeps the
                             * two in step if the design moves the selected surface. The generated
                             * `hover:text-muted-foreground` is overridden along with the fill, since a
                             * surface that lifts under a label that stays dim reads as disabled.
                             *
                             * Only a model with a marker gets the left padding: the icon is absolutely
                             * positioned, so the padding exists purely to keep the label clear of it,
                             * and on every option it would push the precisions that have no marker -
                             * the whole right-hand column - off their own centre. It follows
                             * `description`, the same condition the marker itself is drawn under, so
                             * the two can never disagree.
                             */
                            className={cn(
                                "relative h-auto min-h-8.5 w-auto justify-center rounded-md px-1.5 py-1 text-[13px] text-muted-foreground hover:bg-input/40 hover:text-foreground aria-checked:bg-input aria-checked:text-foreground aria-checked:hover:bg-input",
                                description && "pl-[18px]",
                            )}
                        >
                            {description && (
                                <Tooltip>
                                    {/*
                                     * Off the marker, not off the pill: the icon is what says an
                                     * explanation exists, so it is what has to be pointed at to read
                                     * it. Triggering from the whole option would pop a panel over the
                                     * tray every time the pointer crossed a model on its way to
                                     * another.
                                     *
                                     * A span rather than the icon itself, so the hit area is the
                                     * marker's own box and stays put when the tooltip opens.
                                     */}
                                    <TooltipTrigger asChild>
                                        <span className="absolute top-[5px] left-[5px] flex size-[13px] items-center justify-center">
                                            <Info className="size-[13px] text-warning" />
                                        </span>
                                    </TooltipTrigger>

                                    {/*
                                     * To the left, as the design places it: the panel is already
                                     * against the sidebar, so a tooltip on any other side would
                                     * cover the tray. The offset is measured from the marker,
                                     * which is what the trigger is.
                                     *
                                     * The width is the constraint the descriptions' length is under,
                                     * and there is no maximum height on purpose: a description long
                                     * enough to want a scrollbar is a sign the catalogues have
                                     * drifted back towards the reference's paragraph, and the drift
                                     * should be visible rather than quietly clipped. Osaka's is kept
                                     * to its siblings' length, a declared divergence from the
                                     * reference: a sentence about what a model is for belongs in a
                                     * hover, and its download size belongs where the download is.
                                     */}
                                    <TooltipContent side="left" sideOffset={16} className="max-w-72">
                                        {description}
                                    </TooltipContent>
                                </Tooltip>
                            )}
                            {modelLabel(t, option)}
                        </ToggleGroupItem>
                    );
                })}
            </ToggleGroup>
        </div>
    );
};

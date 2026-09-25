import { useTranslation } from "react-i18next";
import { Progress } from "@/components/ui/progress";
import type { RunProgress } from "@/ipc/enhance";
import { progressLabelKey } from "@/lib/enhancements";

/** What a fraction of one is as a whole percentage, which is all either figure is drawn as. */
const percent = (fraction: number) => Math.round(fraction * 100);

/**
 * What the run is doing, over the preview: how far the whole chain has got, and which enhancement it
 * is on - or the fetch's own percentage while a model is being installed.
 *
 * **Drawn from the moment a run is asked for, before it has reported anything.** A run spends its
 * first seconds loading a model and arranging the work, and nothing reports during that - so a bar
 * that waited for the first report appeared long after the enhancement was added, and the interval
 * in between read as the application having ignored the click. An unreported run is drawn empty and
 * labelled generically, which is the one honest thing to say about it: something is running and it
 * has not said what yet.
 *
 * The cost is that a chain made entirely of results the application already had now flashes the bar
 * for as long as that chain takes, rather than never drawing it. That is the trade the reverse
 * asks for: the case it makes legible is every real run, and the case it spoils finishes too fast to
 * read either way.
 *
 * **The enhancement is named from the report's own family**, in the language the rest of the window
 * is speaking. The report's `operation` is the library's composed English sentence, `Kyoto 4x
 * (FP16)`: right for a log, and not something a front end can translate.
 *
 * **A fetch is spelled out with its own percentage.** The bar tracks the whole chain, and a fetch
 * occupies only the head of one operation's share of it - so during a multi-gigabyte download the
 * bar barely moves, and without the number it reads as a stall. That is the reference's own
 * correction, and the reason the report carries two figures.
 *
 * **A family the catalogue publishes and `ENHANCEMENTS` does not name falls back to the generic
 * label**, which is the same label an unreported run carries. It is the same class of drift
 * `lib/enhancements.ts` answers for a model the catalogue stops publishing, and the same answer:
 * draw something honest rather than throw.
 *
 * **The position is given, not read off the report.** `report.chainFraction` describes the run the
 * report came from, and a face recovery is two runs - a detection and then the chain - each
 * reporting its own `0..1`. `hooks/useEnhancementRun.ts` composes the one figure that spans both;
 * drawing the report's own filled the bar, emptied it and filled it again.
 *
 * **A detection reads as Face Recovery**, which is why the name comes through `progressLabelKey`
 * rather than off `ENHANCEMENTS` directly: the detector is not an enhancement a user adds and is
 * offered by no menu here, so what the chip should say is the enhancement that needed it. The
 * transfer of that detector is the first thing a user sees of a face recovery, and it would
 * otherwise be a window sitting still through a download.
 */
export const ProgressBar = ({ report, fraction }: { report?: RunProgress; fraction: number }) => {
    const { t } = useTranslation();

    const named = progressLabelKey(report?.family);

    const label =
        report?.stage === "installing" && report.installFraction !== undefined
            ? t("preview.progress.downloading", { percent: percent(report.installFraction) })
            : named
              ? t(named)
              : t("preview.progress.enhancing");

    return (
        <div
            /*
             * Three stacked shadows rather than one: a hairline contact shadow that keeps the bar's
             * own edge readable, and two progressively wider and softer casts. A single shadow over
             * a photograph reads as a grey smudge - the bar has to sit above an arbitrary image, and
             * it is the falloff between the tight cast and the wide one that the eye reads as
             * height rather than as a border.
             */
            className="absolute top-4 right-4 z-5 h-8 w-37 overflow-hidden rounded-lg shadow-[0_1px_2px_rgba(0,0,0,0.35),0_4px_10px_-2px_rgba(0,0,0,0.45),0_12px_28px_-8px_rgba(0,0,0,0.5)]"
            data-slot="preview-progress"
        >
            {/*
             * The shared bar rather than a filled `<div>` of this component's own: its indicator
             * carries the transition that turns each report into a slide rather than a jump, and
             * reports arrive about a hundred times a run - far enough apart for the difference to
             * be the whole of how the bar reads. It also makes the thing a `progressbar` to a
             * screen reader, which a pair of nested boxes was not.
             */}
            <Progress value={percent(fraction)} className="h-full rounded-lg bg-secondary" />

            {/*
             * The bevel, as an overlay of its own rather than as an inset shadow on the box above:
             * an inset shadow paints over its element's background but under its children's, so on
             * the container it would disappear behind the track, and on the track it would disappear
             * behind the indicator for as much of the bar as the run has filled. Drawn last, over
             * both, it survives the fill sweeping across it.
             *
             * A light top edge and a dark bottom one are what make the surface look lit from above -
             * without them the layered cast below reads as the bar hovering flat rather than as a
             * solid object with a thickness.
             */}
            <span
                aria-hidden="true"
                className="pointer-events-none absolute inset-0 rounded-lg shadow-[inset_0_1px_0_rgba(255,255,255,0.3),inset_0_-1px_0_rgba(0,0,0,0.35),inset_0_0_0_1px_rgba(255,255,255,0.08)]"
            />

            {/*
             * Stretched to the bar's box rather than laid out inside it: an inset-less absolute
             * child gets a shrink-to-fit width bounded by the space left of its centred static
             * position, which wraps two-word labels that would otherwise fit. The reference's own
             * component carries this correction.
             */}
            <span className="absolute inset-0 flex items-center justify-center px-1 font-medium text-[13px] text-white whitespace-nowrap">
                {label}
            </span>
        </div>
    );
};

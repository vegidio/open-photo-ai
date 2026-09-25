import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import type { ParseKeys } from "i18next";
import { useExportFormats } from "@/hooks/useExportFormats";
import { clampQuality } from "@/stores/settings";
import { useSettingsDraft } from "./draft";
import { SettingsCard } from "./rows.tsx";

/**
 * What a typed quality commits as: rounded and brought inside the bounds, or `undefined` for text that
 * is not a number - which the box answers by going back to the value it held.
 */
export const parseQuality = (text: string): number | undefined => {
    // `Number("")` is 0, which would clamp to the minimum - an emptied box is a user who has not
    // finished, not one who asked for 1.
    if (text.trim() === "") return undefined;

    const value = Number(text);
    if (!Number.isFinite(value)) return undefined;

    return clampQuality(value);
};

/**
 * The typed half of a quality row.
 *
 * **It holds its own text while focused**, so typing "4" on the way to "42" does not move the slider
 * to 4, and clearing the box to retype it does not clamp to the minimum mid-keystroke. Leaving it or
 * pressing Enter commits; unfocused, it shows the draft's value, so a drag and the box never disagree.
 */
const QualityBox = ({
    label,
    value,
    onCommit,
}: {
    label: string;
    value: number;
    onCommit: (value: number) => void;
}) => {
    // `undefined` while not being edited, which is what makes the box follow the slider.
    const [text, setText] = useState<string>();

    const commit = () => {
        if (text === undefined) return;

        const parsed = parseQuality(text);
        if (parsed !== undefined) onCommit(parsed);
        setText(undefined);
    };

    // `type="text"` with a numeric keyboard rather than `type="number"`: a number input's spinner and
    // its browser-specific handling of invalid text would fight the clamp-on-commit rule. Escape is left
    // alone - it cancels the dialog, which discards the uncommitted text with the rest of the draft.
    return (
        <Input
            type="text"
            inputMode="numeric"
            aria-label={label}
            value={text ?? String(value)}
            onFocus={() => setText(String(value))}
            onChange={(event) => setText(event.target.value)}
            onBlur={commit}
            onKeyDown={(event) => {
                if (event.key === "Enter") commit();
            }}
            className="h-8 px-1 text-center font-mono md:text-[13px]"
        />
    );
};

/**
 * Screen 16d: one encoder quality per lossy format, **one value per format rather than one**, each a filled
 * slider and a box it can be typed into. The lossless formats get no row at all.
 *
 * **Which formats have a row, their bounds and where each starts are Rust's**, read off `export_formats`: a
 * format published as taking a quality gets a row, and one the user never moved shows its published default.
 * So the rows are drawn once that answer has arrived.
 */
export const ExportPage = () => {
    const { t } = useTranslation();
    const { values, update } = useSettingsDraft();
    const formats = useExportFormats();

    // One per format because the scales are not comparable between encoders: the same number is a different
    // picture in each, so one value shared across formats would be a setting that means something
    // different in every row it appeared in. The lossless formats' encoders ignore a quality, so a
    // slider for one would be a control with no effect, and there is no stored value behind it either.
    //
    // No mark at each format's starting value, where the reference labels one: the design draws none,
    // and getting back to where a format started is now Reset to defaults on this page.
    return (
        <SettingsCard>
            {formats?.formats.map(({ format, quality: range }) => {
                if (!range) return null;

                // The name is translated where a catalogue names it - the four lossy formats all do - and a
                // format Rust later publishes a quality for reads as its own spelling until one does.
                const name = t(`settings.export.${format}.title` as ParseKeys, { defaultValue: format.toUpperCase() });
                const value = values.quality[format] ?? range.default;
                const set = (quality: number) => update({ quality: { ...values.quality, [format]: quality } });

                return (
                    <div
                        key={format}
                        data-slot="quality-row"
                        className="grid h-14 grid-cols-[72px_minmax(0,1fr)_56px] items-center gap-5 px-4"
                    >
                        <span className="text-sm">{name}</span>

                        <Slider
                            value={[value]}
                            min={range.min}
                            max={range.max}
                            step={1}
                            thumbLabel={name}
                            // The design's `--input` track rather than the generated `--muted`, which
                            // is the card's own step and all but vanishes on it. From here rather than
                            // in the component, whose two other callers sit on other surfaces.
                            className="**:data-[slot=slider-track]:bg-input"
                            onValueChange={([next]) => next !== undefined && set(next)}
                        />

                        <QualityBox
                            label={t("settings.export.valueLabel", { format: name })}
                            value={value}
                            onCommit={set}
                        />
                    </div>
                );
            })}
        </SettingsCard>
    );
};

/**
 * One option in the aspect-ratio grid: what it is called, what it constrains the rectangle to, and
 * what the swap control turns it into.
 */
export type RatioOption = {
    /** The option's own key, which is what the grid marks and what {@link RatioOption.inverse} names. */
    key: string;
    // The numeric ratios are not keyed because a ratio reads the same in every language - the
    // reference's call. `translate` states which a label is rather than leaving the grid to sniff for a
    // dot: a key is data, not a shape to be sniffed at the call site.
    /**
     * What the option is called: a catalogue key for Free and Square, and the literal for the eight
     * numeric ones.
     */
    label: string;
    /** Whether {@link RatioOption.label} is a catalogue key rather than the text itself. */
    translate: boolean;
    /**
     * The constraint, as width over height. Absent for the free ratio - no constraint - rather than a
     * sentinel number, which is also what the widget's `aspectRatio` prop takes.
     */
    value?: number;
    // **Declared rather than derived.** A swap that looked its partner up by inverting the number
    // could not match a key: `16 / 9` and `9 / 16` do not multiply back to 1 in binary floating
    // point, so the lookup would miss for exactly the pairs it exists to serve.
    /**
     * The key of this option's transpose, which is where the swap control goes. Free and Square are
     * their own inverses.
     */
    inverse: string;
    // The design's own numbers - scaled by the 0.82 its own renderer applies, rounded - rather than
    // the ratio applied to a box: it draws each option as a rectangle *in that ratio's own
    // proportions*, sized by eye so that ten wells of different shapes read as one row of controls -
    // 5:4 and 4:3 are a pixel apart in aspect and would otherwise be indistinguishable. Free is the
    // largest, being the absence of a constraint rather than a shape.
    /**
     * The proportions of the little rectangle the grid draws inside the option's well, in pixels at
     * a scale of 1 - `AspectRatios.tsx`'s `RATIO_BOX_SCALE` is what the grid actually draws them at.
     */
    box: { width: number; height: number };
};

/**
 * The ten options the design draws, in the order it draws them: two columns, five rows, every entry
 * beside its own transpose.
 */
export const RATIOS: RatioOption[] = [
    { key: "free", label: "crop.ratio.free", translate: true, inverse: "free", box: { width: 18, height: 18 } },
    {
        key: "square",
        label: "crop.ratio.square",
        translate: true,
        value: 1,
        inverse: "square",
        box: { width: 16, height: 16 },
    },
    { key: "5:4", label: "5:4", translate: false, value: 5 / 4, inverse: "4:5", box: { width: 20, height: 16 } },
    { key: "4:5", label: "4:5", translate: false, value: 4 / 5, inverse: "5:4", box: { width: 16, height: 20 } },
    { key: "4:3", label: "4:3", translate: false, value: 4 / 3, inverse: "3:4", box: { width: 20, height: 15 } },
    { key: "3:4", label: "3:4", translate: false, value: 3 / 4, inverse: "4:3", box: { width: 15, height: 20 } },
    { key: "3:2", label: "3:2", translate: false, value: 3 / 2, inverse: "2:3", box: { width: 20, height: 13 } },
    { key: "2:3", label: "2:3", translate: false, value: 2 / 3, inverse: "3:2", box: { width: 13, height: 20 } },
    { key: "16:9", label: "16:9", translate: false, value: 16 / 9, inverse: "9:16", box: { width: 21, height: 12 } },
    { key: "9:16", label: "9:16", translate: false, value: 9 / 16, inverse: "16:9", box: { width: 12, height: 21 } },
];

/** The key of the option with no constraint, which is what the grid starts on and what Reset returns to. */
export const FREE_RATIO = "free";

/** One option by its key, or `undefined` for a key the table does not carry. */
export const ratioByKey = (key: string) => RATIOS.find((option) => option.key === key);

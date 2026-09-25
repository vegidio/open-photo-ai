import type { CSSProperties } from "react";
import { cn } from "@/lib/utils";

type ShineBorderProps = {
    /** The width of the border the shine is drawn into, in pixels. */
    borderWidth?: number;
    /** How long one sweep of the shine takes, in seconds. */
    duration?: number;
    /** The shine's colour - one, or a list the gradient runs through in the order given. */
    shineColor?: string | string[];
    /** Applied to the shine, which is this element rather than something inside it. */
    className?: string;
    /** Merged over the component's own declarations, which is why it is spread last. */
    style?: CSSProperties;
};

/**
 * A shine travelling around the border of whatever it is placed in.
 *
 * Ported from MagicUI's `shine-border`, whose structure this keeps but for the one divergence noted
 * below. The element is full-bleed over its parent and inherits the parent's radius, and everything
 * it draws is a background: a radial gradient sized to 300% of the box, so only a part of it is over
 * the element at a time, moved by animating `background-position`. What turns that into a border is the mask -
 * two identical `linear-gradient(#fff 0 0)` layers, one clipped to the content box and one to the
 * whole box, composited with `exclude` - which leaves exactly the padding ring and nothing inside
 * it. The padding is `--border-width`, so the ring is as thick as the border asked for. The parent
 * must be positioned and must carry the radius; this component reads it with `rounded-[inherit]`.
 *
 * `-webkit-mask` and `-webkit-mask-composite: xor` are written beside the standard pair because
 * WKWebView is the platform this application ships on, and the two spellings take different
 * keywords for the same operation - `xor` there, `exclude` in the standard property.
 *
 * **Unlike the `border-beam` this replaced, nothing here has a path to follow.** That component
 * moved a rigid square along an `offset-path`, which is why its corners had to be rounded by the
 * beam's own length rather than by the box's, and why `offset-path: rect()` support was a risk the
 * change had to verify in the window first. A moving background has neither problem: the shine is
 * the whole border at once, the mask is the shape, and the corners are the parent's radius exactly.
 *
 * **The animation is a Tailwind theme entry rather than the animation longhands**, which is the one
 * place this differs from how `border-beam` was written here. It can be, because `duration` reaches
 * the keyframe as a custom property rather than as a baked-in value, so one utility serves every
 * caller. **That entry lives in an `@theme inline` block, and the `inline` is load-bearing**: a
 * plain `@theme` emits `--animate-shine` onto `:root`, where its `var(--duration)` would be
 * substituted against a `--duration` that does not exist there and poison the whole declaration
 * before any element could supply one. `inline` substitutes the value into the utility instead, so
 * `--duration` is read from the element wearing it. See `style.css`.
 *
 * `motion-safe:` is upstream's and is kept: this is decoration, and a user who has asked their
 * system for less motion gets the border without the sweep rather than nothing at all.
 *
 * **The colours reach the gradient as `--shine-colors` rather than being interpolated into it**,
 * which is the one structural divergence from upstream. `var()` is substituted before the gradient
 * is parsed, so the picture is identical either way; what changes is that the only part of that
 * declaration a caller varies is now a named property beside the other two, rather than a string
 * rebuilt on every render. It is also the only part that can be seen from a test: jsdom's CSS
 * parser discards any `background-image` whose value contains a `var()`, and every colour in this
 * codebase is a token - so with the list interpolated in, a caller's colours would read back empty
 * and nothing would hold the join or its order to anything.
 *
 * Upstream spreads the rest of `HTMLAttributes<HTMLDivElement>` onto the element; this takes
 * `className` and `style` and stops there, as the `border-beam` port did. Nothing that wears a
 * shine has had anything else to say to it.
 *
 * Decorative, and says so: nothing here is content, and it is `pointer-events-none` so an element
 * wearing a shine still receives everything the pointer does.
 */
export const ShineBorder = ({
    borderWidth = 1,
    duration = 14,
    shineColor = "#000000",
    className,
    style,
}: ShineBorderProps) => (
    <div
        data-slot="shine-border"
        aria-hidden="true"
        className={cn(
            "pointer-events-none absolute inset-0 size-full rounded-[inherit] will-change-[background-position] motion-safe:animate-shine",
            className,
        )}
        style={
            {
                "--border-width": `${borderWidth}px`,
                "--duration": `${duration}s`,
                "--shine-colors": Array.isArray(shineColor) ? shineColor.join(",") : shineColor,
                backgroundImage: "radial-gradient(transparent,transparent,var(--shine-colors),transparent,transparent)",
                backgroundSize: "300% 300%",
                mask: "linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0)",
                WebkitMask: "linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0)",
                WebkitMaskComposite: "xor",
                maskComposite: "exclude",
                padding: "var(--border-width)",
                ...style,
            } as CSSProperties
        }
    />
);

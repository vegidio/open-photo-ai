import { useCallback, useEffect, useMemo, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useResizeObserver } from "@/hooks/useResizeObserver";
import { cn } from "@/lib/utils";

/**
 * One particle, as upstream models it.
 *
 * `targetAlpha` is what the particle fades *towards*; `alpha` is where it is now, which the edge
 * fade below drives down as the particle nears the box's border. `dx`/`dy` are its own drift, fixed
 * for its lifetime - a particle that leaves the box is replaced rather than steered back.
 *
 * **No `magnetism`, and no `translateX`/`translateY`.** Upstream leans each particle toward the
 * cursor by those three, and the design's own port keeps the arithmetic with the term multiplied out
 * (`c.magnetism * 0`). With the pointer gone (see {@link Particles}) the translation is provably
 * zero at every frame - it starts at zero and is only ever driven toward zero - so the three fields,
 * the `ease` that divided them and the `ctx.translate` that applied them are dead rather than
 * merely unused, and carrying them would be carrying a workaround for a feature this does not have.
 */
type Circle = {
    x: number;
    y: number;
    size: number;
    alpha: number;
    targetAlpha: number;
    dx: number;
    dy: number;
};

/** `#rgb` or `#rrggbb` to the three channels `rgba()` wants, as both upstream and the design do. */
const hexToRgb = (hex: string): [number, number, number] => {
    const bare = hex.replace("#", "");
    const full = bare.length === 3 ? [...bare].map((channel) => channel + channel).join("") : bare;
    const value = Number.parseInt(full, 16);

    return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
};

/** How near the border a particle has to be before it starts fading out, in CSS pixels. */
const EDGE = 20;

/**
 * Runs `draw` once per animation frame, but only while the application's window has focus.
 *
 * **Focus, not visibility** (design.md D4). `requestAnimationFrame` is already throttled to a stop
 * by the browser when a tab is hidden, so `document.visibilityState` buys nothing - and in a desktop
 * application the window is *visible* far more often than it is focused. An unfocused window is the
 * normal state of an application nobody is using, and holding a core awake to animate something
 * nobody is looking at is the one cost here with no defence.
 *
 * **Stopping is invisible.** It cancels the pending frame and leaves the last one painted, so the
 * field is still drawn while the window is away; resuming schedules a new frame against the same
 * particle array, so nothing reshuffles. The particles live in a ref rather than in state precisely
 * so there is nothing to preserve across the gap.
 *
 * Tauri's own window focus is the signal, with the DOM's `focus`/`blur` as the fallback: the test
 * environment has no Tauri to ask, and `getCurrentWindow` reads a global that the init script
 * installs. Both are wired at once rather than one or the other - they report the same thing, and
 * scheduling is idempotent below, so a browser that delivers both is not a second loop.
 *
 * `draw` must be stable - it is the effect's dependency, and an inline closure would cancel and
 * reschedule the loop on every render.
 */
const useAnimationFrames = (draw: () => void) => {
    useEffect(() => {
        let pending: number | undefined;

        const tick = () => {
            draw();
            pending = requestAnimationFrame(tick);
        };

        // Idempotent, which is what lets both signals drive it: a focus event arriving while the
        // loop is already running must not leave a second one scheduled and uncancellable.
        const start = () => {
            if (pending === undefined) pending = requestAnimationFrame(tick);
        };

        const stop = () => {
            if (pending !== undefined) cancelAnimationFrame(pending);
            // Cleared rather than left behind, so `start` can tell "not running" from "running", and
            // so the cleanup below cannot cancel a handle that has already been used.
            pending = undefined;
        };

        // `document.hasFocus()` rather than an assumption either way: a window that opens focused
        // must animate without waiting for a focus event that already happened, and one that opens
        // behind another must not.
        if (document.hasFocus()) start();

        window.addEventListener("focus", start);
        window.addEventListener("blur", stop);

        // Tauri's answer, which is the one that is right on a desktop. It resolves asynchronously,
        // so the unlisten may arrive after this effect is torn down - hence the flag rather than a
        // bare assignment, or a listener would outlive the component that registered it.
        let disposed = false;
        let unlisten: (() => void) | undefined;

        void (async () => {
            try {
                const dispose = await getCurrentWindow().onFocusChanged(({ payload: focused }) =>
                    focused ? start() : stop(),
                );

                if (disposed) dispose();
                else unlisten = dispose;
            } catch {
                // No Tauri here, which is every test and nothing else. `getCurrentWindow` reads a
                // global the init script installs, so without one it *throws* rather than rejecting
                // - which is why this is a `try` around the call and not a `.catch` on the promise.
                // The DOM listeners above already cover that case, so there is nothing to report.
            }
        })();

        return () => {
            disposed = true;
            unlisten?.();
            window.removeEventListener("focus", start);
            window.removeEventListener("blur", stop);
            stop();
        };
    }, [draw]);
};

/**
 * A drifting field of particles, drawn on a 2D canvas behind whatever is placed over it.
 *
 * Vendored from [MagicUI](https://magicui.design/docs/components/particles), which declares no npm
 * dependencies - it is hooks and a canvas - so it is adapted here the way every other shadcn
 * component in this frontend is rather than installed. Three deliberate divergences, each argued in
 * the change's design.md (D3):
 *
 * 1. **No mouse magnetism.** Upstream drives a `useState` from a `window` `mousemove` listener and
 *    leans every particle toward the cursor. That is a re-render of this component on every frame of
 *    a pan gesture over the canvas, for an effect that competes with the photograph being judged.
 *    The listener, the hook and the `staticity` prop that weighted it are all gone - see
 *    {@link Circle} for what goes with them.
 * 2. **A `ResizeObserver` rather than a debounced `window` resize listener.** This canvas changes
 *    size when the window does not: the sidebar and the drawer are laid out over it. `hooks/
 *    useResizeObserver.ts` is already here for exactly this, and it guards the environments that
 *    have no `ResizeObserver` - jsdom among them.
 * 3. **The animation is gated on window focus**, which is {@link useAnimationFrames} above.
 *
 * Everything else is upstream's: the per-particle `targetAlpha`, the fade as a particle nears the
 * edge, and the respawn when one leaves the box entirely.
 *
 * **It is decorative and says so structurally.** `aria-hidden` keeps it out of the accessibility
 * tree and `pointer-events-none` keeps it out of the way of the pointer - the second is the one that
 * matters beyond politeness, because the canvas this is drawn into carries the wheel-zoom and
 * drag-to-pan handlers, and a full-bleed element over them that could swallow an event is exactly
 * the failure that class prevents.
 */
export const Particles = ({
    className,
    quantity = 100,
    size = 0.4,
    color = "#ffffff",
    speed = 0.1,
    vx = 0,
    vy = 0,
}: {
    className?: string;
    quantity?: number;
    size?: number;
    color?: string;
    speed?: number;
    vx?: number;
    vy?: number;
}) => {
    const container = useRef<HTMLDivElement>(null);
    const canvas = useRef<HTMLCanvasElement>(null);
    const context = useRef<CanvasRenderingContext2D | null>(null);
    const circles = useRef<Circle[]>([]);
    const box = useRef({ width: 0, height: 0 });

    // Read once per render rather than per frame: it only changes when the window moves to a display
    // of another density, and that resizes the canvas, which re-initialises through the observer.
    const dpr = typeof window === "undefined" ? 1 : window.devicePixelRatio || 1;
    const rgb = useMemo(() => hexToRgb(color), [color]);

    /**
     * A particle somewhere in the box, at a drift and a target opacity of its own.
     *
     * `speed` is the width of the range its drift is drawn from, so a particle moves at up to half
     * of it per axis per *frame* - the loop is an unthrottled `requestAnimationFrame`, so the same
     * value covers twice the ground per second on a 120Hz display as on a 60Hz one. That is
     * upstream's arithmetic unchanged, and the reason the default is the `0.1` it hardcoded.
     */
    const spawn = useCallback(
        (): Circle => ({
            x: Math.floor(Math.random() * box.current.width),
            y: Math.floor(Math.random() * box.current.height),
            size: Math.floor(Math.random() * 2) + size,
            alpha: 0,
            targetAlpha: Number.parseFloat((Math.random() * 0.6 + 0.1).toFixed(1)),
            dx: (Math.random() - 0.5) * speed,
            dy: (Math.random() - 0.5) * speed,
        }),
        [size, speed],
    );

    // Both stable, which `useResizeObserver` requires: an inline closure would tear down and rebuild
    // the observer on every render.
    const getContainer = useCallback(() => container.current, []);

    /** One frame: fade each particle for its distance to the edge, move it, draw it, respawn it. */
    const frame = useCallback(() => {
        const ctx = context.current;
        const { width, height } = box.current;
        if (!ctx) return;

        ctx.clearRect(0, 0, width, height);

        // Set once per frame rather than per particle. The colour is constant for the component's
        // lifetime, so composing an `rgba()` string inside the loop was one array join, one template
        // allocation and one CSS-colour parse per particle per frame - at the configured field size
        // and 60Hz, some seven thousand of each per second to say the same three numbers. What
        // actually varies per particle is the opacity, and that is what `globalAlpha` is for.
        ctx.fillStyle = `rgb(${rgb.join(", ")})`;

        circles.current.forEach((circle, index) => {
            const closest = Math.min(circle.x, circle.y, width - circle.x, height - circle.y);
            const remap = Math.max(0, Math.min(1, closest / EDGE));
            circle.alpha = circle.targetAlpha * remap;

            circle.x += circle.dx + vx;
            circle.y += circle.dy + vy;

            ctx.beginPath();
            ctx.arc(circle.x, circle.y, circle.size, 0, 2 * Math.PI);
            ctx.globalAlpha = circle.alpha;
            ctx.fill();

            // Gone from the box entirely, rather than merely faded out: replaced by a new one, which
            // is what keeps the field's population constant without ever steering a particle.
            const gone =
                circle.x < -circle.size ||
                circle.x > width + circle.size ||
                circle.y < -circle.size ||
                circle.y > height + circle.size;
            if (gone) circles.current[index] = spawn();
        });
    }, [rgb, spawn, vx, vy]);

    /**
     * Sizes the backing store to the box at the display's density and fills it with a fresh field.
     *
     * Run once on mount and again on every resize. The particle array is rebuilt rather than
     * rescaled, which is upstream's behaviour: a field stretched to a new box would bunch up along
     * whichever edge moved.
     *
     * Nothing here writes anything that affects the observed box - the canvas is absolutely
     * positioned and sized in percentages - so this cannot feed its own observer.
     *
     * **It paints one frame itself**, rather than leaving the first to the loop. The loop is gated
     * on focus, so a field mounted while the window is behind another - the application launched
     * and switched away from before its window came up - would otherwise show nothing at all until
     * the user came back to it. That is the same blank canvas the focus gate is careful to avoid on
     * the way out, arrived at from the other side: `useAnimationFrames` stops by leaving the last
     * frame painted precisely so the field is still drawn while the window is away, and a field
     * that has never painted a first frame has no last one to leave.
     *
     * It is also why `frame` is declared above rather than below: a `useCallback` dependency array
     * is evaluated as the component renders, so a `const` named in one has to already exist.
     */
    const initialise = useCallback(
        (element: Element) => {
            const rect = element.getBoundingClientRect();
            box.current = { width: Math.max(1, rect.width), height: Math.max(1, rect.height) };

            const surface = canvas.current;
            if (!surface) return;

            context.current ??= surface.getContext("2d");
            surface.width = box.current.width * dpr;
            surface.height = box.current.height * dpr;
            // jsdom provides no 2D context at all, so every drawing call below is guarded rather
            // than assumed - the component still mounts and still measures there.
            context.current?.setTransform(dpr, 0, 0, dpr, 0, 0);

            circles.current = Array.from({ length: quantity }, spawn);
            frame();
        },
        [dpr, frame, quantity, spawn],
    );

    useResizeObserver(getContainer, initialise);

    useAnimationFrames(frame);

    return (
        <div
            ref={container}
            // Decorative, and not in anyone's way: see the note above on why `pointer-events-none` is
            // load-bearing rather than tidy.
            aria-hidden
            data-slot="particles"
            className={cn("pointer-events-none absolute inset-0", className)}
        >
            <canvas ref={canvas} className="block h-full w-full" />
        </div>
    );
};

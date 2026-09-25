import type { ComponentProps } from "react";
import { CheckIcon, MinusIcon } from "lucide-react";
import { Checkbox as CheckboxPrimitive } from "radix-ui";
import { cn } from "@/lib/utils";

const Checkbox = ({ className, ...props }: ComponentProps<typeof CheckboxPrimitive.Root>) => {
    // The generated file draws a tick and only a tick, so a control Radix is holding in its third
    // state rendered as though every one of its items were picked. The design draws a dash there,
    // which is what "some of them" looks like in every native checkbox - and it is the second edit
    // to reapply after `shadcn add checkbox`, beside the radius below.
    const indeterminate = props.checked === "indeterminate";

    return (
        <CheckboxPrimitive.Root
            data-slot="checkbox"
            className={cn(
                // `rounded-sm`, not the `rounded-lg` the CLI generates: on a `size-4` box an 8px
                // radius is half the side, so the control renders as a circle and reads as a radio
                // button. The design draws every checkbox - this one and the drawer items' - with 4px
                // corners, which is what `--radius-sm` resolves to. Re-running `shadcn add checkbox`
                // puts the generated value back, so this is one of the edits to reapply after it.
                // The `data-[state=indeterminate]:*` pair is this application's, beside the two the
                // CLI generates for `checked`: the design fills the box with the accent from the
                // first pick onwards, so a partial selection is a filled box with a dash rather than
                // an empty one.
                "peer size-4 shrink-0 rounded-sm border border-input shadow-xs transition-shadow outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground",
                className,
            )}
            {...props}
        >
            <CheckboxPrimitive.Indicator
                data-slot="checkbox-indicator"
                className="grid place-content-center text-current transition-none"
            >
                {indeterminate ? <MinusIcon className="size-3.5" /> : <CheckIcon className="size-3.5" />}
            </CheckboxPrimitive.Indicator>
        </CheckboxPrimitive.Root>
    );
};

export { Checkbox };

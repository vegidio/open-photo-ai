import type { ComponentProps } from "react";
import { Popover as PopoverPrimitive } from "radix-ui";
import { cn } from "@/lib/utils";

const Popover = ({ ...props }: ComponentProps<typeof PopoverPrimitive.Root>) => {
    return <PopoverPrimitive.Root data-slot="popover" {...props} />;
};

const PopoverTrigger = ({ ...props }: ComponentProps<typeof PopoverPrimitive.Trigger>) => {
    return <PopoverPrimitive.Trigger data-slot="popover-trigger" {...props} />;
};

// `container` is this project's one addition to what `shadcn add` writes, and it is the same one
// `dropdown-menu.tsx` carries: the component portals its content with the target hard-wired to the
// body, so a caller that needs the panel to land somewhere else has no way to say so - and wrapping
// this in a portal of its own does nothing, because the inner portal wins and nothing errors.
// Absent, it is Radix's default.
const PopoverContent = ({
    className,
    align = "center",
    sideOffset = 4,
    container,
    ...props
}: ComponentProps<typeof PopoverPrimitive.Content> & {
    container?: ComponentProps<typeof PopoverPrimitive.Portal>["container"];
}) => {
    return (
        <PopoverPrimitive.Portal {...(container && { container })}>
            <PopoverPrimitive.Content
                data-slot="popover-content"
                align={align}
                sideOffset={sideOffset}
                className={cn(
                    "z-50 w-72 origin-(--radix-popover-content-transform-origin) rounded-md border bg-popover p-4 text-popover-foreground shadow-md outline-hidden data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95",
                    className,
                )}
                {...props}
            />
        </PopoverPrimitive.Portal>
    );
};

// Radix publishes a `Close` and `shadcn add` does not generate one, which leaves a panel with a
// close box of its own no way to dismiss through Radix. Going through it rather than calling a
// handler is what `DialogTitleBar` records: a panel whose close button called a handler directly
// would have two ways out - that one and Escape - that could come to do different things.
const PopoverClose = ({ ...props }: ComponentProps<typeof PopoverPrimitive.Close>) => {
    return <PopoverPrimitive.Close data-slot="popover-close" {...props} />;
};

const PopoverAnchor = ({ ...props }: ComponentProps<typeof PopoverPrimitive.Anchor>) => {
    return <PopoverPrimitive.Anchor data-slot="popover-anchor" {...props} />;
};

const PopoverHeader = ({ className, ...props }: ComponentProps<"div">) => {
    return <div data-slot="popover-header" className={cn("flex flex-col gap-1 text-sm", className)} {...props} />;
};

const PopoverTitle = ({ className, ...props }: ComponentProps<"h2">) => {
    return <div data-slot="popover-title" className={cn("font-medium", className)} {...props} />;
};

const PopoverDescription = ({ className, ...props }: ComponentProps<"p">) => {
    return <p data-slot="popover-description" className={cn("text-muted-foreground", className)} {...props} />;
};

export {
    Popover,
    PopoverAnchor,
    PopoverClose,
    PopoverContent,
    PopoverDescription,
    PopoverHeader,
    PopoverTitle,
    PopoverTrigger,
};

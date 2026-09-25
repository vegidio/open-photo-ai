import type { ComponentProps } from "react";
import { XIcon } from "lucide-react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const Dialog = ({ ...props }: ComponentProps<typeof DialogPrimitive.Root>) => {
    return <DialogPrimitive.Root data-slot="dialog" {...props} />;
};

const DialogTrigger = ({ ...props }: ComponentProps<typeof DialogPrimitive.Trigger>) => {
    return <DialogPrimitive.Trigger data-slot="dialog-trigger" {...props} />;
};

const DialogPortal = ({ ...props }: ComponentProps<typeof DialogPrimitive.Portal>) => {
    return <DialogPrimitive.Portal data-slot="dialog-portal" {...props} />;
};

const DialogClose = ({ ...props }: ComponentProps<typeof DialogPrimitive.Close>) => {
    return <DialogPrimitive.Close data-slot="dialog-close" {...props} />;
};

const DialogOverlay = ({ className, ...props }: ComponentProps<typeof DialogPrimitive.Overlay>) => {
    return (
        <DialogPrimitive.Overlay
            data-slot="dialog-overlay"
            className={cn(
                "fixed inset-0 z-50 bg-black/50 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-in data-[state=open]:fade-in-0",
                className,
            )}
            {...props}
        />
    );
};

/**
 * Takes focus into the dialog itself on open, rather than letting Radix focus its first tabbable element.
 *
 * Radix focuses the first tabbable element when a dialog opens - and WebKit matches `:focus-visible`
 * on that programmatic focus, so the design's flat buttons would come up wearing a focus ring.
 * Focusing the dialog itself leaves the trap, the labelling and Tab-to-the-buttons exactly as they
 * were, and pre-selects nothing - which is what a dialog whose every action is a real answer wants.
 */
const focusContent = (event: Event) => {
    event.preventDefault();

    const content = event.currentTarget;
    if (content instanceof HTMLElement) content.focus();
};

const DialogContent = ({
    className,
    children,
    showCloseButton = true,
    focusContentOnOpen = false,
    onOpenAutoFocus,
    ...props
}: ComponentProps<typeof DialogPrimitive.Content> & {
    showCloseButton?: boolean;
    /** Focus the dialog itself on open rather than its first control - see {@link focusContent}. */
    focusContentOnOpen?: boolean;
}) => {
    const openAutoFocus = focusContentOnOpen ? focusContent : onOpenAutoFocus;

    return (
        <DialogPortal data-slot="dialog-portal">
            <DialogOverlay />
            <DialogPrimitive.Content
                data-slot="dialog-content"
                className={cn(
                    "fixed top-[50%] left-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border bg-background p-6 shadow-lg duration-200 outline-none data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 sm:max-w-lg",
                    className,
                )}
                {...(openAutoFocus && { onOpenAutoFocus: openAutoFocus })}
                {...props}
            >
                {children}
                {showCloseButton && (
                    <DialogPrimitive.Close
                        data-slot="dialog-close"
                        className="absolute top-4 right-4 rounded-xs opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:ring-2 focus:ring-ring focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none data-[state=open]:bg-accent data-[state=open]:text-muted-foreground [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4"
                    >
                        <XIcon />
                        <span className="sr-only">Close</span>
                    </DialogPrimitive.Close>
                )}
            </DialogPrimitive.Content>
        </DialogPortal>
    );
};

const DialogHeader = ({ className, ...props }: ComponentProps<"div">) => {
    return (
        <div
            data-slot="dialog-header"
            className={cn("flex flex-col gap-2 text-center sm:text-left", className)}
            {...props}
        />
    );
};

const DialogFooter = ({
    className,
    showCloseButton = false,
    children,
    ...props
}: ComponentProps<"div"> & {
    showCloseButton?: boolean;
}) => {
    return (
        <div
            data-slot="dialog-footer"
            className={cn("flex flex-col-reverse gap-2 sm:flex-row sm:justify-end", className)}
            {...props}
        >
            {children}
            {showCloseButton && (
                <DialogPrimitive.Close asChild>
                    <Button variant="outline">Close</Button>
                </DialogPrimitive.Close>
            )}
        </div>
    );
};

const DialogTitle = ({ className, ...props }: ComponentProps<typeof DialogPrimitive.Title>) => {
    return (
        <DialogPrimitive.Title
            data-slot="dialog-title"
            className={cn("text-lg leading-none font-semibold", className)}
            {...props}
        />
    );
};

const DialogDescription = ({ className, ...props }: ComponentProps<typeof DialogPrimitive.Description>) => {
    return (
        <DialogPrimitive.Description
            data-slot="dialog-description"
            className={cn("text-sm text-muted-foreground", className)}
            {...props}
        />
    );
};

export {
    Dialog,
    DialogClose,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogOverlay,
    DialogPortal,
    DialogTitle,
    DialogTrigger,
};

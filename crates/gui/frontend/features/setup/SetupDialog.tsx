import { Dialog, DialogContent } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { useSetupStore } from "@/stores/setup";
import { SetupFailure } from "./SetupFailure";
import { SetupProgress } from "./SetupProgress";
import { SETUP_SURFACE } from "./surface";

/**
 * Screens 02 and 02b: what this machine needs, how far along it got, and - when it does not finish -
 * why, with the way out of both.
 *
 * **One dialog with two states**, switched on the store's failure. It is open for as long as it is
 * mounted, and **it cannot be dismissed.**
 */
export const SetupDialog = ({ onRetry }: { onRetry: () => void }) => {
    // Not two dialogs: the card, the overall bar, the component list and the fact that none of it can
    // be dismissed are the same thing in both states; what changes is the header, what each row says
    // about its component, whether the known cause is named and what the footer offers. Two components
    // would mean two of everything that is common, which is how they come to disagree about their own
    // width, their row heights and what a component that was never reached is called. `SetupProgress`
    // and `SetupFailure` carry what genuinely differs and nothing else.
    //
    // No "is the dialog open" flag beside the status: `App` renders this while the application is
    // starting itself or has failed to, and unmounts it the moment it is ready, because two pieces of
    // state holding one fact is two pieces of state that can disagree.
    const rows = useSetupStore((state) => state.rows);
    const failure = useSetupStore((state) => state.failure);

    return (
        <Dialog open>
            <DialogContent
                showCloseButton={false}
                className={cn(SETUP_SURFACE, "gap-0")}
                /*
                 * `alertdialog` only once it is one. The role is what tells a screen reader this is
                 * an interruption requiring a response, which a failure is and an install in
                 * progress is not - and it is a property of the state rather than of the primitive,
                 * which is why switching primitives to get it would have been the wrong seam.
                 *
                 * `dialog` is written out rather than left to Radix: this prop is spread over the
                 * primitive's own, so `undefined` here removes the role instead of falling back to
                 * it, and the content comes out a plain div with no role at all.
                 */
                role={failure ? "alertdialog" : "dialog"}
                onOpenAutoFocus={(event) => {
                    // Radix focuses the first tabbable element when a dialog opens - and WebKit
                    // matches `:focus-visible` on that programmatic focus, so the design's flat
                    // buttons would come up wearing a focus ring. Focusing the dialog itself leaves the
                    // trap, the labelling and Tab-to-the-buttons exactly as they were, and
                    // pre-selects nothing on a dialog whose actions end or restart the application.
                    event.preventDefault();

                    const content = event.currentTarget;
                    if (content instanceof HTMLElement) content.focus();
                }}
                // Radix offers three ways to close a dialog by default and all three are turned off
                // explicitly here, at the one place the dialog is built, rather than by hiding the close
                // button and hoping. Nothing in the application can be used until initialization has
                // finished, so a dismissed dialog would leave a user in front of an application that
                // looks ready and does nothing.
                onEscapeKeyDown={(event) => event.preventDefault()}
                onPointerDownOutside={(event) => event.preventDefault()}
                onInteractOutside={(event) => event.preventDefault()}
            >
                {failure ? (
                    <SetupFailure failure={failure} rows={rows} onRetry={onRetry} />
                ) : (
                    <SetupProgress rows={rows} />
                )}
            </DialogContent>
        </Dialog>
    );
};

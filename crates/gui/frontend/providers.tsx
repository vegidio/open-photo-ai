import type { ReactNode } from "react";
import { LucideProvider } from "lucide-react";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";

// All three of these are decisions made once for the whole application rather than per call site,
// and all three are the kind of decision a new call site has no way to discover it was supposed to
// make. A provider mounted inside a feature is the failure mode: a `TooltipProvider` inside the setup
// dialog's reason line and another inside the drawer header would each be its own skip-delay group -
// so a tooltip could never hand off to one in another feature - and would rebuild the provider on
// every render of a row.
//
// It is a component rather than lines in `main.tsx` so that a test renders what production renders.
// Tests mount `App`, `Drawer` and `SetupDialog` as three separate roots; without this they would each
// have to remember the list, and a tree under test would drift from the tree that ships.
/** The context every rendered tree needs, in one component so there is one answer to what that is. */
export const AppProviders = ({ children }: { children: ReactNode }) => (
    /*
     * Stroke 1.5 app-wide, which is the weight the design draws icons at; lucide's own default is 2.
     * It is set through lucide's context rather than as a prop on each icon because the shadcn
     * components render icons of their own - Checkbox's tick today, more with every `shadcn add` - so
     * a per-call-site prop would be a rule to remember rather than a decision that is made once.
     * Radix's portals are portals in the DOM but children in the React tree, so the tooltips and
     * popovers mounted on `document.body` inherit it too.
     */
    <LucideProvider strokeWidth={1.5}>
        <TooltipProvider>
            {children}

            {/*
             * The one toaster, beside the other two rather than inside the feature that raises the
             * first toast. `toast()` is called as a function, not rendered, so it addresses whichever
             * toaster happens to be mounted - which means a second one somewhere in the tree is not a
             * second place notices appear, it is an argument about where they appear at all.
             */}
            <Toaster />
        </TooltipProvider>
    </LucideProvider>
);

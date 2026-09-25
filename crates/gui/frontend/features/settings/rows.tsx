import type { ComponentProps, ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectSeparator, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

/**
 * A bordered group of settings rows, as every page draws them, **with a rule between each child** -
 * so a row, a table line or an export slider never draws its own.
 */
export const SettingsCard = ({ className, children }: { className?: string; children: ReactNode }) => (
    <div
        data-slot="settings-card"
        className={cn("flex flex-col divide-y divide-border rounded-lg border border-border", className)}
    >
        {children}
    </div>
);

// One frame rather than one per control, because a row per setting would be a copy of this each free
// to drift in its padding, its label size or whether it has a description at all.
/** What a settings row is: a label over a muted line saying what it means, and the control on the right. */
export const SettingsRow = ({
    label,
    description,
    control,
}: {
    label: string;
    description: string;
    control: ReactNode;
}) => (
    <div data-slot="settings-row" className="flex items-center justify-between gap-6 px-4 py-3.5">
        <div className="flex min-w-0 flex-col gap-1">
            <span className="text-sm">{label}</span>
            <p className="m-0 text-pretty text-foreground-dim text-xs leading-normal">{description}</p>
        </div>

        {control}
    </div>
);

/** The metrics the design gives every control in the right-hand column of a row. */
const CONTROL = "h-[34px] w-[180px] flex-none text-[13px]";

// Both generated components carry a `dark:bg-input/30`, and `twMerge` does not treat a bare utility
// and its `dark:` variant as conflicting - so a plain `bg-background` here does not replace it, it
// loses to it. Since `style.css` rebinds the `dark` variant to the class on `<html>` and forces it
// app-wide, that rule always applies: both controls would come up at `--input` over the card, one
// step lighter than the row they sit in, where the design draws the select a step *darker* and the
// button not filled at all. This is the same trap as `sm:max-w-lg` on the dialog card - see
// SettingsDialog.
//
// The select's one state is the design's; a lift on hover is also a lift while the menu is open,
// since the pointer is over the trigger to have opened it - so the control that is a dark well would
// become a light one at exactly the moment the user is looking at it. The generated
// `hover:bg-input/50` does this, and so does lifting it one token instead. Both are pinned out.
//
// The button keeps a hover, because it is an action rather than a value a user reads: pressing it
// does something, and nothing about it is open while the pointer is on it.
/**
 * The fills the design gives those controls, as `dark:` rules rather than bare ones.
 *
 * **A select holds `--background` in every state, including hover and open.**
 */
const SELECT_FILL =
    "bg-background hover:bg-background data-[state=open]:bg-background dark:bg-background dark:hover:bg-background dark:data-[state=open]:bg-background";
const BUTTON_FILL = "bg-transparent hover:bg-accent dark:bg-transparent dark:hover:bg-accent";

// Pinned for the same reason the select's fill is: the generated item's `dark:bg-input/30` always
// applies, so an unchecked circle would come up filled at `--input` where the design draws it hollow,
// with only its ring. A bare `bg-transparent` does not replace it, it loses to it.
//
// The accent border once chosen is what makes the chosen option readable at a glance rather than only
// by its dot.
/**
 * The radio's own fill, as the design draws it: hollow, with a `--foreground-faint` ring - `--border`
 * all but vanishes on the card - and the accent once chosen. Shared by the background tiles and the
 * processor cards.
 */
export const RADIO_FILL =
    "border-foreground-faint bg-transparent dark:bg-transparent data-[state=checked]:border-primary";

/**
 * A choice among several, sized for a row's control column (or, with `className`, for a table cell).
 *
 * `label` becomes the trigger's accessible name. A Radix trigger announces its current value and
 * nothing else, so without it every select on a page would be a control a screen reader cannot tell
 * apart - and no test could address one by name either.
 */
export const SettingsSelect = ({
    label,
    value,
    onChange,
    className,
    children,
}: {
    label: string;
    value: string;
    onChange: (value: string) => void;
    className?: string;
    children: ReactNode;
}) => (
    <Select value={value} onValueChange={onChange}>
        <SelectTrigger className={cn(CONTROL, SELECT_FILL, className)} aria-label={label}>
            <SelectValue />
        </SelectTrigger>
        {/*
         * The menu carries the trigger's own fill rather than `--popover`. They are the same control,
         * and `--popover` is `--card` - the surface the dialog is already drawn on - so a menu opening
         * over a settings row would be the same colour as the row behind it and read as part of it
         * rather than as a list on top of it. At `--background` it matches the well it came out of and
         * separates from the card by the same step the trigger does.
         */}
        <SelectContent className="bg-background">{children}</SelectContent>
    </Select>
);

/** One option within a {@link SettingsSelect}, re-exported so a page needs one import rather than two. */
export const SettingsSelectItem = SelectItem;

/** A rule between two groups of options within a {@link SettingsSelect}. */
export const SettingsSelectSeparator = SelectSeparator;

/** An action in a row's control column: Show logs, and nothing else yet. */
export const SettingsButton = ({ onPress, children }: { onPress: () => void; children: ReactNode }) => (
    <Button variant="outline" className={cn(CONTROL, BUTTON_FILL)} onClick={onPress}>
        {children}
    </Button>
);

// The generated switch colours only its checked track, so an off switch is transparent - on the card
// it would be a thumb floating over nothing. The design draws the off track at `--input`, and the thumb
// light in both states where the generated one is `--background`, which on the accent track reads as a
// hole rather than a knob.
/** A switch as the settings pages draw it, off track included. `aria-label` is the caller's to give. */
export const SettingsSwitch = ({ className, ...props }: ComponentProps<typeof Switch>) => (
    <Switch
        className={cn("data-[state=unchecked]:bg-input **:data-[slot=switch-thumb]:bg-foreground", className)}
        {...props}
    />
);

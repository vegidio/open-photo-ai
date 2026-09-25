import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

const Input = ({ className, type, ...props }: ComponentProps<"input">) => (
    <input
        type={type}
        data-slot="input"
        className={cn(
            // `bg-background` in place of the generated `bg-transparent ... dark:bg-input/30`. `style.css`
            // forces the `dark` variant app-wide, so the generated rule always applied and drew the box a
            // step lighter than the card it sits on, where the design draws it as a `--background` well,
            // like the select. Replaced here rather than overridden at a call site, because `twMerge`
            // does not treat a bare utility and its `dark:` variant as conflicting - see `SELECT_FILL` in
            // `features/settings/rows.tsx` for the same trap.
            "h-9 w-full min-w-0 rounded-md border border-input bg-background px-3 py-1 text-base shadow-xs outline-none transition-[color,box-shadow] selection:bg-primary selection:text-primary-foreground file:inline-flex file:font-medium file:text-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
            "focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50",
            "aria-invalid:border-destructive aria-invalid:ring-destructive/20",
            className,
        )}
        {...props}
    />
);

export { Input };

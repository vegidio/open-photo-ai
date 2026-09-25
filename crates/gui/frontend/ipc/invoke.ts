import { type InvokeArgs, invoke } from "@tauri-apps/api/core";
import { traced } from "@/lib/faro";

// The window's one call site for a request to the backend, so every request is traced by name and
// carries its trace into Rust, where `command_span!` continues it. `invoke.guard.test.ts` fails on
// `invoke` imported anywhere else, except by the two modules whose commands Rust leaves untraced:
// `analytics.ts`, because an opt-out is never reported, and `log.ts`, because a window failure is not
// a request - Faro already carries it as an exception.

/**
 * Invokes `command` with `args`, as a span named after it while the window is sending.
 *
 * The span's `traceparent` goes in the request's headers, beside the arguments rather than among them,
 * so no command's wire type names telemetry. With no span to send, `invoke` is called exactly as it
 * would be without this: no options, and no arguments object where the caller gave none.
 */
export const call = <T>(command: string, args?: InvokeArgs): Promise<T> =>
    traced(command, (headers) => {
        if (headers !== undefined) return invoke<T>(command, args, { headers });

        return args === undefined ? invoke<T>(command) : invoke<T>(command, args);
    });

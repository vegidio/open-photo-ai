import { log, type WindowRecord } from "@/ipc/log";
import { type ErrorContext, sendError } from "@/lib/faro";

// The one place the window decides what reaches the log, and Grafana. Each failure goes two ways in
// one step, under the same rules: to the log command, which writes it to the file alone, and to Faro,
// which sends it to Grafana as an exception while the window is sending. So each is in the file once
// and in Grafana once. Rust decides the rest of the record - its target, and how long each text may
// be - in `crates/gui/src/frontend.rs`.

/** At most this many records are sent per load of the window. */
const LIMIT = 50;

/** Sent once, in place of the first record past {@link LIMIT}. */
const FLOODED = "the window is recording too many failures; the rest of this load's are dropped";

// Module state, so a reload starts both again.
/** What this load has sent already, by level, message and error. */
let sent = new Set<string>();
let count = 0;

/**
 * Whether `error` is a command's answer, which Rust has already accounted for.
 *
 * Every command rejects with a `{ kind, … }` object, and each of those is either a stop or a failure
 * recorded where it happened, so recording it here would record it twice. Structural rather than a list
 * of kinds, so a kind added in Rust later needs no edit here.
 *
 * An `Error` is never one, even given a `kind` property: nothing Rust sends is an `Error`. Neither is a
 * string, which is how Tauri rejects a request that never reached a command's body - an unknown name,
 * or an argument that did not deserialize - and so one nothing on the Rust side recorded.
 */
export const alreadyRecorded = (error: unknown) =>
    typeof error === "object" &&
    error !== null &&
    !(error instanceof Error) &&
    typeof (error as { kind?: unknown }).kind === "string";

/** `error` as the record's `error` and `stack`. */
const describe = (error: unknown): Pick<WindowRecord, "error" | "stack"> => {
    if (error instanceof Error) {
        const { name, message, stack } = error;
        return stack === undefined ? { error: `${name}: ${message}` } : { error: `${name}: ${message}`, stack };
    }
    if (typeof error === "string") return { error };

    try {
        const plain =
            typeof error === "object" &&
            error !== null &&
            [Object.prototype, null].includes(Object.getPrototypeOf(error));

        return { error: plain ? JSON.stringify(error) : String(error) };
    } catch {
        // A cycle, or a `toString` that throws. The console still has the value itself.
        return { error: Object.prototype.toString.call(error) };
    }
};

/**
 * Sends `record` to the log and `error` to Faro, unless this load has sent it already or has sent its
 * share.
 */
const send = (record: WindowRecord, error: unknown, context: ErrorContext) => {
    const key = JSON.stringify([record.level, record.message, record.error]);
    if (sent.has(key) || count > LIMIT) return;

    sent.add(key);
    count += 1;

    const flooded = count > LIMIT;
    const sending = flooded ? { level: "warn" as const, message: FLOODED } : record;

    // Never reported, and never written to the console: a bridge that is gone would otherwise report its
    // own failure with every call, and the console already has the failure this was about. Through
    // `then` so an `invoke` that throws rather than rejects is dropped too.
    Promise.resolve()
        .then(() => log(sending))
        .catch(() => {});

    // The drop notice is a statement about the file, not a failure of the window's.
    if (flooded) return;

    // An `Error` as itself, so Faro parses its stack. Anything else in one carrying the record's text.
    // Dropped if Faro throws, as the log command's failure is: each path fails on its own.
    try {
        sendError(error instanceof Error ? error : new Error(record.error), context);
    } catch {}
};

/**
 * Reports a failure the window observed and carried on through: to the console, as always, and to the
 * log at warning severity and to Faro, unless Rust has recorded it already (see {@link alreadyRecorded}).
 *
 * `message` says what the window was doing; `error` is what it caught.
 */
export const report = (message: string, error: unknown) => {
    console.error(message, error);

    if (alreadyRecorded(error)) return;

    send({ level: "warn", message, ...describe(error) }, error, { type: "warn", message });
};

/**
 * Reports a render crash: to the console, and to the log at error severity and to Faro with the
 * components it was thrown under. For the error boundary alone - a render error is never a command's answer, so nothing
 * is skipped.
 */
export const reportCrash = (error: unknown, componentStack?: string) => {
    const message = "the window could not be drawn";
    console.error(message, error);

    send(
        { level: "error", message, ...describe(error), ...(componentStack === undefined ? {} : { componentStack }) },
        error,
        componentStack === undefined ? { type: "crash" } : { type: "crash", componentStack },
    );
};

/** For tests alone: a test that did not reset this would find the previous test's records sent. */
export const forget = () => {
    sent = new Set();
    count = 0;
};

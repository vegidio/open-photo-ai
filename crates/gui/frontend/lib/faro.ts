import { type Faro, initializeFaro, SessionInstrumentation, ViewInstrumentation } from "@grafana/faro-web-sdk";
import { TracingInstrumentation } from "@grafana/faro-web-tracing";
import type { TelemetryIds } from "@/ipc/app";
import type { EventName, EventParams } from "@/lib/events";

// The one module that touches Grafana Faro's SDK, or the OpenTelemetry one beneath its tracing;
// `faro.guard.test.ts` fails on an import of either anywhere else. The window sends what is the
// window's, on its own: the backend sends its records through `opai`'s telemetry, and neither half
// asks the other. Both obey the same Analytics choice, read once per launch, and both stop the moment
// it is turned off.
//
// Every export is a no-op until Faro has started, and forever in a build or a launch that sends
// nothing, so a caller never checks.

/** What a failure sent to Faro says beside the error, as `lib/report.ts` describes it. */
export type ErrorContext = { type: "warn"; message: string } | { type: "crash"; componentStack?: string };

/** The started instance, while it may send. */
let faro: Faro | undefined;

/** Whether this load has been told to stop. Terminal: nothing resumes it before the next load. */
let paused = false;

/** Whether `startFaro` has been called, so a second call asks nothing. */
let starting = false;

// OpenTelemetry's `SpanKind.CLIENT` and `SpanStatusCode.ERROR`, by number: this module imports from
// Faro alone, and its tracing hands over the API's types but not its enums.
const CLIENT = 2;
const ERROR = 2;

/**
 * Starts Faro, when this build names a collector and `analytics` - the stored choice, as this load
 * found it - is on.
 *
 * Read once, at load, as the backend reads its copy once at launch: turning Analytics on takes effect
 * at the next launch for both halves. Not awaited by `main.tsx`: what the window reports before the
 * version arrives is dropped rather than queued, and still reaches the file. An opt-out that arrives
 * meanwhile wins.
 *
 * `version` is `appVersion` from `ipc/app.ts`, and `backendIds` its `telemetryIds`, both handed in by
 * `main.tsx` rather than imported: every `ipc/` module sends through `ipc/invoke.ts`, which imports
 * this one. Only the type is imported, which leaves nothing behind at runtime.
 */
export const startFaro = async (
    analytics: boolean,
    version: () => Promise<string>,
    backendIds: () => Promise<TelemetryIds | null>,
): Promise<void> => {
    if (starting) return;
    starting = true;

    // The collector this build sends to, from its secrets. A build without one sends nothing.
    const collector = import.meta.env.VITE_FARO_URL;
    if (!collector || !analytics || paused) return;

    // The backend's ids are only a link between the two halves' records, so a failure to ask for them
    // starts Faro without them rather than not at all.
    const [built, backend] = await Promise.all([version(), backendIds().catch(() => null)]);
    if (paused) return;

    faro = initializeFaro({
        url: collector,
        // The build that bundled the window, as the backend's is the build that compiled it: a
        // release build of the application is both.
        app: { name: "opai", version: built, environment: import.meta.env.PROD ? "production" : "development" },
        // Session, view and tracing only. Not `getWebInstrumentations()`, which would add error and
        // console capture - the window already routes every failure through `lib/report.ts`, so those
        // would send the reporter's own output a second time - and web vitals, which the series left out.
        //
        // Tracing with none of its own instrumentations: `invoke` is itself a fetch to `ipc://`, and
        // the canvas loads pixels over `opai://`, so the fetch and XHR defaults would trace the
        // transport, anonymously, beside every request. The window's unit is the request, which
        // `traced` spans once, by name.
        instrumentations: [
            new SessionInstrumentation(),
            new ViewInstrumentation(),
            new TracingInstrumentation({ instrumentations: [] }),
        ],
        // The backend's session and machine on every event, as `session_attr_backend_session` and
        // `session_attr_machine_id`, so a dashboard can put the window's location beside the backend's
        // records and exclude a machine from both halves alike. In the configuration rather than set on
        // the session afterwards: Faro carries configured attributes into each session it starts after this one.
        sessionTracking: backend
            ? { enabled: true, session: { attributes: sessionAttributes(backend) } }
            : { enabled: true },
        // No global `window.faro` for other code to reach.
        isolate: true,
    });
};

/** The session attributes naming the backend's ids; a host with no machine id sends the session alone. */
const sessionAttributes = ({ session, machine }: TelemetryIds): Record<string, string> =>
    machine === null ? { backend_session: session } : { backend_session: session, machine_id: machine };

/**
 * Stops sending for the rest of this load, dropping what is held unsent. Idempotent.
 *
 * Called by the Analytics mirror before Rust is told of an opt-out. Nothing resumes it: opting in
 * takes effect at the next launch, for the window as for the backend.
 */
export const pauseFaro = (): void => {
    if (paused) return;
    paused = true;

    faro?.pause();
    faro = undefined;
};

/** Sends one of the window's failures as an exception. For `lib/report.ts` alone, which decides what is sent. */
export const sendError = (error: Error, context: ErrorContext): void => {
    if (faro === undefined) return;

    // Faro's context is text only, and a crash without a component stack has none to give.
    const { type, ...rest } = context;
    const fields = Object.fromEntries(Object.entries(rest).filter(([, value]) => value !== undefined));

    faro.api.pushError(error, { context: { type, ...fields } });
};

/** Sends one usage event from the catalogue in `lib/events.ts`. */
export const track = <E extends EventName>(event: E, params: EventParams[E]): void => {
    if (faro === undefined) return;

    // Faro's attributes are text only. A list - the changed settings - is sent the way `formatList`
    // sends file types: joined by commas.
    const attributes = Object.fromEntries(
        Object.entries(params).map(([key, value]) => [key, Array.isArray(value) ? value.join(",") : String(value)]),
    );

    // Never into the caller: an event is not worth a control that stopped working.
    try {
        faro.api.pushEvent(event, attributes);
    } catch {}
};

/** A request's span, while it is open: the header naming it, and how to close it. */
type RequestSpan = { headers: Record<string, string>; end: (failed: boolean) => void };

/** Opens `name`'s span, or answers nothing when there is no span to open or anything about it throws. */
const openSpan = (name: string): RequestSpan | undefined => {
    if (faro === undefined) return undefined;

    // Kept outside the `try`, so a span opened before something threw is still closed.
    let opened: { end: () => void } | undefined;

    try {
        const span = faro.api.getOTEL()?.trace.getTracer("opai").startSpan(name, { kind: CLIENT });
        if (span === undefined) return undefined;
        opened = span;

        // A session Faro's sampler left out is not sending, so its request names no trace.
        if (!span.isRecording()) {
            span.end();
            return undefined;
        }

        const { traceId, spanId, traceFlags } = span.spanContext();
        const traceparent = `00-${traceId}-${spanId}-${traceFlags.toString(16).padStart(2, "0")}`;

        return {
            headers: { traceparent },
            end: (failed) => {
                try {
                    // No message: the backend's span, in the same trace, carries the reason.
                    if (failed) span.setStatus({ code: ERROR });
                    span.end();
                } catch {}
            },
        };
    } catch {
        try {
            opened?.end();
        } catch {}

        return undefined;
    }
};

/**
 * Sends one request to the backend as a span named `name`, from when it is made until it is answered,
 * handing `send` the `traceparent` header that makes the backend's work its children.
 *
 * Before Faro starts, after an opt-out, in a build or launch that sends nothing, and whenever the span
 * cannot be opened, `send` is called with no headers - a request is worth more than its span. `send` is
 * called synchronously either way. The span carries nothing but its name, its timing, and whether the
 * request was refused.
 */
export const traced = <T>(name: string, send: (headers?: Record<string, string>) => Promise<T>): Promise<T> => {
    const span = openSpan(name);
    if (span === undefined) return send();

    let sent: Promise<T>;

    try {
        sent = send(span.headers);
    } catch (error) {
        span.end(true);
        throw error;
    }

    return sent.then(
        (value) => {
            span.end(false);
            return value;
        },
        (error: unknown) => {
            span.end(true);
            throw error;
        },
    );
};

/** For tests alone: a test that did not reset this would find the previous test's Faro still started. */
export const forgetFaro = () => {
    faro = undefined;
    paused = false;
    starting = false;
};

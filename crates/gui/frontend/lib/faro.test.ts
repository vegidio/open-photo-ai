import { initializeFaro } from "@grafana/faro-web-sdk";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { formatsOf } from "@/lib/events";
import { forgetFaro, pauseFaro, sendError, startFaro, traced, track } from "./faro";

// The one thing the window asks Rust for before it starts, answered with a fixed version here.
const version = vi.fn(async () => "26.10.0");

const TRACE_ID = "4bf92f3577b34da6a3ce929d0e0e4736";
const SPAN_ID = "00f067aa0ba902b7";

// The span a request opens, recording as a sampled session's does.
const span = {
    isRecording: vi.fn(() => true),
    spanContext: vi.fn(() => ({ traceId: TRACE_ID, spanId: SPAN_ID, traceFlags: 1 })),
    setStatus: vi.fn(),
    end: vi.fn(),
};
const tracer = { startSpan: vi.fn(() => span) };

// The instance `initializeFaro` hands back, and the calls made on it.
const instance = {
    api: {
        pushError: vi.fn(),
        pushEvent: vi.fn(),
        getOTEL: vi.fn((): unknown => ({ trace: { getTracer: () => tracer } })),
    },
    pause: vi.fn(),
};

vi.mock("@grafana/faro-web-sdk", () => ({
    initializeFaro: vi.fn(() => instance),
    // Classes, so `new` works; each records only that it was made.
    SessionInstrumentation: class {
        readonly kind = "session";
    },
    ViewInstrumentation: class {
        readonly kind = "view";
    },
}));

vi.mock("@grafana/faro-web-tracing", () => ({
    TracingInstrumentation: class {
        readonly kind = "tracing";
        constructor(readonly options: unknown) {}
    },
}));

const initialized = initializeFaro as unknown as Mock;

const COLLECTOR = "https://faro.example.grafana.net/collect/k";

/** Starts Faro in a build that names a collector, on a launch with Analytics on. */
const start = () => startFaro(true, version);

beforeEach(() => {
    forgetFaro();
    vi.clearAllMocks();
    vi.stubEnv("VITE_FARO_URL", COLLECTOR);
});

afterEach(() => vi.unstubAllEnvs());

describe("startFaro", () => {
    it("initialises nothing on a launch with Analytics off", async () => {
        await startFaro(false, version);

        expect(initialized).not.toHaveBeenCalled();
        expect(version).not.toHaveBeenCalled();
    });

    it("initialises nothing in a build that names no collector", async () => {
        vi.stubEnv("VITE_FARO_URL", "");

        await start();

        expect(initialized).not.toHaveBeenCalled();
    });

    it("initialises session, view and tracing only, isolated, with the build's collector", async () => {
        await start();

        expect(initialized).toHaveBeenCalledOnce();
        const config = initialized.mock.calls[0]?.[0];
        expect(config).toMatchObject({
            url: COLLECTOR,
            app: { name: "opai", version: "26.10.0" },
            sessionTracking: { enabled: true },
            isolate: true,
        });
        // The test run is not a production bundle.
        expect(config.app.environment).toBe("development");
        expect(config.instrumentations.map((i: { kind: string }) => i.kind)).toEqual(["session", "view", "tracing"]);
        // None of tracing's own: no fetch or XHR spans beside the requests `traced` names.
        expect(config.instrumentations[2].options).toEqual({ instrumentations: [] });
    });

    it("starts once, however often it is called", async () => {
        await Promise.all([start(), start()]);

        expect(initialized).toHaveBeenCalledOnce();
    });

    it("never starts once an opt-out has paused it, even when the version arrives after", async () => {
        const started = start();
        pauseFaro();
        await started;

        expect(initialized).not.toHaveBeenCalled();
    });
});

describe("track and sendError", () => {
    it("send nothing before Faro has started, or on a launch that does not start it", async () => {
        track("autopilot_run", { count: 3 });
        sendError(new Error("early"), { type: "warn", message: "m" });

        await startFaro(false, version);
        track("autopilot_run", { count: 3 });
        sendError(new Error("never"), { type: "warn", message: "m" });

        expect(instance.api.pushEvent).not.toHaveBeenCalled();
        expect(instance.api.pushError).not.toHaveBeenCalled();
    });

    it("send once started, with every attribute as text", async () => {
        await start();

        track("files_added", { count: 2, source: "drop", formats: formatsOf(["a.JPG", "b.png"]) });
        track("settings_saved", { changed: ["language", "processor"] });
        track("crop_applied", { rotated: true, flipped: false, has_ratio: false });

        expect(instance.api.pushEvent.mock.calls).toEqual([
            ["files_added", { count: "2", source: "drop", formats: "jpg,png" }],
            ["settings_saved", { changed: "language,processor" }],
            ["crop_applied", { rotated: "true", flipped: "false", has_ratio: "false" }],
        ]);

        const error = new Error("probe");
        sendError(error, { type: "warn", message: "the preview could not be drawn" });
        sendError(error, { type: "crash" });

        expect(instance.api.pushError.mock.calls).toEqual([
            [error, { context: { type: "warn", message: "the preview could not be drawn" } }],
            [error, { context: { type: "crash" } }],
        ]);
    });

    it("never throw into the caller when Faro does", async () => {
        await start();
        instance.api.pushEvent.mockImplementationOnce(() => {
            throw new Error("faro broke");
        });

        expect(() => track("update_opened", {})).not.toThrow();
    });
});

describe("pauseFaro", () => {
    it("pauses once, however often it is called, and stops everything after it", async () => {
        await start();

        pauseFaro();
        pauseFaro();
        track("autopilot_run", { count: 1 });
        sendError(new Error("after"), { type: "warn", message: "m" });

        expect(instance.pause).toHaveBeenCalledOnce();
        expect(instance.api.pushEvent).not.toHaveBeenCalled();
        expect(instance.api.pushError).not.toHaveBeenCalled();
    });

    it("is a no-op before start", () => {
        expect(() => pauseFaro()).not.toThrow();
        expect(instance.pause).not.toHaveBeenCalled();
    });
});

describe("traced", () => {
    /** A `send` that answers `value`, recording the headers it was handed. */
    const answering = (value: string) => vi.fn((_headers?: Record<string, string>) => Promise.resolve(value));

    it("sends with no headers before Faro has started", async () => {
        const send = answering("before");

        await expect(traced("enhance", send)).resolves.toBe("before");

        expect(send.mock.calls).toEqual([[]]);
        expect(tracer.startSpan).not.toHaveBeenCalled();
    });

    it("sends with no headers after an opt-out has paused it", async () => {
        await start();
        pauseFaro();
        const send = answering("after");

        await traced("enhance", send);

        expect(send.mock.calls).toEqual([[]]);
        expect(tracer.startSpan).not.toHaveBeenCalled();
    });

    it("sends the span's traceparent and ends it when the request is answered", async () => {
        await start();
        const send = answering("done");

        await expect(traced("enhance", send)).resolves.toBe("done");

        expect(tracer.startSpan).toHaveBeenCalledWith("enhance", { kind: 2 });
        expect(send.mock.calls).toEqual([[{ traceparent: `00-${TRACE_ID}-${SPAN_ID}-01` }]]);
        expect(span.setStatus).not.toHaveBeenCalled();
        expect(span.end).toHaveBeenCalledOnce();
    });

    it("marks the span failed, with no message, and ends it when the request is refused", async () => {
        await start();
        const refusal = new Error("the image is gone");

        await expect(traced("enhance", () => Promise.reject(refusal))).rejects.toBe(refusal);

        expect(span.setStatus.mock.calls).toEqual([[{ code: 2 }]]);
        expect(span.end).toHaveBeenCalledOnce();
    });

    it("names no trace for a session the sampler left out", async () => {
        await start();
        span.isRecording.mockReturnValueOnce(false);
        const send = answering("unsampled");

        await traced("enhance", send);

        expect(send.mock.calls).toEqual([[]]);
        expect(span.end).toHaveBeenCalledOnce();
    });

    it("falls back to the untraced call when the span cannot be opened", async () => {
        await start();
        tracer.startSpan.mockImplementationOnce(() => {
            throw new Error("tracer broke");
        });
        instance.api.getOTEL.mockReturnValueOnce(undefined);
        const send = answering("fallback");

        await traced("enhance", send);
        await traced("enhance", send);

        expect(send.mock.calls).toEqual([[], []]);
    });

    it("closes the span and falls back when the header cannot be built", async () => {
        await start();
        span.spanContext.mockImplementationOnce(() => {
            throw new Error("no context");
        });
        const send = answering("fallback");

        await traced("enhance", send);

        expect(send.mock.calls).toEqual([[]]);
        expect(span.end).toHaveBeenCalledOnce();
    });
});

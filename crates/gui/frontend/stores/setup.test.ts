import { beforeEach, describe, expect, it } from "vitest";
import {
    alreadyInstalled,
    apply,
    finished,
    PLAN,
    PROVIDERS,
    progress,
    REJECTION,
    resetSetupStore,
    SINGLE_PLAN,
} from "@/test/support";
import {
    installedCount,
    liveState,
    overallFraction,
    type SetupRow,
    settledState,
    stoppedRow,
    stoppedShape,
    useSetupStore,
} from "./setup";

const rows = () => useSetupStore.getState().rows;
const status = () => useSetupStore.getState().status;
const failure = () => useSetupStore.getState().failure;
const providers = () => useSetupStore.getState().providers;

describe("useSetupStore", () => {
    beforeEach(() => {
        resetSetupStore();
    });

    it("starts with nothing to draw", () => {
        expect(rows()).toEqual([]);
        expect(status()).toBe("idle");
    });

    it("draws every planned component as queued, in the order they will be installed", () => {
        apply(PLAN);

        expect(rows().map((row) => [row.name, row.state, row.fraction])).toEqual([
            ["ONNX Runtime", "queued", 0],
            ["NVIDIA CUDA", "queued", 0],
            ["NVIDIA cuDNN", "queued", 0],
            ["NVIDIA TensorRT", "queued", 0],
        ]);
    });

    it("stays idle on the plan alone, and starts running on the first report of work", () => {
        apply(PLAN);
        expect(status()).toBe("idle");

        // An already-installed component is not work: a launch with nothing to do must not open a
        // dialog, however briefly.
        apply(alreadyInstalled("ONNX Runtime"));
        expect(status()).toBe("idle");

        apply(progress("NVIDIA CUDA", "downloading", 0.02));
        expect(status()).toBe("running");
    });

    it("never opens on a launch where every component was already installed", () => {
        apply(PLAN, ...PLAN.rows.map((row) => alreadyInstalled(row.name)));

        expect(status()).toBe("idle");
        expect(rows().every((row) => row.state === "installed")).toBe(true);
    });

    it("keeps one row per component however many times it reports", () => {
        apply(
            PLAN,
            progress("NVIDIA CUDA", "downloading", 0.1),
            progress("NVIDIA CUDA", "downloading", 0.5),
            progress("NVIDIA CUDA", "extracting", 0.85),
        );

        expect(rows()).toHaveLength(PLAN.rows.length);

        // The most recent report, and the size still the plan's - a report carries none.
        const cuda = rows().find((row) => row.name === "NVIDIA CUDA");
        expect(cuda).toEqual({ name: "NVIDIA CUDA", size: 612_000_000, state: "extracting", fraction: 0.85 });
    });

    it("does not grow when something reports that the plan did not name", () => {
        // A model install reaches the same reporter. Nothing in initialization produces one, but the
        // list is fixed at the plan's length for the whole launch either way.
        apply(PLAN, progress("up_kyoto_4x_fp32", "downloading", 0.3));

        expect(rows()).toHaveLength(PLAN.rows.length);
    });

    it("leaves the rows alone when the command resolves or rejects", () => {
        apply(PLAN, progress("NVIDIA cuDNN", "downloading", 0.09));

        useSetupStore.getState().failed(REJECTION);
        expect(status()).toBe("failed");
        // The failing row is still the row it was, which is what the failure dialog marks.
        expect(rows().find((row) => row.name === "NVIDIA cuDNN")?.state).toBe("downloading");

        useSetupStore.setState({ status: "running" });
        useSetupStore.getState().succeeded(PROVIDERS);
        expect(status()).toBe("ready");
    });

    it("ignores a report that arrives after the command resolved", () => {
        // The channel's handler and the promise's resolution are two paths, so a late report is
        // expected rather than a fault. It must not put the application back into start-up.
        apply(PLAN);
        useSetupStore.getState().succeeded(PROVIDERS);

        apply(progress("NVIDIA TensorRT", "downloading", 0.2));

        expect(status()).toBe("ready");
    });

    it("keeps the provider report the command resolved with", () => {
        useSetupStore.getState().succeeded(PROVIDERS);

        expect(providers()).toEqual(PROVIDERS);
    });

    it("does not clear the provider report when a later attempt fails", () => {
        // A failed attempt does not un-discover what this machine offers. Nothing in the application
        // is set up at that moment, but the report describes the machine rather than the attempt.
        useSetupStore.getState().succeeded(PROVIDERS);

        useSetupStore.getState().failed(REJECTION);

        expect(status()).toBe("failed");
        expect(providers()).toEqual(PROVIDERS);
    });

    it("keeps the provider report across a retry", () => {
        // `retry` replaces the whole state rather than merging, which is how `failure` is made absent
        // rather than `undefined`. Everything it does not deliberately drop has to survive that, and
        // this is the one thing beside the rows that would be lost if it did not.
        useSetupStore.getState().succeeded(PROVIDERS);
        useSetupStore.getState().failed(REJECTION);

        useSetupStore.getState().retry();

        expect(status()).toBe("running");
        expect(providers()).toEqual(PROVIDERS);
    });

    it("has no provider report before a setup has succeeded", () => {
        // Absent rather than a machine with nothing on it: before start-up finishes, nothing has
        // decided the answer, and an empty report is indistinguishable from a machine that offers
        // nothing.
        apply(PLAN, progress("NVIDIA cuDNN", "downloading", 0.09));

        expect(providers()).toBeUndefined();
    });

    it("keeps the reason and the kind the rejection carried", () => {
        useSetupStore.getState().failed(REJECTION);

        expect(failure()).toEqual({ kind: "transfer", message: REJECTION.message });
    });

    it("reads a rejection that is not a SetupError as one that gets no advice and keeps the button", () => {
        // Nothing produces this today, but an `invoke` rejects with whatever crossed the boundary and
        // the promise carries no type for it. A launch that failed still has to be drawn.
        useSetupStore.getState().failed(new Error("the webview went away"));

        expect(failure()?.kind).toBe("other");
        expect(failure()?.message).toContain("the webview went away");
    });

    it("clears the failure and puts the dialog back when a retry starts", () => {
        apply(PLAN, progress("NVIDIA cuDNN", "downloading", 0.09));
        useSetupStore.getState().failed(REJECTION);

        useSetupStore.getState().retry();

        expect(status()).toBe("running");
        // Absent rather than `undefined`, which is what an optional property means under
        // `exactOptionalPropertyTypes` - and what `retry` has to do rather than merge a key in.
        expect("failure" in useSetupStore.getState()).toBe(false);
    });

    it("does nothing when there is no failure to leave behind", () => {
        useSetupStore.getState().retry();
        expect(status()).toBe("idle");

        apply(PLAN, ...PLAN.rows.map((row) => alreadyInstalled(row.name)));
        useSetupStore.getState().retry();
        expect(status()).toBe("idle");
    });

    it("stays running through a retry whose plan replaces the rows", () => {
        apply(PLAN, progress("NVIDIA cuDNN", "downloading", 0.09));
        useSetupStore.getState().failed(REJECTION);
        useSetupStore.getState().retry();

        // The retry's plan is the authority on what that attempt will do, and it arrives with the
        // status already `running` - so `apply`'s `idle` guard is not reached, and an attempt whose
        // every component reports `installed` does not fall back out of the dialog.
        apply(PLAN, ...PLAN.rows.map((row) => alreadyInstalled(row.name)));

        expect(status()).toBe("running");
        expect(rows().every((row) => row.state === "installed")).toBe(true);
    });
});

describe("stoppedRow", () => {
    beforeEach(() => {
        resetSetupStore();
    });

    it("is the one component that was working, and only that one", () => {
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            alreadyInstalled("NVIDIA CUDA"),
            progress("NVIDIA cuDNN", "downloading", 0.09),
        );

        expect(stoppedRow(rows())?.name).toBe("NVIDIA cuDNN");
    });

    it("is the component that was expanding, not only the one that was transferring", () => {
        apply(PLAN, progress("NVIDIA CUDA", "extracting", 0.85));

        expect(stoppedRow(rows())?.name).toBe("NVIDIA CUDA");
    });

    it("is not a component that finished by installing, which rests in a working state", () => {
        // The regression this guards. A component this launch installed comes to rest on `extracting`
        // at 1 - only one that was already on disk ever reports `installed` - so a test of the state
        // alone finds CUDA here and draws cuDNN, which actually stopped, as a component that was never
        // reached.
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            finished("NVIDIA CUDA"),
            progress("NVIDIA cuDNN", "downloading", 0.34),
        );

        expect(stoppedRow(rows())?.name).toBe("NVIDIA cuDNN");
    });

    it("is nothing when every component finished by installing", () => {
        // The same shape as the already-installed case below, reached the other way: nothing is in
        // flight, so the failure belongs to no component even though every row rests on `extracting`.
        apply(PLAN, ...PLAN.rows.map((row) => finished(row.name)));

        expect(stoppedRow(rows())).toBeUndefined();
    });

    it("is nothing when the failure belongs to no component", () => {
        // The common case on a machine where everything is already on disk and only the runtime will
        // not start.
        apply(PLAN, ...PLAN.rows.map((row) => alreadyInstalled(row.name)));

        expect(stoppedRow(rows())).toBeUndefined();
    });

    it("is nothing when the failure was raised before any component was reached", () => {
        expect(stoppedRow([])).toBeUndefined();

        apply(PLAN);
        expect(stoppedRow(rows())).toBeUndefined();
    });
});

describe("settledState", () => {
    const row = (state: SetupRow["state"], fraction: number): SetupRow => ({
        name: "NVIDIA CUDA",
        size: 612_000_000,
        state,
        fraction,
    });

    it("is the reported state while the component is still moving", () => {
        expect(settledState(row("queued", 0))).toBe("queued");
        expect(settledState(row("downloading", 0.41))).toBe("downloading");
        expect(settledState(row("extracting", 0.99))).toBe("extracting");
    });

    it("is installed once the component finished, whatever phase it finished in", () => {
        // A component comes to rest on the phase it finished in, so a row read as reported would go
        // on saying `Extracting` for the rest of the launch.
        expect(settledState(row("extracting", 1))).toBe("installed");
        expect(settledState(row("downloading", 1))).toBe("installed");
        expect(settledState(row("installed", 1))).toBe("installed");
    });
});

describe("liveState", () => {
    beforeEach(() => {
        resetSetupStore();
    });

    const named = (name: string) => {
        const row = rows().find((candidate) => candidate.name === name);
        if (!row) throw new Error(`no row named ${name}`);

        return liveState(rows(), row);
    };

    it("holds a finished component's label until its successor starts", () => {
        // The gap: CUDA has landed on 1 in the phase it finished in and cuDNN has reported nothing
        // yet. Read as settled, CUDA would say `Installed` here and cuDNN would sit at `Queued` for
        // as long as the library takes to check the next record and open its connection - a list with
        // nothing happening in it.
        apply(PLAN, alreadyInstalled("ONNX Runtime"), finished("NVIDIA CUDA"));

        expect(named("NVIDIA CUDA")).toBe("extracting");
        expect(named("NVIDIA cuDNN")).toBe("queued");
    });

    it("changes both rows on the report that starts the next component", () => {
        apply(PLAN, alreadyInstalled("ONNX Runtime"), finished("NVIDIA CUDA"));
        apply(progress("NVIDIA cuDNN", "downloading", 0.02));

        // One report, both changes: the hand-off is one moment on screen rather than two with a
        // stall between them.
        expect(named("NVIDIA CUDA")).toBe("installed");
        expect(named("NVIDIA cuDNN")).toBe("downloading");
    });

    it("holds nothing for a component that was already on disk", () => {
        // Its reported state is `installed` already, so the hold has nothing to show - which is what
        // keeps a launch with nothing to do from drawing a working row.
        apply(PLAN, alreadyInstalled("ONNX Runtime"));

        expect(named("ONNX Runtime")).toBe("installed");
    });

    it("installs the last component as soon as it finishes, having nothing to hand over to", () => {
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            finished("NVIDIA CUDA"),
            finished("NVIDIA cuDNN"),
            finished("NVIDIA TensorRT"),
        );

        expect(named("NVIDIA TensorRT")).toBe("installed");
    });

    it("does not hold a row the failure dialog would then blame", () => {
        // A failure raised during a hand-off belongs to the component being started, or to no
        // component - never to the one that had just succeeded. `stoppedRow` reads settled for
        // exactly this, so the two readings disagree here on purpose.
        apply(PLAN, alreadyInstalled("ONNX Runtime"), finished("NVIDIA CUDA"));

        expect(named("NVIDIA CUDA")).toBe("extracting");
        expect(stoppedRow(rows())).toBeUndefined();
    });
});

describe("stoppedShape", () => {
    beforeEach(() => {
        resetSetupStore();
    });

    it("is the one-component shape on a plan with a single row", () => {
        // "The first one" implies there are others to be first of.
        apply(SINGLE_PLAN, progress("ONNX Runtime", "downloading", 0.4));

        expect(stoppedShape(rows())).toBe("only");
    });

    it("is the nothing-finished shape when the first component is the one that stopped", () => {
        apply(PLAN, progress("ONNX Runtime", "downloading", 0.12));

        expect(stoppedShape(rows())).toBe("first");
    });

    it("is the nothing-left shape when the component that stopped is the last one", () => {
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            finished("NVIDIA CUDA"),
            finished("NVIDIA cuDNN"),
            progress("NVIDIA TensorRT", "downloading", 0.5),
        );

        expect(stoppedShape(rows())).toBe("last");
    });

    it("is the something-left shape when components behind it were never reached", () => {
        apply(PLAN, alreadyInstalled("ONNX Runtime"), progress("NVIDIA CUDA", "downloading", 0.41));

        expect(stoppedShape(rows())).toBe("rest");
    });

    it("is nothing when the failure belongs to no component", () => {
        // Nothing stopped part way, so there is no second sentence to add - the header carries its
        // first alone, in the two lines it reserves either way.
        apply(PLAN, ...PLAN.rows.map((row) => alreadyInstalled(row.name)));
        expect(stoppedShape(rows())).toBeUndefined();

        expect(stoppedShape([])).toBeUndefined();
    });
});

describe("overallFraction", () => {
    const row = (size: number, fraction: number): SetupRow => ({
        name: `${size}`,
        size,
        state: fraction >= 1 ? "installed" : "downloading",
        fraction,
    });

    it("weights by size rather than by count", () => {
        // A 1 MB component finishing must not advance the bar as far as a 612 MB one does.
        const small = overallFraction([row(1_000_000, 1), row(612_000_000, 0)]);
        const large = overallFraction([row(1_000_000, 0), row(612_000_000, 1)]);

        expect(small).toBeCloseTo(1_000_000 / 613_000_000, 10);
        expect(large).toBeCloseTo(612_000_000 / 613_000_000, 10);
        expect(small).toBeLessThan(large);
    });

    it("counts an already-installed component as its whole size", () => {
        // Which is what stops the bar starting at zero on a launch that has most of its work done.
        expect(overallFraction([row(184_000_000, 1), row(612_000_000, 0)])).toBeCloseTo(184 / 796, 10);
    });

    it("never decreases while its inputs do not", () => {
        apply(PLAN);
        const seen: number[] = [overallFraction(rows())];

        for (const event of [
            alreadyInstalled("ONNX Runtime"),
            progress("NVIDIA CUDA", "downloading", 0.4),
            progress("NVIDIA CUDA", "extracting", 0.8),
            progress("NVIDIA CUDA", "extracting", 1),
            progress("NVIDIA cuDNN", "downloading", 0.5),
            progress("NVIDIA cuDNN", "extracting", 1),
            progress("NVIDIA TensorRT", "downloading", 1),
        ]) {
            apply(event);
            seen.push(overallFraction(rows()));
        }

        expect(seen.every((value, index) => index === 0 || value >= (seen[index - 1] ?? 0))).toBe(true);
        expect(seen.at(0)).toBe(0);
        expect(seen.at(-1)).toBe(1);
    });

    it("is zero before the plan arrives rather than a division by nothing", () => {
        expect(overallFraction([])).toBe(0);
    });
});

describe("installedCount", () => {
    beforeEach(() => {
        resetSetupStore();
    });

    it("counts what is already there in the denominator", () => {
        // Two of four were already on disk. The dialog still says four, so the sentence does not
        // change between two launches of the same application on the same machine.
        apply(PLAN, alreadyInstalled("ONNX Runtime"), alreadyInstalled("NVIDIA CUDA"));

        expect(installedCount(rows())).toBe(2);
        expect(rows()).toHaveLength(4);
    });

    it("counts a component that just finished, not one part way through", () => {
        apply(PLAN, progress("ONNX Runtime", "extracting", 1), progress("NVIDIA CUDA", "downloading", 0.99));

        expect(installedCount(rows())).toBe(1);
    });
});

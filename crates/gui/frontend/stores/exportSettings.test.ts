import { beforeEach, describe, expect, it } from "vitest";
import { exportSettingsData, exportSettingsDefaults, useExportSettingsStore } from "./exportSettings";

/** Seeds `localStorage` with a stored state and rehydrates the store from it, as `settings.test.ts` does. */
const stored = async (state: Record<string, unknown>) => {
    localStorage.setItem("export-storage", JSON.stringify({ state, version: 0 }));

    await useExportSettingsStore.persist.rehydrate();
};

const data = () => exportSettingsData(useExportSettingsStore.getState());

beforeEach(() => {
    localStorage.clear();
    useExportSettingsStore.setState(useExportSettingsStore.getInitialState(), true);
});

describe("the export settings", () => {
    it("start at the reference's defaults: no affixes, the original directory, no overwriting, PNG", () => {
        expect(data()).toEqual({ prefix: "", suffix: "", overwrite: false, format: "png" });
        expect(exportSettingsDefaults()).toEqual(data());
        expect("location" in useExportSettingsStore.getState()).toBe(false);
    });

    it("round-trip through storage", async () => {
        const { setPrefix, setSuffix, setLocation, setOverwrite, setFormat } = useExportSettingsStore.getState();
        setPrefix("new-");
        setSuffix("-opai");
        setLocation("/exports");
        setOverwrite(true);
        setFormat("webp");

        const written = JSON.parse(localStorage.getItem("export-storage") ?? "{}");
        expect(written.state).toEqual({
            prefix: "new-",
            suffix: "-opai",
            location: "/exports",
            overwrite: true,
            format: "webp",
        });

        // A reset writes the defaults through `persist` too, so what was written is put back before the next launch.
        useExportSettingsStore.setState(useExportSettingsStore.getInitialState(), true);
        await stored(written.state);

        expect(data()).toEqual(written.state);
    });

    it("go back to the original directory, leaving no location behind", () => {
        useExportSettingsStore.getState().setLocation("/exports");
        useExportSettingsStore.getState().setLocation(undefined);

        expect("location" in useExportSettingsStore.getState()).toBe(false);
        expect(typeof useExportSettingsStore.getState().setFormat).toBe("function");
    });

    it("read the reference's jpg as jpeg", async () => {
        await stored({ prefix: "", suffix: "", overwrite: false, format: "jpg" });

        expect(useExportSettingsStore.getState().format).toBe("jpeg");
    });

    it("read a format this application has no name for as PNG", async () => {
        await stored({ prefix: "", suffix: "", overwrite: false, format: "jxl" });

        expect(useExportSettingsStore.getState().format).toBe("png");
    });

    it("read a location that is not a path as the original directory, and repair the other fields' shapes", async () => {
        await stored({ prefix: 4, suffix: null, location: 12, overwrite: "yes", format: "preserve" });

        expect(data()).toEqual({ prefix: "", suffix: "", overwrite: false, format: "preserve" });
    });
});

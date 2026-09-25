// Vite's ambient declarations, which is what teaches `tsc` that `import "./style.css"` resolves to
// something. `noUncheckedSideEffectImports` in tsconfig.json exists to catch a bare import of a file
// that is not there - a typo'd stylesheet path otherwise fails silently - and without this reference
// it cannot tell that case apart from a stylesheet Vite handles. It also declares `import.meta.env`.
/// <reference types="vite/client" />

/** The variables this application reads from the build's environment. Each is absent from a build that set none. */
// biome-ignore lint/style/useConsistentTypeDefinitions: this adds to Vite's own `ImportMetaEnv`, which only declaration merging can do, and a `type` cannot merge.
interface ImportMetaEnv {
    /** The Grafana Faro collector's URL, key included, from the build's secrets. Without it the window sends nothing. */
    readonly VITE_FARO_URL?: string;
}

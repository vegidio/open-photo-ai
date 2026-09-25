//! Generates the Tauri context, the capability schemas and the platform resources.

fn main() {
    // `try_build` rather than `build()` for one reason: the Windows app manifest is taken away from
    // Tauri here and embedded by `embed_common_controls_manifest` below, so that the *test*
    // executables get it too. See that function for why they must. Tauri's copy has to go, because two
    // `RT_MANIFEST` resources in one image is a link failure; it carries nothing else - no DPI awareness,
    // no OS compatibility list - so `windows-app-manifest.xml` replaces it whole rather than shadowing
    // part of it.
    //
    // The failure handling is `build()`'s own, kept because its hint is worth having: the usual cause
    // of an "unknown field" here is a `tauri.conf.json` written by a CLI newer than this crate.
    if let Err(error) = tauri_build::try_build(
        tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest()),
    ) {
        let error = format!("{error:#}");
        println!("{error}");
        if error.starts_with("unknown field") {
            println!(
                "found an unknown configuration field, which usually means the Tauri CLI is newer \
                 than `tauri-build`; try `cargo update` in crates/gui"
            );
        }
        std::process::exit(1);
    }

    embed_common_controls_manifest();
}

/// Embed the Common Controls v6 manifest into **every** executable this crate links - the binary and
/// the test harness alike. A no-op everywhere but MSVC.
fn embed_common_controls_manifest() {
    // **Without it the tests do not start on Windows.** Tauri embeds this manifest through a Windows
    // resource file, and a resource reaches the bin target alone; a test harness is its own executable
    // and gets none. The manifest's one job is to declare a dependency on Common Controls **v6**, and
    // with no manifest the loader binds the process to the v5 `comctl32.dll` in System32, which does not
    // export `TaskDialogIndirect`. The Tauri runtime linked in by the `test` feature imports that
    // function statically, so the image is refused before `main` runs and the process dies with
    // `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139) - no panic, no backtrace, and `cargo test` able to
    // report only that the binary did not exit successfully.
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");

    // Read from the environment rather than `cfg!`: a build script is compiled for the host, so
    // `cfg!(windows)` would answer for the machine doing the building and not for the machine the
    // executable has to load on. MSVC alone, because `/MANIFEST` is that linker's flag and no other
    // target wants it.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }

    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"))
        .join("windows-app-manifest.xml");

    // `cargo:rustc-link-arg` rather than one of its per-target spellings, and the choice is forced.
    // `-bins` reaches the binary and not the tests, which is the state this exists to correct;
    // `-tests` is rejected outright by Cargo unless the package has an integration test target, and this
    // one's tests are the lib's own `#[cfg(test)]` modules. The unscoped form reaches both.
    //
    // `/MANIFESTINPUT` is only read when the manifest is being embedded, so the two travel together.
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}

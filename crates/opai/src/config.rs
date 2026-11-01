//! Resolving the application's configuration directory, and removing what earlier releases left in it.

use std::path::{Path, PathBuf};

use rust_sak::fs;

use crate::error::InitError;

/// Resolves and creates `<platform config dir>/<name>`, the directory every dependency installs beneath.
///
/// `~/.config/<name>` on Linux, `~/Library/Application Support/<name>` on macOS, `%LOCALAPPDATA%\<name>` on Windows — so
/// the same `name` has to be passed on every run for an installation to be found again.
///
/// # Errors
///
/// Returns [`InitError::InvalidName`] if `name` is not a usable directory segment, and [`InitError::Fs`] if the
/// platform has no configuration directory or it could not be created.
pub(crate) fn app_dir(name: &str) -> Result<PathBuf, InitError> {
    validate_name(name)?;
    Ok(fs::mk_user_config_dir(name, "")?)
}

/// Deletes `%APPDATA%\<name>`, the roaming directory releases before `rust-sak` moved the configuration root to
/// `%LOCALAPPDATA%` installed into, so its models and libraries do not linger on disk.
///
/// Best effort: a directory that is absent is the ordinary case, and one that cannot be removed is recorded and
/// otherwise ignored, since nothing this run needs lives there.
// TODO: Remove this, and its call in `Opai::initialize`, once users have had time to upgrade past the release that
// moved the directory.
#[cfg(target_os = "windows")]
pub(crate) fn remove_roaming_dir(name: &str, app_dir: &Path) {
    let Some(old) = dirs::config_dir().map(|root| root.join(name)) else {
        return;
    };

    if old == app_dir || !old.exists() {
        return;
    }

    match std::fs::remove_dir_all(&old) {
        Ok(()) => tracing::info!(path = %old.display(), "removed the application directory older releases used"),
        Err(error) => tracing::warn!(
            path = %old.display(),
            %error,
            "the application directory older releases used could not be removed"
        ),
    }
}

/// The folders releases before the shared dependency root installed the runtime and the NVIDIA libraries into, inside
/// the application's directory.
// TODO: remove once users have upgraded past this release
const OLD_INSTALL_DIRS: [&str; 2] = ["runtime", "libs"];

/// Deletes `<app_dir>/runtime` and `<app_dir>/libs`, where releases before the shared dependency root installed the
/// ONNX Runtime and the NVIDIA libraries, so their gigabytes do not stay on disk unused. Nothing else in `app_dir` is
/// touched.
///
/// Best effort: a folder that is absent is the ordinary case, and one that cannot be removed is recorded and
/// otherwise ignored, since nothing this run needs lives there.
// TODO: remove once users have upgraded past this release
pub(crate) fn remove_old_install_dirs(app_dir: &Path) {
    for old in OLD_INSTALL_DIRS.map(|sub| app_dir.join(sub)) {
        match std::fs::remove_dir_all(&old) {
            Ok(()) => tracing::info!(path = %old.display(), "removed the install folder earlier releases used"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(
                path = %old.display(),
                %error,
                "the install folder earlier releases used could not be removed"
            ),
        }
    }
}

/// Resolves and creates `<app_dir>/<sub>`.
///
/// `sub` may name nested directories (`libs/cuda`); what it may not do is escape its parent.
///
/// # Errors
///
/// Returns [`InitError::InvalidName`] if `name` is not a usable directory segment, [`InitError::Fs`] if `sub` could
/// escape its parent, and [`InitError::Io`] if the directory could not be created.
pub(crate) fn sub_dir(app_dir: &Path, name: &str, sub: &str) -> Result<PathBuf, InitError> {
    validate_name(name)?;
    // The escape rule is `rust-sak`'s rather than a second copy of it here: `fs::user_config_dir` applies the same
    // validation archive entries get and creates nothing, so only its verdict is taken and the path itself is rebuilt
    // under `app_dir` — which is a temporary directory under test and the real configuration directory otherwise.
    fs::user_config_dir(name, sub)?;

    let path = app_dir.join(sub);
    std::fs::create_dir_all(&path).map_err(InitError::io(&path))?;
    Ok(path)
}

/// Rejects an application name that cannot be one directory segment.
pub(crate) fn validate_name(name: &str) -> Result<(), InitError> {
    // Stricter than the validation `rust-sak` applies, and deliberately so: it accepts `a/b` as two nested components,
    // which is right for an archive entry and wrong for an application name — `~/.config/a/b` would silently become a
    // second application's directory. Callers run this *before* anything is created, so a rejected name leaves
    // nothing on disk.
    let reject = |reason: &'static str| Err(InitError::InvalidName { name: name.to_string(), reason });

    match name {
        "" => reject("it is empty"),
        "." | ".." => reject("it names a directory relative to another one"),
        // Both separators, on every platform: a name is written once and has to mean the same directory everywhere,
        // and Windows resolves `/` as a separator too.
        _ if name.contains('/') || name.contains('\\') => reject("it contains a path separator"),
        _ if name.contains('\0') => reject("it contains a NUL byte"),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unwraps the name an [`InitError::InvalidName`] rejected, failing the test on any other variant.
    fn rejected_name(error: InitError) -> String {
        match error {
            InitError::InvalidName { name, .. } => name,
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }

    #[test]
    fn a_name_that_is_not_one_directory_segment_is_rejected() {
        for name in ["", ".", "..", "a/b", "a\\b", "../escape", "/absolute"] {
            let error = app_dir(name).unwrap_err();
            assert_eq!(rejected_name(error), name, "expected {name:?} to be rejected by name");
        }
    }

    #[test]
    fn a_rejected_name_creates_nothing() {
        // The spec's promise for a bad name is that nothing happens on disk at all, so the check has to come before
        // the directory is created rather than after.
        let root = tempfile::tempdir().unwrap();
        assert!(sub_dir(root.path(), "..", "runtime").is_err());

        let entries: Vec<_> = std::fs::read_dir(root.path()).unwrap().collect();
        assert!(entries.is_empty(), "a rejected name left something behind");
    }

    #[test]
    fn a_sub_directory_is_resolved_and_created_under_the_root() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("opai-test");

        let runtime = sub_dir(&app, "opai-test", "runtime").unwrap();

        assert_eq!(runtime, app.join("runtime"));
        assert!(runtime.is_dir());
    }

    #[test]
    fn resolving_an_existing_sub_directory_again_succeeds() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("opai-test");

        let first = sub_dir(&app, "opai-test", "runtime").unwrap();
        std::fs::write(first.join("keep.txt"), b"kept").unwrap();
        let second = sub_dir(&app, "opai-test", "runtime").unwrap();

        assert_eq!(first, second);
        assert!(second.join("keep.txt").exists(), "an existing directory was replaced");
    }

    #[test]
    fn a_sub_path_that_would_escape_its_parent_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("opai-test");

        let error = sub_dir(&app, "opai-test", "../../escape").unwrap_err();
        assert!(matches!(error, InitError::Fs(_)), "got {error:?}");
        assert!(!root.path().join("escape").exists());
    }

    #[test]
    fn the_app_directory_is_named_after_the_application() {
        // Against the real configuration directory, since that is the lookup being checked; the name is unique
        // enough not to collide with anything, and the directory is removed again.
        let dir = app_dir("opai-config-test").unwrap();

        assert!(dir.is_dir());
        assert!(dir.ends_with("opai-config-test"));

        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn the_old_install_folders_are_removed_and_nothing_else_is() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path();
        for kept in ["models/up_kyoto_4x_fp32", "cache", "engines"] {
            std::fs::create_dir_all(app.join(kept)).unwrap();
        }
        std::fs::write(app.join("settings.json"), b"{}").unwrap();
        std::fs::create_dir_all(app.join("runtime")).unwrap();
        std::fs::write(app.join("runtime/libonnxruntime.1.30.0.dylib"), b"old").unwrap();
        std::fs::create_dir_all(app.join("libs/cuda")).unwrap();
        std::fs::write(app.join("libs/cuda/libcudart.so.13"), b"old").unwrap();

        let (log, ()) = crate::logging::records_of_blocking("info", || remove_old_install_dirs(app));

        assert!(!app.join("runtime").exists());
        assert!(!app.join("libs").exists());
        for kept in ["models/up_kyoto_4x_fp32", "cache", "engines", "settings.json"] {
            assert!(app.join(kept).exists(), "{kept} was removed");
        }
        assert_eq!(
            crate::logging::records(&log, "removed the install folder earlier releases used").len(),
            2,
            "{log}"
        );

        // A second launch finds nothing to remove, and says nothing.
        let (log, ()) = crate::logging::records_of_blocking("info", || remove_old_install_dirs(app));
        assert!(!log.contains("install folder"), "{log}");
    }

    #[test]
    fn an_old_folder_that_cannot_be_removed_is_recorded_and_initialization_goes_on() {
        // A file where the folder would be, inside a folder that cannot be written to, is a removal that fails on every
        // platform without needing elevated rights to set up.
        let root = tempfile::tempdir().unwrap();
        let app = root.path();
        std::fs::create_dir_all(app.join("runtime/locked")).unwrap();
        std::fs::write(app.join("runtime/locked/file"), b"x").unwrap();
        let locked = app.join("runtime/locked");
        let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&locked, permissions.clone()).unwrap();

        let (log, ()) = crate::logging::records_of_blocking("info", || remove_old_install_dirs(app));

        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "restoring the folder so the temporary directory can go"
        )]
        permissions.set_readonly(false);
        std::fs::set_permissions(&locked, permissions).unwrap();

        if cfg!(unix) {
            let failed = crate::logging::records(&log, "the install folder earlier releases used could not be removed");
            assert_eq!(failed.len(), 1, "{log}");
            assert!(failed[0].contains("level=WARN"), "{}", failed[0]);
        }
    }
}

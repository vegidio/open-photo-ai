//! What a file becomes when the user names it, the set of files this application has admitted, and the
//! commands the window reaches both through.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::decoded::Decoded;
use crate::command::{Answer, CommandError, Ended, Traceparent, command_span, traced, traced_sync};
use crate::sync::lock;
use crate::task::spawn_blocking;

// ── What a file becomes ───────────────────────────────────────────────────────────────────────────────────────────

/// One image file, described without decoding it: where it is, its identity, dimensions and size.
///
/// `identity`, `width`, `height` and `size` are `None` where they could not be read, not `0`, and the
/// frontend sees them as absent properties rather than nulls.
///
/// `width` and `height` come from one probe and so are either both present or both absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageRecord {
    // Text, not a path: this is what a drag-and-drop hands over, and what `reveal_image` is compared against. The
    // registry keeps the real `PathBuf`.
    /// Where the file is, rendered lossily if not UTF-8.
    pub(super) path: String,
    /// XXH3-64 of the whole file, matching what [`opai::image::load`] computes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) identity: Option<String>,
    /// The photograph's width in pixels, from the file's header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) width: Option<u32>,
    /// The photograph's height in pixels, from the file's header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) height: Option<u32>,
    /// The file's extension, lowercase and without its dot. Empty where the name has none.
    pub(super) extension: String,
    /// The file's size on disk, in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) size: Option<u64>,
}

impl ImageRecord {
    /// Describes the file at `path` on the calling thread, each read failing independently: the hash over
    /// the whole file, the header for dimensions, and the metadata for size.
    pub(super) fn describe(path: &Path) -> Self {
        // Each read fails on its own: a selection is a batch, and one bad file shouldn't cost the rest.
        //
        // A failure is not logged here: `opai` records it, naming the file and the reason — which is what tells a
        // truncated header from a file on an ejected drive, two things that look identical to the user.
        let (identity, dimensions) = if opai::image::is_raw(path) {
            // The RAW probe needs the whole file rather than a header, and so does the hash, so both come from one
            // read rather than two — a folder of 40 MB RAWs would otherwise be read twice over.
            match opai::image::identify_raw_blocking(path) {
                Ok((identity, info)) => (Some(identity), info.ok().map(|info| (info.width, info.height))),
                Err(_) => (None, None),
            }
        } else {
            (identity_of(path), opai::image::probe_blocking(path).ok().map(|info| (info.width, info.height)))
        };

        Self {
            path: path.to_string_lossy().into_owned(),
            identity,
            width: dimensions.map(|(width, _)| width),
            height: dimensions.map(|(_, height)| height),
            extension: path.extension().map(|extension| extension.to_string_lossy().to_lowercase()).unwrap_or_default(),
            size: std::fs::metadata(path).ok().map(|metadata| metadata.len()),
        }
    }
}

/// The identity of the file at `path`, or `None` where it could not be read — which `opai` records.
fn identity_of(path: &Path) -> Option<String> {
    // A hash and not a decode, which is why describing hundreds of files stays cheap.
    opai::image::identity_blocking(path).ok()
}

// ── The set of files the user has opened ──────────────────────────────────────────────────────────────────────────

/// Every file this application has admitted — the only door a request or a reveal can reach a file
/// through. [`open_images`] and [`describe_images`] are its sole writers.
///
/// Nothing is ever removed.
#[derive(Debug, Default)]
pub(crate) struct Opened {
    // Nothing is removed because a path and a 16-character string per file is noise.
    admitted: Mutex<Admitted>,
    /// The admitted photographs most recently decoded. Beside the registry because every reader of an admitted
    /// file's pixels already holds this, and it bounds itself — see [`Decoded`].
    decoded: Decoded,
}

/// The inside of [`Opened`], behind its one lock.
#[derive(Debug, Default)]
struct Admitted {
    // Two collections because the two doors key differently: `super::serve::serve` needs an **identity** (the only
    // honest key for pixels), while `reveal_image` needs a **path** (a file manager opens a location, and identity
    // is absent for exactly the unreadable files a user most wants to find on disk).
    //
    // One struct rather than a mutex per collection because admitting a file writes all of it at once, which is
    // what makes "a file with an identity is in both" hold for every reader, with no moment between two locks where
    // it is in one and not yet the other. Same judgement as `crate::enhance::slot::Runs`.
    /// Where the file with each identity is. What [`super::serve::serve`] resolves against.
    files: HashMap<String, PathBuf>,
    /// Every path this application has described, readable or not. What [`reveal_image`] checks against.
    paths: HashSet<PathBuf>,
}

impl Opened {
    /// Admits every described file by path (so it can be revealed) and every one carrying an identity (so
    /// [`super::serve::serve`] can resolve it). A file with no identity is admitted by path only, since
    /// nothing can address its pixels.
    ///
    /// Re-admitting a known identity overwrites its path: the same photograph opened from a second
    /// location is served from wherever it was most recently pointed at.
    pub(super) fn admit(&self, described: &[(ImageRecord, PathBuf)]) {
        let mut admitted = lock(&self.admitted);

        for (record, path) in described {
            admitted.paths.insert(path.clone());

            if let Some(identity) = &record.identity {
                admitted.files.insert(identity.clone(), path.clone());
            }
        }
    }

    /// Where the file with this identity is, or `None` if this application never admitted it.
    pub(crate) fn resolve(&self, identity: &str) -> Option<PathBuf> {
        lock(&self.admitted).files.get(identity).cloned()
    }

    /// The cache every reader of an admitted file's pixels decodes through.
    pub(crate) fn decoded(&self) -> &Decoded {
        &self.decoded
    }

    /// Whether this application opened the file at this path. Compared verbatim, not canonicalized.
    fn knows(&self, path: &Path) -> bool {
        // Not canonicalized because both sides come from the same string already.
        lock(&self.admitted).paths.contains(path)
    }
}

// ── The commands ──────────────────────────────────────────────────────────────────────────────────────────────────
//
// Every command's name is duplicated in `frontend/ipc/images.ts`; nothing checks the two stay in sync.

// Shaped as `crate::logs::LogsError` is.
/// Why one of this module's commands could not answer. One variant per command.
///
/// A single file's failure never reaches here — it's folded into its [`ImageRecord`] instead — so what's
/// left is the runtime shutting down mid-describe, or an unauthorized reveal.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum ImagesError {
    /// The files the user chose could not be described, the application having closed while it happened.
    #[error("{message}")]
    OpenImages {
        /// What went wrong, in full, so it can be copied into a bug report.
        message: String,
    },

    /// The files could not be described, which in practice means the application was closing while it
    /// happened.
    #[error("{message}")]
    DescribeImages {
        /// What went wrong, in full.
        message: String,
    },

    // One variant, since both look the same to the window — the log records which it was.
    /// The image could not be shown in the file manager: either it's not one this application opened, or
    /// the file manager wouldn't open it.
    #[error("{message}")]
    RevealImage {
        /// What went wrong, in full.
        message: String,
    },
}

impl CommandError for ImagesError {
    fn ended(&self) -> Ended<'_> {
        match self {
            // A describe fails only when the runtime shut down under it: the application is closing.
            Self::OpenImages { .. } | Self::DescribeImages { .. } => Ended::Stopped,
            Self::RevealImage { .. } => Ended::Failed { error: self, recorded: false },
        }
    }
}

impl Answer for Vec<ImageRecord> {}

/// Ask the user for image files, and describe the ones they chose.
///
/// Filters the platform picker to every format [`opai::image::load`] can open. `title` and `filter_name`
/// arrive pre-translated.
///
/// **Dismissing the picker is an empty selection, not an error** — and so is a picker that failed to
/// open. Both are logged, and the user added no images either way.
///
/// # Errors
///
/// [`ImagesError::OpenImages`] if the chosen files could not be described, i.e. the application closed
/// mid-selection.
#[tauri::command]
pub(crate) async fn open_images(
    app: AppHandle,
    title: String,
    filter_name: String,
    opened: State<'_, Opened>,
    traceparent: Traceparent,
) -> Result<Vec<ImageRecord>, ImagesError> {
    traced(command_span!("open_images", traceparent), async move {
        // `async` is required here: the dialog plugin's blocking call must run off the main thread, while the
        // native picker itself must be opened *on* it. A synchronous command would deadlock — the same
        // thread-shape issue `src/reveal.rs` records for `tauri-plugin-opener`.
        let extensions = opai::image::input_extensions();

        // Only the word is translated: the frontend owns the catalogue, and this crate doesn't keep a second one. The
        // extension list stays derived from the decoder, so no second list of extensions lives in the frontend.
        let picked = app
            .dialog()
            .file()
            .set_title(&title)
            .add_filter(&filter_name, &extensions)
            .blocking_pick_files()
            // `None` is a dismissal or a picker that failed to open, with no way to tell the two apart; nothing at
            // the interface branches on the difference.
            .unwrap_or_default();

        // into_path refuses content:// URIs (an Android-only concern); skip rather than fail the whole selection.
        let paths: Vec<PathBuf> = picked.into_iter().filter_map(|file| file.into_path().ok()).collect();

        if paths.is_empty() {
            // Dismissed, never opened, or everything refused by into_path — not knowable here, so name them all.
            tracing::info!(
                "the file picker added nothing: it was dismissed, could not be opened, or held nothing usable"
            );
            return Ok(Vec::new());
        }

        describe_and_admit(paths, &opened).await.map_err(|message| ImagesError::OpenImages { message })
    })
    .await
}

/// Describe image files the user supplied some other way, and admit them.
///
/// The other route in: a drag onto the window (a `tauri://drag-drop` event carrying paths), and later a
/// file named on the command line. The records are identical to [`open_images`]' for the same files.
///
/// No extension filtering here: a file that isn't an image is described as far as possible and reported
/// with no dimensions, the same answer a truncated photograph gets.
///
/// # Errors
///
/// [`ImagesError::DescribeImages`] if the runtime shut down before the files could be described.
#[tauri::command]
pub(crate) async fn describe_images(
    paths: Vec<String>,
    opened: State<'_, Opened>,
    traceparent: Traceparent,
) -> Result<Vec<ImageRecord>, ImagesError> {
    traced(command_span!("describe_images", traceparent), async move {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();

        describe_and_admit(paths, &opened)
            .await
            .map_err(|message| ImagesError::DescribeImages { message })
    })
    .await
}

/// Show an opened image to the user, in the platform's own file manager, with the file selected.
///
/// Takes the path from an [`ImageRecord`] the window already holds, and **refuses one [`Opened`] doesn't
/// know**.
///
/// # Errors
///
/// [`ImagesError::RevealImage`] for a path this application never opened, and for a file manager that
/// wouldn't open — including a file since moved or deleted, which the plugin reports because it
/// canonicalizes the path before revealing it.
#[tauri::command]
pub(crate) async fn reveal_image<R: tauri::Runtime>(
    app: AppHandle<R>,
    path: String,
    opened: State<'_, Opened>,
    traceparent: Traceparent,
) -> Result<(), ImagesError> {
    // The same entitlement the `opai://` scheme establishes, applied to the other door out of the window. By path
    // rather than by identity, unlike `super::serve::serve`: identity is absent exactly for files that couldn't be
    // read, which are still open images and the ones a user is most likely to want to look at on disk.
    traced(command_span!("reveal_image", traceparent), async move {
        let path = PathBuf::from(path);

        // A bug in the interface, since the window can only name a file it was given, and it's invisible from the
        // frontend's side of it. The command wrapper records it, which is why the refusal names the path.
        if !opened.knows(&path) {
            return Err(ImagesError::RevealImage {
                message: format!("`{}` is not an image this application has opened", path.display()),
            });
        }

        // The reveal itself is `crate::reveal`'s (the plugin's, with a thread picked for it); this command adds the
        // entitlement check above and the error message.
        crate::reveal::reveal(&app, path).await.map_err(|error| ImagesError::RevealImage {
            message: format!("the image could not be shown in the file manager: {error}"),
        })
    })
    .await
}

/// The file extensions this application opens, lowercase and without their dots.
///
/// The list is compiled in, so it can't change while the application runs.
#[tauri::command]
pub(crate) fn input_extensions(traceparent: Traceparent) -> Vec<&'static str> {
    // The picker filters on `opai::image::input_extensions` itself; this command exists for the drag-and-drop
    // route, which needs the same list to filter what it accepts and to explain what it refused (the interface owns
    // the plural form the i18n catalogue supplies).
    //
    // Reported rather than duplicated, for the same reason `crates/opai/src/image/raw.rs` gives for its own list: a
    // second copy goes stale the moment the backend gains a format.
    traced_sync(command_span!("input_extensions", traceparent), opai::image::input_extensions)
}

/// Describes every path, admits what carries an identity, and reports the records ordered by path.
///
/// The one path both commands take, which is what makes their records equal for the same file.
///
/// # Errors
///
/// Fails only if the runtime is shutting down — not because of anything wrong with a file.
pub(crate) async fn describe_and_admit(paths: Vec<PathBuf>, opened: &Opened) -> Result<Vec<ImageRecord>, String> {
    // Each file is described concurrently on its own blocking thread, since hashing every byte dominates a large
    // selection — the reference does the same with a worker per CPU.
    let describing: Vec<_> = paths
        .into_iter()
        .map(|path| spawn_blocking(move || (ImageRecord::describe(&path), path)))
        .collect();

    let mut described = Vec::with_capacity(describing.len());
    for handle in describing {
        described.push(handle.await.map_err(|error| format!("the files could not be described: {error}"))?);
    }

    // Sorted by path rather than left in completion order, so the same selection lists the same way twice.
    described.sort_by(|(_, left), (_, right)| left.cmp(right));
    opened.admit(&described);

    Ok(described.into_iter().map(|(record, _)| record).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::images::serve::IDENTITY_LENGTH;
    use crate::images::test_support::{admit, write_image};

    use opai::ImageFormat;
    use tauri::Manager;
    use tempfile::tempdir;

    // ── The record ────────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_sound_file_is_described_in_full() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_image(&dir, "holiday.PNG", 64, 48, ImageFormat::Png);

        let record = ImageRecord::describe(&path);

        assert_eq!(record.path, path.to_string_lossy());
        assert_eq!((record.width, record.height), (Some(64), Some(48)));
        assert_eq!(record.extension, "png", "the extension is reported lowercased and without its dot");
        assert_eq!(record.size, Some(std::fs::metadata(&path).expect("readable").len()));
    }

    #[test]
    fn the_identity_is_the_one_the_loaded_image_carries() {
        // The equality the whole module depends on: the registry is keyed on what `describe` computed, and `serve`
        // hands back what `load` produced. If the two disagreed, every rendition would be a refusal.
        let dir = tempdir().expect("a temporary directory");
        let path = write_image(&dir, "holiday.png", 32, 24, ImageFormat::Png);

        let described = ImageRecord::describe(&path);
        let loaded = opai::image::load_blocking(&path).expect("decodable");

        assert_eq!(described.identity.as_deref(), Some(loaded.identity()));
        assert_eq!(described.identity.as_deref().map(str::len), Some(IDENTITY_LENGTH));
    }

    #[test]
    fn a_file_whose_header_cannot_be_read_survives_alongside_sound_ones() {
        // A selection is a batch the user assembled. Discarding all of it because one file has a truncated header
        // costs them the other files for no gain, and the file that failed is still one they named.
        let dir = tempdir().expect("a temporary directory");
        let sound = write_image(&dir, "a-sound.png", 16, 16, ImageFormat::Png);
        let truncated = dir.path().join("b-truncated.png");
        std::fs::write(&truncated, b"\x89PNG\r\n\x1a\n and then nothing").expect("writable");

        let records = block_on_describe(vec![sound, truncated]);

        assert_eq!(records.len(), 2, "the sound file was lost with the broken one");
        assert_eq!((records[0].width, records[0].height), (Some(16), Some(16)));
        assert_eq!((records[1].width, records[1].height), (None, None), "the header was not readable");
        // Its bytes were readable even though its header made no sense, so it has an identity and a size.
        assert!(records[1].identity.is_some(), "a readable file has an identity whatever its header says");
        assert!(records[1].size.is_some());
    }

    #[test]
    fn a_missing_file_is_reported_with_everything_unknown() {
        // The user named it, so it is reported. Nothing about it could be read, so nothing about it is claimed — and
        // with no identity it is never admitted, which is what stops `serve` being asked for a file that is not there.
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.jpg");

        let record = ImageRecord::describe(&path);

        assert_eq!(record.identity, None);
        assert_eq!((record.width, record.height), (None, None));
        assert_eq!(record.size, None);
        // The two facts that come from the name rather than from the file, and so survive it being absent.
        assert_eq!(record.extension, "jpg");
        assert_eq!(record.path, path.to_string_lossy());
    }

    #[test]
    fn a_file_with_no_extension_reports_an_empty_one_rather_than_failing() {
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("holiday");
        std::fs::copy(write_image(&dir, "source.png", 8, 8, ImageFormat::Png), &path).expect("copyable");

        let record = ImageRecord::describe(&path);

        assert_eq!(record.extension, "");
        // And the picture is still described: loading routes on content, and so does probing a file it can read.
        assert!(record.identity.is_some());
    }

    #[test]
    fn what_could_not_be_read_is_absent_from_the_wire_rather_than_null() {
        // The frontend's own convention, which this is the Rust half of: a value that is not there is an absent
        // property, so `record.width` is `undefined` rather than `null`.
        let dir = tempdir().expect("a temporary directory");
        let record = ImageRecord::describe(&dir.path().join("never-written.jpg"));

        let json = serde_json::to_string(&record).expect("the record should serialize");

        assert!(!json.contains("null"), "an unknown value crossed as null: {json}");
        assert!(!json.contains("width"), "an absent dimension named itself anyway: {json}");
        assert!(json.contains(r#""extension":"jpg""#), "what IS known must still cross: {json}");
    }

    // ── The batch ─────────────────────────────────────────────────────────────────────────────────────────────────

    /// [`describe_and_admit`] against a registry the caller does not care about, for the tests that are about the
    /// records rather than about what was admitted.
    fn block_on_describe(paths: Vec<PathBuf>) -> Vec<ImageRecord> {
        tauri::async_runtime::block_on(describe_and_admit(paths, &Opened::default())).expect("the runtime is alive")
    }

    #[test]
    fn a_selection_is_reported_ordered_by_path() {
        let dir = tempdir().expect("a temporary directory");
        let first = write_image(&dir, "a.png", 8, 8, ImageFormat::Png);
        let second = write_image(&dir, "b.png", 8, 8, ImageFormat::Png);
        let third = write_image(&dir, "c.png", 8, 8, ImageFormat::Png);

        let records = block_on_describe(vec![third.clone(), first.clone(), second.clone()]);
        let paths: Vec<_> = records.iter().map(|record| record.path.as_str()).collect();

        assert_eq!(paths, [first.to_string_lossy(), second.to_string_lossy(), third.to_string_lossy()]);
    }

    #[test]
    fn the_same_selection_described_twice_is_reported_the_same_way_twice() {
        // The property the order exists for. The files are described on their own threads, so they finish in
        // whatever order they finish in — a listing that inherited that would shuffle the user's files per call.
        let dir = tempdir().expect("a temporary directory");
        let paths: Vec<_> = (0..8)
            .map(|index| write_image(&dir, &format!("{index}.png"), 8 + index, 8, ImageFormat::Png))
            .collect();

        let first = block_on_describe(paths.clone());
        let second = block_on_describe(paths.into_iter().rev().collect());

        assert_eq!(first, second);
    }

    // ── The registry ──────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_file_with_no_identity_is_never_admitted() {
        // Nothing addresses it, so nothing can ask for it. It is still reported — the user named it.
        let dir = tempdir().expect("a temporary directory");
        let opened = Opened::default();

        let records =
            tauri::async_runtime::block_on(describe_and_admit(vec![dir.path().join("never-written.png")], &opened))
                .expect("the runtime is alive");

        assert_eq!(records.len(), 1, "the file the user named was dropped");
        assert!(lock(&opened.admitted).files.is_empty(), "a file with no identity was admitted anyway");
    }

    #[test]
    fn a_file_described_by_either_route_is_known_by_its_path() {
        // Both commands funnel into `describe_and_admit`, which is what makes "the application opened this file" one
        // question with one answer however the file arrived. Asserted on the funnel rather than on the picker, which
        // cannot be scripted — the same substitution `a_file_described_by_either_route_produces_the_same_record`
        // makes.
        let dir = tempdir().expect("a temporary directory");
        let readable = write_image(&dir, "holiday.png", 8, 8, ImageFormat::Png);

        // Never written, so it has no identity — and it is still a file the user opened, which is the whole of why
        // the reveal is addressed by path.
        let unreadable = dir.path().join("never-written.png");

        let opened = Opened::default();
        tauri::async_runtime::block_on(describe_and_admit(vec![readable.clone(), unreadable.clone()], &opened))
            .expect("the runtime is alive");

        assert!(opened.knows(&readable), "a described file is not known by its path");
        assert!(
            opened.knows(&unreadable),
            "a file with no identity was left unknown, so it could never be revealed"
        );
    }

    #[test]
    fn a_path_the_user_never_opened_is_not_known() {
        // The whole of what stops a reveal reaching the machine: the set is what was opened, and nothing else is in
        // it. A sibling of an admitted file is the closest miss there is.
        let dir = tempdir().expect("a temporary directory");
        let path = write_image(&dir, "holiday.png", 8, 8, ImageFormat::Png);
        let opened = Opened::default();
        admit(&opened, &path);

        assert!(!opened.knows(&dir.path().join("holiday-2.png")), "a file beside an admitted one was known");
        assert!(!opened.knows(Path::new("/etc/passwd")), "an arbitrary path on the machine was known");
    }

    #[test]
    fn a_reveal_naming_a_file_the_user_never_opened_is_refused() {
        // The assertion is as much about *where* it fails as that it fails: the refusal happens before
        // `crate::reveal` is reached, so nothing opens a file manager during this test. Were the check absent, this
        // would put Finder in front of whoever ran it.
        let app = tauri::test::mock_builder()
            .manage(Opened::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build");

        let refused = tauri::async_runtime::block_on(reveal_image(
            app.handle().clone(),
            "/etc/passwd".to_string(),
            app.state::<Opened>(),
            Traceparent::default(),
        ))
        .expect_err("a file the application never opened was revealed");

        assert_eq!(refused.to_string(), "`/etc/passwd` is not an image this application has opened");
    }

    #[test]
    fn a_failed_reveal_is_tagged_with_its_own_command() {
        // Tagged like `LogsError`'s is, so a caller reading `kind` is reading what it asked for. The subject is
        // supplied here rather than by `crate::reveal`, which states the failure and names nothing - see its own
        // documentation.
        let error =
            ImagesError::RevealImage { message: "the image is not one this application has opened".to_string() };

        assert_eq!(
            serde_json::to_string(&error).expect("the error should serialize"),
            r#"{"kind":"revealImage","message":"the image is not one this application has opened"}"#
        );
    }

    // ── The two routes in ─────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_file_described_by_either_route_produces_the_same_record() {
        // The requirement in the spec's own words: "a file dragged onto the window is the same photograph as a file
        // chosen from a dialog, and an interface that had to tell them apart would be carrying a difference that does
        // not exist." Both commands funnel into `describe_and_admit`, so this asserts the funnel rather than the
        // picker — which cannot be scripted and is checked by hand instead.
        let dir = tempdir().expect("a temporary directory");
        let path = write_image(&dir, "holiday.png", 24, 16, ImageFormat::Png);

        let app = tauri::test::mock_builder()
            .manage(Opened::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build");
        let state = app.state::<Opened>();

        let supplied = tauri::async_runtime::block_on(describe_images(
            vec![path.to_string_lossy().into_owned()],
            state.clone(),
            Traceparent::default(),
        ))
        .expect("the runtime is alive");

        // What the picker's half of `open_images` reduces to once the paths are in hand: the same call, with paths
        // that came from a dialog rather than from a window event.
        let picked = tauri::async_runtime::block_on(describe_and_admit(vec![path.clone()], &state))
            .expect("the runtime is alive");

        assert_eq!(supplied, picked, "the same file described by the two routes is two different photographs");
        assert_eq!(state.resolve(supplied[0].identity.as_ref().expect("readable")), Some(path));
    }

    #[test]
    fn the_formats_reported_to_the_window_are_the_decoders_own() {
        // Equality rather than a spot check of a few extensions: what makes the command worth having is that the
        // window judges a dropped file by the decoder's list instead of a copy, and a command that filtered, sorted
        // or added to it would be that copy wearing the library's name.
        assert_eq!(input_extensions(Traceparent::default()), opai::image::input_extensions());

        // The two properties the drop route depends on, stated here because a list that lost either would refuse
        // files the picker offers: camera RAW is an input format, and the extensions carry no dots to trim.
        assert!(
            input_extensions(Traceparent::default()).contains(&"nef"),
            "camera RAW is a format the picker is filtered to"
        );
        assert!(input_extensions(Traceparent::default()).iter().all(|extension| !extension.starts_with('.')));
    }

    // ── The errors that cross the boundary ────────────────────────────────────────────────────────────────────────

    #[test]
    fn each_command_s_failure_is_tagged_with_its_own_name() {
        // The convention `LogsError` set: a caller reading `kind` is reading what it asked for.
        let picker = ImagesError::OpenImages { message: "the file picker could not be opened".to_string() };
        let describe = ImagesError::DescribeImages { message: "the files could not be described".to_string() };

        assert_eq!(
            serde_json::to_string(&picker).expect("serializable"),
            r#"{"kind":"openImages","message":"the file picker could not be opened"}"#
        );
        assert_eq!(
            serde_json::to_string(&describe).expect("serializable"),
            r#"{"kind":"describeImages","message":"the files could not be described"}"#
        );
    }

    #[test]
    fn each_ending_is_recorded_by_whoever_saw_it() {
        let s = String::new;
        let cells = [
            (ImagesError::OpenImages { message: s() }, "stopped"),
            (ImagesError::DescribeImages { message: s() }, "stopped"),
            (ImagesError::RevealImage { message: s() }, "recorded by the wrapper"),
        ];
        for (error, cell) in cells {
            assert_eq!(CommandError::ended(&error).cell(), cell, "{error:?}");
        }
    }

    #[test]
    fn describing_files_keeps_each_image_read_beneath_the_command() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let paths: Vec<_> = ["a.png", "b.png"]
            .iter()
            .map(|name| {
                let path = dir.path().join(name);
                opai::image::save_blocking(&image::DynamicImage::new_rgb8(4, 3), &path, opai::ImageFormat::Png, None)
                    .expect("writable");
                path
            })
            .collect();
        let opened = Opened::default();

        let (recorded, answer) = crate::test_support::recorded(|| {
            tauri::async_runtime::block_on(traced(command_span!("describe_images", Traceparent::default()), async {
                describe_and_admit(paths, &opened)
                    .await
                    .map_err(|message| ImagesError::DescribeImages { message })
            }))
        });
        assert_eq!(answer.expect("both files are readable").len(), 2);

        let command = recorded.only("describe_images");
        let units: Vec<_> = recorded.spans.iter().filter(|span| span.target == "opai::unit").collect();
        assert!(!units.is_empty(), "describing a file opened no unit of `opai`'s");
        for unit in units {
            assert!(
                recorded.descends_from(unit.index, command.index),
                "`{}` rooted a trace of its own: {recorded:#?}",
                unit.name
            );
        }
    }
}

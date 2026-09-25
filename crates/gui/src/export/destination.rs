//! Where an export is written, and the name it is written under when the one asked for is taken.
//!
//! [`claim`] answers a [`Claim`]: a path this export alone may write to, held until the export either commits it or
//! gives it up. See design.md D4.

use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

/// The highest number tried after the destination itself: `name_1.ext` up to `name_999.ext`. Also how many hidden
/// names an overwriting claim tries beside the destination.
const LAST_NUMBER: u32 = 999;

/// A destination this export may write to.
///
/// **Gives the name back when dropped, unless [`commit`](Self::commit) succeeded.** Every way out of an export after
/// the claim — a failed write, a stop, a panic — therefore leaves nothing at the name, and leaves a file an overwrite
/// would have replaced exactly as it was. The same shape a [`Registration`](crate::stops::Registration) takes for the
/// table of stops.
#[derive(Debug)]
pub(crate) struct Claim {
    /// Where the export ends up: the destination asked for, or the first free numbered name beside it.
    destination: PathBuf,
    /// The empty file this claim created to hold its place, and the one the export is written into. `destination`
    /// itself when not overwriting; when overwriting, a hidden name beside it, renamed over it on commit.
    placeholder: PathBuf,
    /// Whether the export finished, so the placeholder is the result rather than something to remove.
    committed: bool,
}

impl Claim {
    /// The path to write to.
    pub(crate) fn path(&self) -> &Path {
        &self.placeholder
    }

    /// Keeps what was written, and answers where it now is.
    ///
    /// When overwriting, this is the moment the destination is replaced: the written file is renamed over it, taking
    /// the permissions of the file it replaces.
    ///
    /// # Errors
    ///
    /// The filesystem's own, if the rename is refused. The destination is then left as it was, and the written file
    /// is removed.
    pub(crate) fn commit(mut self) -> io::Result<PathBuf> {
        if self.placeholder != self.destination {
            // A file the user replaces should not change who may read it. Nothing there yet: the placeholder's own.
            if let Ok(replaced) = std::fs::metadata(&self.destination) {
                std::fs::set_permissions(&self.placeholder, replaced.permissions())?;
            }

            std::fs::rename(&self.placeholder, &self.destination)?;
        }

        self.committed = true;

        Ok(std::mem::take(&mut self.destination))
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if !self.committed {
            // Nothing to report to: the export is already answering why it did not finish. A placeholder someone
            // else removed first is not a problem either.
            if let Err(error) = std::fs::remove_file(&self.placeholder) {
                tracing::warn!(path = %self.placeholder.display(), %error, "an unfinished export's name could not be released");
            }
        }
    }
}

/// Claims a destination to write an export to.
///
/// - **Overwriting:** the destination itself, whatever is there, but **not touched until [`Claim::commit`]**. The
///   export is written into a hidden name beside it, so an encode that fails halfway, or a stop, leaves the file it
///   would have replaced — possibly the photograph itself — exactly as it was.
/// - **Not overwriting:** the destination if it is free, else `stem_1.ext`, `stem_2.ext` .. `stem_999.ext`,
///   whichever is free first. A destination that is the source photograph's own file is simply taken.
///
/// Either way the name held is created empty and exclusively, so the filesystem decides a race and two claims can
/// never hold one name. Not a temporary file from `rust_sak::fs`: those are created readable by their owner alone, and
/// an export renamed from one would keep that.
///
/// # Errors
///
/// - The filesystem's own error, at the first name tried, for anything other than a name being taken: a directory
///   that does not exist, or one this application may not write to. Probing a thousand names would not change it.
/// - [`io::ErrorKind::AlreadyExists`], naming the destination, when every name up to `_999` is taken. Nothing is
///   replaced: overwriting is only ever the user's choice.
pub(crate) fn claim(requested: &Path, overwrite: bool) -> io::Result<Claim> {
    if overwrite {
        let placeholder = create_first_free((1..=LAST_NUMBER).map(|n| hidden(requested, n)))?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("no name beside `{}` was free to write it through", requested.display()),
            )
        })?;

        return Ok(Claim { destination: requested.to_path_buf(), placeholder, committed: false });
    }

    let candidates = std::iter::once(requested.to_path_buf()).chain((1..=LAST_NUMBER).map(|n| numbered(requested, n)));

    match create_first_free(candidates)? {
        Some(path) => Ok(Claim { destination: path.clone(), placeholder: path, committed: false }),
        None => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("`{}` and every numbered name up to _{LAST_NUMBER} beside it already exist", requested.display()),
        )),
    }
}

/// Creates the first free name among `candidates`, empty and exclusively, answering which one, or `None` when every
/// one is taken.
fn create_first_free(candidates: impl IntoIterator<Item = PathBuf>) -> io::Result<Option<PathBuf>> {
    for candidate in candidates {
        match OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(_) => return Ok(Some(candidate)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }

    Ok(None)
}

/// `requested` with `_n` added before its extension: `photo.png` becomes `photo_1.png`, and `photo` becomes
/// `photo_1`.
fn numbered(requested: &Path, n: u32) -> PathBuf {
    let mut name = requested.file_stem().unwrap_or_default().to_os_string();
    name.push(format!("_{n}"));

    if let Some(extension) = requested.extension() {
        name.push(".");
        name.push(extension);
    }

    requested.with_file_name(name)
}

/// A hidden name beside `requested` for an overwrite to be written through: `photo.png` becomes
/// `.photo.png.opai-1`. In the same directory, so the rename over the destination never crosses a filesystem.
fn hidden(requested: &Path, n: u32) -> PathBuf {
    let mut name = std::ffi::OsString::from(".");
    name.push(requested.file_name().unwrap_or_default());
    name.push(format!(".opai-{n}"));

    requested.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::*;

    /// A temporary directory, and a path inside it that nothing exists at yet.
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let requested = dir.path().join("photo-opai.png");

        (dir, requested)
    }

    #[test]
    fn a_free_destination_is_claimed_as_it_was_asked_for() {
        let (_dir, requested) = fixture();

        let claim = claim(&requested, false).expect("a free name in a writable directory");

        assert_eq!(claim.path(), requested);
        assert!(requested.exists(), "the name was not held while the export works");
    }

    #[test]
    fn a_taken_destination_is_numbered_and_left_unchanged() {
        let (dir, requested) = fixture();
        std::fs::write(&requested, b"an earlier export").expect("writable");

        let claim = claim(&requested, false).expect("a numbered name is free");

        assert_eq!(claim.path(), dir.path().join("photo-opai_1.png"));
        assert_eq!(std::fs::read(&requested).expect("readable"), b"an earlier export", "the taken file was touched");
    }

    #[test]
    fn the_numbering_goes_on_past_every_taken_name() {
        let (dir, requested) = fixture();
        for name in ["photo-opai.png", "photo-opai_1.png", "photo-opai_2.png"] {
            std::fs::write(dir.path().join(name), b"taken").expect("writable");
        }

        assert_eq!(claim(&requested, false).expect("free").path(), dir.path().join("photo-opai_3.png"));
    }

    #[test]
    fn a_name_without_an_extension_is_numbered_at_its_end() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let requested = dir.path().join("photo");
        std::fs::write(&requested, b"taken").expect("writable");

        assert_eq!(claim(&requested, false).expect("free").path(), dir.path().join("photo_1"));
    }

    #[test]
    fn the_source_photographs_own_path_is_numbered_like_any_taken_name() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let source = dir.path().join("holiday.png");
        std::fs::write(&source, b"the photograph").expect("writable");

        let claim = claim(&source, false).expect("a numbered name is free");

        assert_eq!(claim.path(), dir.path().join("holiday_1.png"));
        assert_eq!(std::fs::read(&source).expect("readable"), b"the photograph", "the source was touched");
    }

    #[test]
    fn overwriting_leaves_a_free_destination_empty_until_committed() {
        let (dir, requested) = fixture();

        let claimed = claim(&requested, true).expect("overwriting claims anything");
        assert_ne!(claimed.path(), requested, "an overwrite was written straight into its destination");
        assert!(!requested.exists(), "an overwriting claim created the destination ahead of the write");

        std::fs::write(claimed.path(), b"the export").expect("writable");

        assert_eq!(claimed.commit().expect("renamable"), requested);
        assert_eq!(std::fs::read(&requested).expect("readable"), b"the export");
        assert_eq!(std::fs::read_dir(dir.path()).expect("listable").count(), 1, "the hidden name was left behind");
    }

    #[test]
    fn overwriting_replaces_a_taken_destination_only_when_committed() {
        let (dir, requested) = fixture();
        std::fs::write(&requested, b"an earlier export").expect("writable");

        let claimed = claim(&requested, true).expect("overwriting claims anything");
        std::fs::write(claimed.path(), b"the export").expect("writable");
        assert_eq!(std::fs::read(&requested).expect("readable"), b"an earlier export", "replaced before commit");

        claimed.commit().expect("renamable");

        assert_eq!(std::fs::read(&requested).expect("readable"), b"the export");
        assert_eq!(
            std::fs::read_dir(dir.path()).expect("listable").count(),
            1,
            "a numbered or hidden file was left"
        );
    }

    #[test]
    fn an_overwrite_given_up_leaves_the_destination_as_it_was_and_nothing_beside_it() {
        let (dir, requested) = fixture();
        std::fs::write(&requested, b"an earlier export").expect("writable");

        let claimed = claim(&requested, true).expect("overwriting claims anything");
        // Half an encode, then a failure.
        std::fs::write(claimed.path(), b"the ex").expect("writable");
        drop(claimed);

        assert_eq!(
            std::fs::read(&requested).expect("readable"),
            b"an earlier export",
            "a failed overwrite touched it"
        );
        assert_eq!(std::fs::read_dir(dir.path()).expect("listable").count(), 1, "a given-up overwrite left a file");
    }

    #[cfg(unix)]
    #[test]
    fn an_overwrite_keeps_the_permissions_of_the_file_it_replaces() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, requested) = fixture();
        std::fs::write(&requested, b"an earlier export").expect("writable");
        std::fs::set_permissions(&requested, std::fs::Permissions::from_mode(0o640)).expect("chmod");

        let claimed = claim(&requested, true).expect("overwriting claims anything");
        std::fs::write(claimed.path(), b"the export").expect("writable");
        claimed.commit().expect("renamable");

        let mode = std::fs::metadata(&requested).expect("written").permissions().mode() & 0o777;
        assert_eq!(mode, 0o640, "an overwrite changed who may read the file");
    }

    #[test]
    fn an_overwrite_into_a_directory_that_does_not_exist_is_refused() {
        let (dir, _) = fixture();
        let requested = dir.path().join("missing").join("photo-opai.png");

        let refused = claim(&requested, true).expect_err("nothing can be written into a missing directory");

        assert_eq!(refused.kind(), io::ErrorKind::NotFound, "the refusal was not the filesystem's own: {refused}");
    }

    #[test]
    fn two_claims_racing_for_one_name_never_both_win_it() {
        for _ in 0..50 {
            let (dir, requested) = fixture();
            let start = Arc::new(Barrier::new(2));

            let racers: Vec<_> = (0..2)
                .map(|_| {
                    let (requested, start) = (requested.clone(), Arc::clone(&start));

                    std::thread::spawn(move || {
                        start.wait();

                        claim(&requested, false).expect("one of two names is free").commit().expect("nothing to rename")
                    })
                })
                .collect();

            let mut won: Vec<_> = racers.into_iter().map(|racer| racer.join().expect("no panic")).collect();
            won.sort();

            assert_eq!(won, [dir.path().join("photo-opai.png"), dir.path().join("photo-opai_1.png")]);
        }
    }

    #[test]
    fn a_claim_given_up_leaves_nothing_and_a_committed_one_keeps_its_file() {
        let (_dir, requested) = fixture();

        drop(claim(&requested, false).expect("free"));
        assert!(!requested.exists(), "a claim given up left its placeholder behind");

        let claimed = claim(&requested, false).expect("free again");
        std::fs::write(claimed.path(), b"the export").expect("writable");

        assert_eq!(claimed.commit().expect("nothing to rename"), requested);
        assert_eq!(std::fs::read(&requested).expect("readable"), b"the export", "a committed export was removed");
    }

    #[test]
    fn a_claim_given_up_during_a_panic_still_leaves_nothing() {
        let (_dir, requested) = fixture();

        let panicked = std::panic::catch_unwind(|| {
            let _claim = claim(&requested, false).expect("free");

            panic!("an export that panics");
        });

        assert!(panicked.is_err());
        assert!(!requested.exists(), "a panicking export left its placeholder behind");
    }

    #[test]
    fn a_directory_that_does_not_exist_is_refused_at_the_first_name() {
        let (dir, _) = fixture();
        let requested = dir.path().join("missing").join("photo-opai.png");

        let refused = claim(&requested, false).expect_err("nothing can be created in a missing directory");

        assert_eq!(refused.kind(), io::ErrorKind::NotFound, "the refusal was not the filesystem's own: {refused}");
        assert!(!dir.path().join("missing").exists());
    }

    #[test]
    fn every_numbered_name_taken_is_refused_naming_the_destination() {
        let (dir, requested) = fixture();
        std::fs::write(&requested, b"taken").expect("writable");
        for n in 1..=LAST_NUMBER {
            std::fs::write(dir.path().join(format!("photo-opai_{n}.png")), b"taken").expect("writable");
        }

        let refused = claim(&requested, false).expect_err("every name is taken");

        assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
        assert!(
            refused.to_string().contains(&requested.display().to_string()),
            "the refusal did not name the destination: {refused}"
        );
        assert_eq!(std::fs::read(dir.path().join("photo-opai_999.png")).expect("readable"), b"taken");
    }
}

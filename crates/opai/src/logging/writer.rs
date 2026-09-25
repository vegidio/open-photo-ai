//! The rotating file the records are written to, and the one lock around it.
//!
//! # Why the live file keeps its name
//!
//! `opai.log` is what a user is asked to attach to a bug report, so rotation has to move the *old* records out from
//! under that name and leave the live ones in it. That one requirement decides the rotator.
//!
//! `tracing-appender` is the obvious candidate — it is the `tokio-rs/tracing` project's own — and it cannot do it: it
//! *"will automatically append the current date and hour (UTC format) to the file name"*, so there is no `opai.log`,
//! only `opai.log.2026-09-14`, and the sentence a support instruction is written in stops being writable. It has no
//! compression either, which is the other half of what the reference implementation configures.
//! `tracing-rolling-file` and `rolling-file` have the same dated-live-file shape.
//!
//! # A rotated file is named for the day it covers
//!
//! Rotation fires on the first write *after* the day boundary, so the moment of rotation is already the next day and
//! a rotation-time stamp would name every file one day later than the records inside it. `DateFrom::DateYesterday` is
//! `file-rotate`'s own answer to that — *"date yesterday, to represent the timestamps within the log file"* — and it
//! is what makes `opai.log.2026-09-14.gz` the file holding the 14th's records.
//!
//! It is exact for a session that runs across midnight, which is the case a reader is in when they go looking. It is
//! approximate for the other one: an application opened after several days off rotates a live file older than
//! yesterday, and names it yesterday anyway. What matters there is that the stale file is rotated *before* this
//! session writes into it, so no two days' records share a file; the name being a day nobody wrote on is the cost of
//! not carrying a second copy of the file's modification time into the suffix scheme.
//!
//! # No stale check at startup
//!
//! The reference implementation needs one, because *"timberjack's `RotateAt` only fires while the process is alive
//! across midnight"*. `file-rotate` reads the live file's modification time when it opens and rotates on the first
//! write if the boundary has passed, so *"the application starts on a later day"* is its ordinary path. The check is
//! simply not written here — which is also why the session divider goes through this writer rather than around it.
//!
//! # One lock, held for the write
//!
//! [`Sink`] is the [`MakeWriter`] the subscriber formats into, and the writer it makes is the lock guard, so the lock
//! is held for exactly as long as one record's bytes take to reach the kernel. That is what keeps a record from being
//! interleaved with another thread's halfway through a line.
//!
//! The mutex is **never unwrapped**. ORT's callback runs this code on threads this project did not start, under C++
//! frames that cannot accept an unwind, so a poisoned lock is taken over rather than panicked on: a previous panic
//! while holding it can have left at most a partial line in the file, which is not a reason to stop writing the
//! records that explain the panic.

use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use file_rotate::compression::Compression;
use file_rotate::suffix::{AppendTimestamp, DateFrom, FileLimit};
use file_rotate::{ContentLimit, FileRotate, TimeFrequency};
use tracing_subscriber::fmt::MakeWriter;

/// How many rotated files are kept, beyond the live one.
///
/// A week, the reference implementation's figure: long enough that a user reporting a problem from a few days ago
/// still has the session in hand, short enough that a daily-launched application does not grow without bound.
const RETAINED_FILES: usize = 7;

/// The date a rotated file carries, and the only part of its name that is not `opai.log`.
///
/// Day-resolution rather than the crate's default `%Y%m%dT%H%M%S`, because the rotation is daily and the name is
/// meant to be read: `opai.log.2026-09-14.gz` says which day to open. A second rotation on the same day — a manual
/// one, or a clock moved backwards — gets `.1` appended by the rotator rather than colliding.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// The rotating file itself.
///
/// A newtype rather than a bare [`FileRotate`] so that the four configuration decisions are made in one place and
/// nothing else in this crate names the crate behind them — which is what bounds the exposure to a dependency on an
/// unhurried release cadence: replacing it is this file, and nothing that emits a record.
pub(super) struct Rotator(FileRotate<AppendTimestamp>);

/// Named rather than derived, because `AppendTimestamp` has no `Debug` of its own and the suffix scheme is a
/// configuration constant rather than state worth printing.
impl fmt::Debug for Rotator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rotator").field("daily", &true).field("retained", &RETAINED_FILES).finish()
    }
}

impl Rotator {
    /// Opens `path` for append, rotating it first if its last write was on an earlier day.
    ///
    /// The rotation does not happen here: the modification time is read now and acted on by the first write, which is
    /// why [`init`](super::init) writes the session divider through this rather than to the file directly.
    ///
    /// Infallible by construction, and that is [`FileRotate`]'s shape rather than a decision here — it holds its file
    /// as an `Option` and discards writes when it is `None`. The caller opens the path itself first so that an
    /// unwritable file is reported as an error before this is reached.
    pub(super) fn open(path: &Path) -> Self {
        // Explicit rather than left to the default, which happens to be the same three flags. `append(true)` is the
        // load-bearing one: each write is positioned by the kernel rather than by a cached offset, which is what
        // keeps two processes' records interleaved by line instead of overwriting one another during the one window
        // where two of them can both be writing — a second process that has started logging and has not yet been
        // refused by the single-instance claim.
        let mut options = OpenOptions::new();
        options.read(true).create(true).append(true);

        Self(FileRotate::new(
            path,
            AppendTimestamp::with_format(DATE_FORMAT, FileLimit::MaxFiles(RETAINED_FILES), DateFrom::DateYesterday),
            ContentLimit::Time(TimeFrequency::Daily),
            // `OnRotate(0)` compresses every rotated file rather than leaving the most recent ones plain: the live
            // file is the one a user attaches, and the history is there to be kept rather than read in place.
            Compression::OnRotate(0),
            Some(options),
        ))
    }

    /// Rotates now, whatever the day is.
    ///
    /// Only the tests call this — the application rotates on a date boundary and never on demand — and they call it
    /// because seven days cannot be waited for.
    #[cfg(test)]
    pub(super) fn rotate_now(&mut self) -> io::Result<()> {
        self.0.rotate()
    }

    /// The rotated files, oldest first. Test-only, for the same reason.
    #[cfg(test)]
    pub(super) fn rotated(&mut self) -> Vec<std::path::PathBuf> {
        self.0.log_paths()
    }
}

impl Write for Rotator {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// The sink the subscriber writes through: one [`Rotator`] behind one lock.
///
/// Cloneable over an [`Arc`] because [`MakeWriter`] is handed to the subscriber by value while the tests keep a
/// handle to read back what was written.
#[derive(Debug, Clone)]
pub(super) struct Sink(Arc<Mutex<Rotator>>);

impl Sink {
    /// The sink writing into `rotator`.
    pub(super) fn new(rotator: Rotator) -> Self {
        Self(Arc::new(Mutex::new(rotator)))
    }

    /// Takes the lock, recovering it rather than panicking if a previous holder panicked while it was held.
    ///
    /// Through [`crate::task::lock`], which is the rule for every mutex in this crate rather than one written again
    /// here. See this module's header for why this site in particular cannot afford the alternative: this code runs
    /// on threads ORT started, under frames that cannot accept an unwind, so there is no path here that may panic.
    fn lock(&self) -> MutexGuard<'_, Rotator> {
        crate::task::lock(&self.0)
    }

    /// Writes `line` and a newline, discarding any failure.
    ///
    /// Used for the session divider, which is written before the subscriber exists and so has no other way in. The
    /// failure is discarded for the same reason the subscriber discards its writer's: a log that cannot be written is
    /// not a reason to stop the application, and there is nowhere left to report it to.
    pub(super) fn write_line(&self, line: &str) {
        let mut rotator = self.lock();
        let _ = writeln!(rotator, "{line}");
    }

    /// Everything written so far. Test-only.
    #[cfg(test)]
    pub(super) fn rotator(&self) -> MutexGuard<'_, Rotator> {
        self.lock()
    }
}

/// The writer one record is formatted into: the lock guard, and nothing else.
///
/// So the lock is taken when the subscriber starts a line and released when it finishes one, and a record cannot be
/// interleaved with another thread's part-way through. A newtype only because [`MutexGuard`] is not this crate's to
/// implement [`Write`] on.
#[derive(Debug)]
pub(super) struct Locked<'a>(MutexGuard<'a, Rotator>);

impl Write for Locked<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a> MakeWriter<'a> for Sink {
    type Writer = Locked<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        Locked(self.lock())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime};

    /// A [`Sink`] over a fresh `opai.log` in `dir`.
    fn sink_in(dir: &Path) -> (Sink, std::path::PathBuf) {
        let path = dir.join("opai.log");
        (Sink::new(Rotator::open(&path)), path)
    }

    /// Backdates `path`'s modification time by `days`, which is how a test reaches a date boundary without waiting.
    fn backdate(path: &Path, days: u64) {
        let file = OpenOptions::new().write(true).open(path).unwrap();
        let when = SystemTime::now() - Duration::from_secs(days * 24 * 60 * 60);
        file.set_modified(when).unwrap();
    }

    #[test]
    fn records_are_appended_rather_than_replacing_what_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opai.log");
        fs::write(&path, "from a previous run\n").unwrap();

        let sink = Sink::new(Rotator::open(&path));
        sink.write_line("from this one");
        drop(sink);

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "from a previous run\nfrom this one\n");
    }

    #[test]
    fn a_stale_live_file_is_rotated_before_this_session_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opai.log");

        // A previous day's session, left behind at that day's date.
        fs::write(&path, "yesterday's records\n").unwrap();
        backdate(&path, 1);

        // Opening reads the modification time; the *first write* is what acts on it. That is why the session divider
        // goes through this writer: it is what rotates the stale file, so this session's first line is the first line
        // of a new `opai.log`.
        let sink = Sink::new(Rotator::open(&path));
        sink.write_line("---");

        let live = fs::read_to_string(&path).unwrap();
        assert_eq!(live, "---\n", "this session's divider is not the first line of a new file");

        let rotated = sink.rotator().rotated();
        assert_eq!(rotated.len(), 1, "the stale file was not rotated: {rotated:?}");
    }

    #[test]
    fn a_live_file_written_today_is_not_rotated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opai.log");
        fs::write(&path, "earlier today\n").unwrap();

        let sink = Sink::new(Rotator::open(&path));
        sink.write_line("---");

        assert_eq!(fs::read_to_string(&path).unwrap(), "earlier today\n---\n");
        assert!(sink.rotator().rotated().is_empty(), "a file written today was rotated");
    }

    #[test]
    fn a_rotated_file_is_gzip_compressed_and_carries_the_day_it_covers() {
        let dir = tempfile::tempdir().unwrap();
        let (sink, path) = sink_in(dir.path());

        sink.write_line("the day's records");
        sink.rotator().rotate_now().unwrap();

        let rotated = sink.rotator().rotated();
        assert_eq!(rotated.len(), 1);

        let name = rotated[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.ends_with(".gz"), "a rotated file is not compressed: {name}");
        assert!(name.starts_with("opai.log."), "a rotated file is not named after the live one: {name}");

        // The day it covers, not the day it was rotated on. Rotation fires after the boundary, so "yesterday" from
        // the rotator's point of view is the day whose records are inside.
        let yesterday = chrono_yesterday();
        assert!(name.contains(&yesterday), "{name} does not carry the day it covers ({yesterday})");

        // And the bytes really are gzip: the two-byte magic number, so this is the file a user can open rather than a
        // plain file with a misleading extension.
        let bytes = fs::read(&rotated[0]).unwrap();
        assert_eq!(&bytes[..2], &[0x1f, 0x8b], "a rotated file is not gzip");

        // The live file keeps its name, which is the whole reason this rotator was chosen.
        assert!(path.exists());
        assert_eq!(path.file_name().unwrap(), "opai.log");
    }

    #[test]
    fn history_is_pruned_to_seven_files() {
        let dir = tempfile::tempdir().unwrap();
        let (sink, _) = sink_in(dir.path());

        // Ten rotations, which is three more than are kept.
        for day in 0..10 {
            sink.write_line(&format!("day {day}"));
            sink.rotator().rotate_now().unwrap();
        }

        let rotated = sink.rotator().rotated();
        assert_eq!(rotated.len(), RETAINED_FILES, "history is not bounded at seven: {rotated:?}");

        // And what is on disk agrees with what the rotator believes, so the pruning deleted files rather than just
        // forgetting them: the live file plus seven rotated ones.
        let on_disk = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(on_disk, RETAINED_FILES + 1, "the pruned files are still on disk");
    }

    #[test]
    fn a_poisoned_lock_is_taken_over_rather_than_panicked_on() {
        let dir = tempfile::tempdir().unwrap();
        let (sink, path) = sink_in(dir.path());

        // Poison the lock the way a panic in a formatter would, then go on writing. ORT's callback runs this code
        // under C++ frames that cannot accept an unwind, so there is no path here that may panic.
        let poisoner = sink.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.lock();
            panic!("a formatter panicked while holding the lock");
        })
        .join();

        sink.write_line("after the poisoning");

        assert_eq!(fs::read_to_string(&path).unwrap(), "after the poisoning\n");
    }

    /// Yesterday in `DATE_FORMAT`, as the suffix scheme spells it.
    fn chrono_yesterday() -> String {
        // Through the same `%Y-%m-%d` the rotator is configured with, built from `SystemTime` so the test needs no
        // date library of its own. Seconds since the epoch, less a day, to a civil date.
        let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        let days = secs / 86_400 - 1;
        let (year, month, day) = civil_from_days(days as i64);
        format!("{year:04}-{month:02}-{day:02}")
    }

    /// Howard Hinnant's `civil_from_days`, which is the shortest correct way to get a UTC date without a dependency.
    ///
    /// Only the test needs it: the rotator is told a format and does its own arithmetic through `chrono`.
    fn civil_from_days(z: i64) -> (i64, u32, u32) {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (if m <= 2 { y + 1 } else { y }, m, d)
    }
}

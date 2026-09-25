//! The single-instance claim: at most one Open Photo AI process per user.
//!
//! One advisory file lock on `instance.lock` in the configuration directory, taken by
//! [`Opai::initialize`](crate::Opai::initialize) and held for the life of the handle it returns. Two initializations
//! in one process under one directory share one claim; two under different names are two directories and both succeed.
//!
//! Per user, because the configuration directory already is on all three platforms: two people signed into one machine
//! contend for nothing. Advisory, and deliberately not a security boundary: it coordinates cooperating participants,
//! and every participant is this application.
//!
//! The one setup where the lock itself does not hold is a configuration directory on a network filesystem — `flock`
//! over NFS and SMB is variously emulated, silently a no-op or subtly wrong — which needs a roaming profile or a
//! network-mounted home to reach. The failure there is that two processes both start, as though there were no claim.

// The claim is what makes "one writer" true for the three things that assume it — the run cache's store, the model and
// dependency manifests, and the log file every binary of this project writes into. Its scope is the scope of the state
// it protects.
//
// A lock rather than a file naming the holder: **the kernel owns the lock, not the file's contents.** It is released
// when the last descriptor on the file closes, which the operating system does for every descriptor of a dying process
// whatever killed it — a panic, a `SIGKILL`, a native crash inside an inference session, a power cut. There is no
// cleanup path to get wrong and no state to go stale.
//
// A process-id file is the obvious alternative and is wrong here: it outlives the process that wrote it, so every
// launch has to guess whether the recorded id is a live Open Photo AI or one the kernel has since recycled onto
// something else. Guessing wrong one way leaves the application permanently unable to start, and the other way
// silently permits the second process the whole mechanism exists to refuse. The record this module *does* write is
// therefore a courtesy and nothing more — see `read_holder` — kept in a file of its own for a reason that is entirely
// about Windows — see `HOLDER_FILE`.
//
// `flock(LOCK_EX | LOCK_NB)` on Unix and `LockFileEx(LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY)` on Windows,
// behind `File::try_lock`, which the standard library has carried since Rust 1.89. **No dependency**: the obvious
// crates for this (`fs4`, `fd-lock`, and the long-unmaintained `fs2`) exist largely because std did not have it, and
// each wraps the same two system calls.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use crate::app::Holder;
use crate::error::InitError;
use crate::task;

// Outside all of them, because the claim guards them and a lock living inside one of the directories it protects
// invites being deleted along with it. Not in the system temp directory either, which is neither per-user on every
// platform nor guaranteed to survive.
/// The lock file, a sibling of `cache/`, `models/`, `runtime/` and `libs/` rather than a child of any of them.
///
/// Nothing is ever read out of it or written into it — see [`HOLDER_FILE`].
const LOCK_FILE: &str = "instance.lock";

// Two files rather than one, and the reason is Windows. The standard library takes the lock with
// `LockFileEx(LOCKFILE_EXCLUSIVE_LOCK, .., u32::MAX, u32::MAX)`, which locks the whole file *and* makes that region
// unreadable and unwritable through every other handle — including a second handle in the locking process's own. A
// record kept inside the lock file would therefore be impossible to write on Windows and impossible for the refused
// process to read, so every refusal there would name no holder while the tests for it passed on Unix, where `flock`
// guards the open file description and blocks no I/O at all.
//
// Keeping the record in a file nothing locks costs nothing anywhere: the contents decide nothing in either design —
// see `read_holder` — so the only thing the separation protects is the name in the message.
/// The holder record, beside the lock file rather than inside it.
const HOLDER_FILE: &str = "instance.holder";

/// Where the holder record for `config_dir` is.
fn holder_path(config_dir: &Path) -> PathBuf {
    config_dir.join(HOLDER_FILE)
}

/// An exclusive advisory lock on one configuration directory, released when this is dropped.
#[derive(Debug)]
struct OwnedLock {
    // There is no `Drop` impl and none is wanted: the lock belongs to the open file description, so closing the handle
    // is what releases it, and dropping this drops the handle. Writing a `Drop` that unlocked first would add a step
    // that can be skipped — by a panic, by `std::mem::forget` — to something the operating system already does
    // unconditionally, including for a process that never ran any Rust destructor at all.
    /// The locked file. Holding it is holding the claim.
    ///
    /// Never read and never written: the contents decide nothing, so it has none.
    #[expect(dead_code, reason = "the handle is the claim; its value is that it stays open")]
    file: File,
    /// The configuration directory this is the claim on. Both file names hang off it.
    dir: PathBuf,
}

impl OwnedLock {
    /// Where the lock file is.
    #[cfg(test)]
    fn path(&self) -> PathBuf {
        self.dir.join(LOCK_FILE)
    }

    /// Where this claim's holder record is.
    fn holder(&self) -> PathBuf {
        holder_path(&self.dir)
    }
}

// Why a registry: `Opai::initialize` may be called more than once in a process — it is what a front end restarting its
// application layer after an error does — and the second call must not be refused by the first one's claim. A second
// lock request on the same path from the same process does **not** quietly succeed: on Unix a lock belongs to the open
// file description, so opening the file again takes a second, independent lock that contends with this process's own
// first one. So the first initialization under a directory opens and locks the file; a later one under the same
// directory takes a share of what is already held, and the file is closed — releasing the lock — when the last share is
// dropped.
//
// This is the answer `rust-sak`'s `memo` already gives one layer down for the cache's own store, which *"hands back the
// store this process already has open rather than trying to open it again"*. A third mechanism with different
// semantics for the same situation is the thing to avoid.
//
// Keyed on the resolved directory rather than on the application name, because the directory is what is actually
// contended: it is what the cache, the models and the log all live in, and two names that somehow resolved to one
// directory would be one claim rather than two.
//
// `Weak`, so that the registry records what is held without *keeping* it held — the lifetime of a claim belongs to the
// `Opai` handle that took it, and an `Arc` here would make it the process's instead.
//
// A `BTreeMap` rather than a `HashMap` for one reason only: it is `const`-constructible, so the whole registry is a
// `static` with no lazy initialization around it. Nothing here is hot — it is touched twice per initialization — and it
// holds one entry on every machine that is not running a test.
/// Every configuration directory this process holds a claim on.
static CLAIMS: Mutex<BTreeMap<PathBuf, Weak<OwnedLock>>> = Mutex::new(BTreeMap::new());

/// A share of this process's claim on one configuration directory, released when it is dropped.
///
/// What [`Opai`](crate::Opai) holds: a field on its shared state, so the claim's lifetime is the handle's and the
/// last clone of a handle to be dropped is what gives the claim up. There is deliberately **no** way to release it
/// early.
#[derive(Debug)]
pub(crate) struct Claim {
    // No `unlock` method: one would be a way to keep an `Opai` whose invariant no longer holds. Dropping the
    // application is how you stop being the application.
    //
    // The directory this is a share of — the registry key `Drop` removes — is read off the lock rather than kept
    // beside it: two copies of one key are two things that have to agree, and a `Claim` whose own copy had drifted
    // would evict a different directory's entry.
    /// The shared lock. `Option` only so that [`Drop`] can take it and release it while the registry is held.
    lock: Option<Arc<OwnedLock>>,
}

impl Claim {
    /// Where the lock file is.
    #[cfg(test)]
    fn path(&self) -> PathBuf {
        self.lock.as_ref().expect("the lock is only taken by Drop").path()
    }
}

impl Drop for Claim {
    /// Gives up this share, and the lock itself where this was the last one.
    fn drop(&mut self) {
        // The registry is held across the release, which is not incidental. The lock belongs to the open file
        // description, so it is live until the file is actually closed — and a thread that observed the last share go
        // away and then opened the file *before* that close would be refused by this process's own dying claim.
        // Holding the registry makes the two one step.
        let mut claims = task::lock(&CLAIMS);
        let Some(lock) = self.lock.take() else { return };

        // Under the registry, so the count cannot move: the only thing that takes a second share is `claim` below,
        // which holds this same guard.
        if Arc::strong_count(&lock) == 1 {
            claims.remove(&lock.dir);
        }

        drop(lock);
    }
}

/// Claims `config_dir` for this process, or reports who already holds it.
///
/// Takes a share of what this process already holds where it holds one, and otherwise takes the lock for real. So a
/// second initialization under one directory succeeds and is not refused by the first's claim, while a second
/// *process* under that directory still is.
///
/// `app` is recorded only by the call that actually takes the lock, so a later initialization in this process under a
/// different identity leaves the first one's name in place.
///
/// `app` has been validated by the time it reaches here — see [`app::validate`](crate::app::validate) — so it is one
/// word with no whitespace in it, which is what makes the record readable back.
///
/// # Errors
///
/// As [`acquire`]: [`InitError::AlreadyRunning`] where another *process* holds the claim, and [`InitError::Io`] where
/// the lock file could not be opened or the lock request failed for a reason that is not contention.
pub(crate) fn claim(config_dir: &Path, app: &str) -> Result<Claim, InitError> {
    let mut claims = task::lock(&CLAIMS);

    // An entry whose `Weak` no longer upgrades is one whose last share has been dropped, which is the same state as
    // no entry at all — the lock is gone with it.
    //
    // A share records no holder: the holder is the process, and the first initialization is the one that named it.
    if let Some(held) = claims.get(config_dir).and_then(Weak::upgrade) {
        return Ok(Claim { lock: Some(held) });
    }

    let lock = Arc::new(acquire(config_dir, app)?);
    claims.insert(config_dir.to_path_buf(), Arc::downgrade(&lock));

    // Only where the lock was actually taken. A second initialization in this process returns above, having taken a
    // share of what was already held, and it is not a second claim to record.
    //
    // `debug` rather than `info`: a claim that succeeded is the ordinary case and says nothing a reader of a healthy
    // session needs. The record that matters is the refusal, and that one is a returned error the front end reports —
    // it is logged where it is *handled*, not here, so that a caller which chooses to carry on unrefused does not
    // leave an unexplained warning in the file.
    tracing::debug!(app, dir = %config_dir.display(), "single-instance claim taken");

    Ok(Claim { lock: Some(lock) })
}

/// Opens and locks `config_dir`'s lock file, taking the claim for real.
///
/// The inner half of [`claim`], which is what callers use: this one contends with **this process's own** claim as
/// readily as with another's, so calling it twice under one directory refuses the second — see [`CLAIMS`].
///
/// `config_dir` must exist: the lock file is created inside it, and by the time this is called the application name
/// has been validated and the directory resolved. Nothing beneath it is created here but the lock file and, once the
/// lock is held, the holder record beside it.
///
/// # Errors
///
/// Returns [`InitError::AlreadyRunning`] where another process of this user holds the claim, naming the holder where
/// its record could be read. Returns [`InitError::Io`] where the lock file could not be opened, or where the lock
/// request failed for a reason other than the lock being held — a read-only directory, a filesystem that does not
/// implement locking. Neither of those is a refusal, because neither means another process is running.
fn acquire(config_dir: &Path, app: &str) -> Result<OwnedLock, InitError> {
    let path = config_dir.join(LOCK_FILE);

    // Created if absent and **never truncated on open**, though nothing is ever written into it: an existing lock file
    // is the ordinary case, and opening it for truncation would be a write to a file another process may hold — which
    // on Windows fails against that process's lock rather than quietly doing nothing.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(InitError::io(&path))?;

    match file.try_lock() {
        Ok(()) => {}
        // The one answer that means another process is running. The holder is read *after* the refusal is decided,
        // never as part of deciding it.
        Err(TryLockError::WouldBlock) => {
            return Err(InitError::AlreadyRunning { holder: read_holder(&holder_path(config_dir)) });
        }
        Err(TryLockError::Error(source)) => return Err(InitError::Io { path, source }),
    }

    let lock = OwnedLock { file, dir: config_dir.to_path_buf() };
    lock.record(app);
    Ok(lock)
}

impl OwnedLock {
    /// Writes this process's [`Holder`] beside the lock, best effort, and only ever with the lock held.
    ///
    /// Failures are dropped, and a write that failed takes the previous holder's record with it: a lock held with no
    /// record beside it is a working claim whose refusals name no holder, never one that names a *stale* holder.
    fn record(&self, app: &str) {
        // The lock being held is what makes a plain truncate-and-write safe: the only processes that write this file
        // are the ones that hold the claim, and there is at most one of those. A reader of it is by definition a
        // process that has just been refused and is looking for something to put in a message.
        //
        // Failures are dropped on the floor deliberately. Nothing here is allowed to turn a claim that the kernel
        // granted into a failed initialization, and a missing record is exactly the degradation `read_holder` is
        // written for.
        let holder = self.holder();

        if std::fs::write(&holder, format!("{} {}\n", app, std::process::id())).is_err() {
            let _ = std::fs::remove_file(&holder);
        }
    }
}

/// Reads the [`Holder`] recorded at `path`, or `None` where there is nothing legible there.
///
/// **The contents decide nothing.** The kernel's answer to the lock request is the only thing that says whether a
/// process may proceed; this is read afterwards, purely so that a refusal can say *"already running (gui, pid 4821)"*
/// rather than that something unnamed is. So every way of failing — the file absent, empty, truncated, half-written,
/// written by a version that used another format, carrying a field this build does not expect — returns `None`,
/// which is a refusal that names no holder. None of them is an error, and none of them is ever a grant.
fn read_holder(path: &Path) -> Option<Holder> {
    // Keeping the contents from deciding is the whole point of choosing a lock over a process-id file: the moment the
    // contents can grant or deny, every failure mode of a process-id file is back.
    let text = std::fs::read_to_string(path).ok()?;

    // `<app> <pid>` on one line. Both halves must be there and the process id must parse; a line with a third field
    // leaves that half unparseable and is refused along with everything else unrecognised.
    //
    // The identity half is taken as written rather than checked against a list: any application may declare its own.
    // What stops an ambiguous or empty one from ever being recorded is the validation at the point it is declared,
    // which is why nothing is re-checked here.
    let (app, pid) = text.trim().split_once(' ')?;
    Some(Holder { app: app.to_string(), pid: pid.parse().ok()? })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::{CLI, GUI, PERF};
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    /// What the re-executed child below is asked to do, read from the environment because a test binary takes no
    /// arguments of its own.
    const MODE: &str = "OPAI_TEST_INSTANCE_MODE";
    /// The configuration directory the child acts on.
    const DIR: &str = "OPAI_TEST_INSTANCE_DIR";

    /// The child re-executes this test binary, so the lock is taken by a genuinely separate process rather than by a
    /// second handle in this one — which is the only way to exercise the thing being built, since two locks in one
    /// process are what the registry above deliberately collapses into one.
    ///
    /// Ignored so it never runs as part of an ordinary sweep; the helpers below name it explicitly and pass
    /// `--ignored`.
    #[test]
    #[ignore = "re-executed as a child process by the tests in this crate; not a test of its own"]
    fn child_process() {
        let dir = std::env::var(DIR).expect("the parent sets the directory");
        let mode = std::env::var(MODE).expect("the parent sets the mode");

        match acquire(Path::new(&dir), CLI) {
            Ok(lock) => {
                println!("{ACQUIRED}");
                if mode == "hold" {
                    // Held until the parent kills this process or lets it run out. Nothing here releases the lock;
                    // that is what makes this usable for both the ordinary case and the terminated one.
                    std::thread::sleep(std::time::Duration::from_secs(120));
                    drop(lock);
                }
            }
            Err(error) => println!("{REFUSED}{error}"),
        }
    }

    /// The two things the child says about its attempt, matched on by the parent.
    pub(crate) const ACQUIRED: &str = "opai-test: acquired";
    pub(crate) const REFUSED: &str = "opai-test: refused: ";

    /// What both of those start with, which is how either is found inside a line of `libtest`'s own output.
    const MARKER: &str = "opai-test: ";

    /// The lock file's name, for tests outside this module that walk a configuration directory and have to treat that
    /// one file differently from everything else in it — see `snapshot` in the crate root's tests.
    pub(crate) const LOCK_FILE_NAME: &str = LOCK_FILE;

    /// The child's answer within `line`, where that line carries one.
    ///
    /// `libtest` prints `test <name> ... ` with **no newline** and then whatever the test itself writes, so under
    /// `--nocapture` the answer arrives in the middle of the harness's own line rather than at the start of one.
    fn answer(line: &str) -> Option<&str> {
        line.find(MARKER).map(|at| &line[at..])
    }

    /// Spawns the child against `dir` in `mode`, with its output piped.
    fn spawn(dir: &Path, mode: &str) -> Child {
        Command::new(std::env::current_exe().expect("a test binary knows its own path"))
            .args(["instance::tests::child_process", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
            .env(DIR, dir)
            .env(MODE, mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the test binary is executable")
    }

    /// Runs the child to completion against `dir` and returns the line it printed about its attempt.
    ///
    /// Everything the test harness prints around it is discarded, so this stays a statement about the lock rather
    /// than about `libtest`'s output format.
    pub(crate) fn child_attempt(dir: &Path) -> String {
        let output = spawn(dir, "probe").wait_with_output().expect("the child ran");
        let stdout = String::from_utf8_lossy(&output.stdout);

        stdout
            .lines()
            .find_map(answer)
            .unwrap_or_else(|| panic!("the child said nothing about its attempt; it printed:\n{stdout}"))
            .to_string()
    }

    /// Spawns a child that takes the claim on `dir` and holds it, returning once it actually has it.
    ///
    /// Waiting for the child to *say* it has the lock rather than sleeping for a while is what keeps the tests that
    /// use it from being a race: a spawn that has not reached the lock yet is indistinguishable from one that failed
    /// to take it.
    pub(crate) fn child_holding(dir: &Path) -> Child {
        let mut child = spawn(dir, "hold");
        let stdout = child.stdout.take().expect("stdout was piped");

        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Some(answer) = answer(&line) else { continue };

            assert!(
                !answer.starts_with(REFUSED),
                "the child could not take the claim on an unheld directory: {answer}"
            );
            return child;
        }

        let _ = child.kill();
        panic!("the child exited without taking the claim");
    }

    /// The holder recorded beside the claim on `config_dir`, as a refused process would read it.
    ///
    /// For tests outside this module that need to see what an identity actually became on disk.
    pub(crate) fn holder_of(config_dir: &Path) -> Option<Holder> {
        read_holder(&holder_path(config_dir))
    }

    /// The claim [`Opai::initialize`](crate::Opai::initialize) would have taken by the time it reaches the install,
    /// against a directory of a test's own.
    ///
    /// `Opai::install` takes a claim by value, so every test that drives an install has to have taken one — which is
    /// the point of that signature rather than a cost of it: it is what makes "nothing is installed before the claim"
    /// impossible to get wrong. The directory is created here because the real `config::app_dir` creates it, and the
    /// lock file has to live somewhere.
    pub(crate) fn claimed(app_dir: &Path) -> Claim {
        std::fs::create_dir_all(app_dir).expect("the application directory is creatable");
        claim(app_dir, CLI).expect("a directory of this test's own is unheld")
    }

    /// A configuration directory of its own, standing in for the one initialization resolves.
    fn config_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
    }

    #[test]
    fn the_claim_is_taken_refused_while_held_and_available_again_once_released() {
        let dir = config_dir();

        let lock = acquire(dir.path(), GUI).expect("an unheld directory");
        assert!(lock.path().is_file(), "the lock file was not created");

        // A genuinely separate process, because that is the contention this exists for.
        assert!(child_attempt(dir.path()).starts_with(REFUSED), "a second process was allowed in");

        drop(lock);

        // And the release is real rather than merely the handle going out of scope: the same separate process now
        // gets in.
        assert!(child_attempt(dir.path()).starts_with(ACQUIRED), "the claim was not released");
    }

    #[test]
    fn a_refusal_is_an_already_running_error_rather_than_any_other_failure() {
        let dir = config_dir();
        let mut held = child_holding(dir.path());

        let error = acquire(dir.path(), CLI).expect_err("the child holds the claim");
        assert!(matches!(error, InitError::AlreadyRunning { .. }), "got {error:?}");

        let _ = held.kill();
        let _ = held.wait();
    }

    #[test]
    fn the_holder_record_is_written_beside_the_lock_and_reads_back_while_it_is_held() {
        let dir = config_dir();
        let lock = acquire(dir.path(), PERF).expect("an unheld directory");

        // Read back through a handle that holds no lock, which is the only way a refused process can read it — and on
        // Windows the reason the record is not kept inside the locked file, where this read would fail.
        assert_eq!(read_holder(&lock.holder()), Some(Holder { app: PERF.to_string(), pid: std::process::id() }));

        // Two files, and the lock is the empty one: nothing is ever written into what the kernel is guarding.
        assert!(lock.path().is_file(), "the lock file was not created");
        assert_ne!(lock.path(), lock.holder());
        // By its length rather than by reading it: on Windows the lock covers the whole file and makes those bytes
        // unreadable through every other handle — including this test's own, which holds the lock. That is the same
        // hazard the holder record was moved out of this file for, and reading it here would be walking into it.
        assert_eq!(
            lock.path().metadata().expect("the lock file is there").len(),
            0,
            "the locked file was written to"
        );
    }

    #[test]
    fn a_refusal_names_the_process_that_actually_holds_the_claim() {
        let dir = config_dir();
        let mut held = child_holding(dir.path());

        let error = acquire(dir.path(), GUI).expect_err("the child holds the claim");

        // The child took it as the terminal application, so this is the holder's own identity read off disk rather
        // than anything the refused process knew about itself.
        let InitError::AlreadyRunning { holder: Some(holder) } = error else {
            panic!("expected a refusal naming a holder, got {error:?}");
        };
        assert_eq!(holder, Holder { app: CLI.to_string(), pid: held.id() });

        let _ = held.kill();
        let _ = held.wait();
    }

    #[test]
    fn an_accepted_identity_is_the_exact_text_a_refusal_names() {
        // The round trip the validation exists to protect: an identity is declared, written into the record, read
        // back out of it by a refused process, and rendered — and what comes out the far end is the text that went
        // in, character for character. An embedder's own identity rather than one of this project's three, since the
        // three are the case that was already covered and the arbitrary one is what changed.
        let dir = config_dir();
        let identity = "com.example.Photos";

        let _held = acquire(dir.path(), identity).expect("an unheld directory");

        // `acquire` contends with this process's own claim as readily as with another's — see the module
        // documentation — so this is a real refusal built from the record the line above wrote, without a child.
        let error = acquire(dir.path(), GUI).expect_err("the claim is held");
        let InitError::AlreadyRunning { holder: Some(holder) } = error else {
            panic!("expected a refusal naming a holder, got {error:?}");
        };

        assert_eq!(holder.app, identity);
        assert_eq!(holder.pid, std::process::id());
        assert_eq!(holder.to_string(), "com.example.Photos, pid ".to_string() + &std::process::id().to_string());
    }

    #[test]
    fn a_record_that_cannot_be_read_names_no_holder() {
        let dir = config_dir();
        let path = holder_path(dir.path());

        // Absent, empty, half-written, missing either half, and from a build that wrote another format. Every one of
        // them is a holder that cannot be named.
        //
        // The identity itself is no longer one of the things that can fail: any application may declare its own, so
        // there is no list for `GUI` or `photos` to be absent from. What replaced that check is the validation at the
        // point an identity is declared — which is why this list is the same one it was, less that case.
        assert_eq!(read_holder(&path), None, "an absent record");

        for record in ["", "\n", "   ", "gui", "gui ", "gui\n", "4821", "gui 4821 extra", "gui four", "gui -1"] {
            std::fs::write(&path, record).expect("the record is writable");
            assert_eq!(read_holder(&path), None, "{record:?} was read as a holder");
        }

        // And the format that is actually written still reads, so the cases above are failing for the right reason
        // rather than because nothing parses at all — for one of this project's binaries and for an embedder's own
        // identity alike.
        std::fs::write(&path, "gui 4821\n").expect("the record is writable");
        assert_eq!(read_holder(&path), Some(Holder { app: GUI.to_string(), pid: 4821 }));

        std::fs::write(&path, "com.example.Photos 91\n").expect("the record is writable");
        assert_eq!(read_holder(&path), Some(Holder { app: "com.example.Photos".to_string(), pid: 91 }));
    }

    #[test]
    fn an_unreadable_record_beside_a_held_lock_refuses_rather_than_grants() {
        // D6, end to end and in the direction that matters: the contents are what a second process reads, so making
        // them illegible must cost the *name* in the message and nothing else. A grant here would be every failure
        // mode of a process-id file, back.
        let dir = config_dir();
        let mut held = child_holding(dir.path());

        // Nothing locks the record, which is exactly how it gets to be corrupted under a live holder — and is the
        // point: what the kernel guards and what a message is built from are two different files.
        std::fs::write(holder_path(dir.path()), b"\0\0 not a record at all").expect("the record is writable");

        let error = acquire(dir.path(), GUI).expect_err("the child still holds the claim");
        assert!(matches!(error, InitError::AlreadyRunning { holder: None }), "got {error:?}");

        let _ = held.kill();
        let _ = held.wait();
    }

    #[test]
    fn a_second_claim_in_one_process_shares_the_first_rather_than_contending_with_it() {
        // The constraint the registry exists for: a front end restarting its application layer is one process
        // initializing twice, which is a supported state and not a second instance.
        let dir = config_dir();

        let first = claim(dir.path(), GUI).expect("an unheld directory");
        let second = claim(dir.path(), GUI).expect("a second initialization in this process");

        // One lock, shared — not two locks that happened to both succeed.
        assert_eq!(first.path(), second.path());

        // And it is still one claim as far as any other process is concerned.
        assert!(child_attempt(dir.path()).starts_with(REFUSED));
    }

    #[test]
    fn the_lock_is_released_only_when_the_last_share_is_dropped() {
        let dir = config_dir();

        let first = claim(dir.path(), GUI).expect("an unheld directory");
        let second = claim(dir.path(), GUI).expect("a second initialization in this process");

        drop(first);
        assert!(child_attempt(dir.path()).starts_with(REFUSED), "dropping one share gave up the claim");

        drop(second);
        assert!(child_attempt(dir.path()).starts_with(ACQUIRED), "the last share did not release the claim");
    }

    #[test]
    fn a_directory_claimed_again_after_its_last_share_went_away_takes_a_fresh_lock() {
        // The registry must not remember a claim that is over: a stale entry would either hand out a share of a
        // released lock or refuse a directory nothing holds.
        let dir = config_dir();

        drop(claim(dir.path(), GUI).expect("an unheld directory"));
        let again = claim(dir.path(), CLI).expect("the claim was given up");

        assert!(child_attempt(dir.path()).starts_with(REFUSED), "the second claim is not a real lock");
        drop(again);
    }

    #[test]
    fn two_application_names_in_one_process_both_claim() {
        // Two names are two configuration directories, so they are two applications that share no cache, no models
        // and no log — and refusing either because of the other would be refusing it for nothing.
        let (one, two) = (config_dir(), config_dir());

        let first = claim(one.path(), GUI).expect("the first name's directory");
        let second = claim(two.path(), CLI).expect("the second name's directory");

        assert_ne!(first.path(), second.path());

        // Each against its own directory, and each real: neither borrowed the other's.
        assert!(child_attempt(one.path()).starts_with(REFUSED));
        assert!(child_attempt(two.path()).starts_with(REFUSED));
    }

    #[test]
    fn a_lock_file_left_behind_by_a_dead_process_does_not_lock_the_application_out() {
        // The property the mechanism was *chosen* for, so it is checked rather than assumed. A populated lock file
        // with no live holder is what a crash, a kill or a power cut leaves on disk — and under a process-id file it
        // is the state that leaves the application permanently unable to start.
        let dir = config_dir();
        std::fs::write(dir.path().join(LOCK_FILE), "").expect("the lock file is writable");
        std::fs::write(holder_path(dir.path()), "gui 4821\n").expect("the record is writable");

        let lock = acquire(dir.path(), CLI).expect("a lock file with no live holder must not refuse");

        // And the stale record is replaced rather than merged with or believed: the holder is now this process.
        assert_eq!(read_holder(&lock.holder()), Some(Holder { app: CLI.to_string(), pid: std::process::id() }));
    }

    #[test]
    fn the_claim_is_released_by_a_holder_terminated_without_cleanup() {
        let dir = config_dir();
        let mut held = child_holding(dir.path());

        // Refused while that process lives.
        let error = acquire(dir.path(), GUI).expect_err("the child holds the claim");
        assert!(matches!(error, InitError::AlreadyRunning { .. }), "got {error:?}");

        // Then killed outright — `SIGKILL` on Unix, `TerminateProcess` on Windows — neither of which runs one line of
        // the child's cleanup, or one Rust destructor. The kernel closes its descriptors regardless, and the lock
        // goes with them; `wait` is what makes that observable rather than raced against.
        held.kill().expect("the child is killable");
        held.wait().expect("the child is reapable");

        acquire(dir.path(), CLI).expect("a terminated holder must release the claim");
    }

    #[test]
    fn two_configuration_directories_are_two_claims() {
        // The per-user property, checked the only way a test can: two directories that share nothing both grant.
        let (one, two) = (config_dir(), config_dir());

        let first = acquire(one.path(), GUI).expect("the first directory");
        let second = acquire(two.path(), CLI).expect("the second directory");

        assert_ne!(first.path(), second.path());
    }
}

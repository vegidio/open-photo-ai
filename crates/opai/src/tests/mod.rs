//! The library's tests, and the dependency fixtures and recorders they share.

mod claim;
mod dependencies;
mod initialize;
mod selection;

use super::*;
use crate::setup::Plan;
use deps::hf_server::{self, HfServer};
use gpu::{adapter, cuda_device};
use instance::tests::{ACQUIRED, LOCK_FILE_NAME, REFUSED, child_attempt, child_holding, claimed};
use rust_sak::sysinfo::{CudaInfo, GpuInfo};
use std::ffi::OsStr;
use std::sync::Mutex;

/// Every dependency, in install order, as a Turing-or-newer NVIDIA machine installs them.
const EVERYTHING: [Dependency; 4] = [Dependency::Runtime, Dependency::Cuda, Dependency::Cudnn, Dependency::TensorRt];

#[test]
fn version_matches_the_crate_manifest() {
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    assert!(!version().is_empty());
}

#[test]
fn the_application_name_is_the_published_one() {
    // Pinned as a literal: changing it doesn't break a build, it orphans every user's install.
    assert_eq!(APP_NAME, "io.vinicius.opai");
    assert_ne!(APP_NAME, "open-photo-ai");

    // `gui`'s own suite pins the other half.

    // Checked against the rule rather than by resolving it (which would create the real directory).
    assert!(config::validate_name(APP_NAME).is_ok(), "the application's own name is not a usable name");
}

#[test]
fn the_dependency_root_is_the_shared_one() {
    // Pinned as a literal for the same reason: every first-party application installs into it, so renaming it here
    // orphans every install on every machine and every other application's too.
    assert_eq!(deps::folders::ROOT, "io.vinicius.ai");
}

/// The runtime alone, as a machine with no NVIDIA adapter installs it, from `server`.
fn runtime_only(server: &HfServer) -> Plan {
    server.plan(&[Dependency::Runtime])
}

/// All four, as a machine with a Turing-or-newer NVIDIA card installs them, from `server`.
fn every_dependency(server: &HfServer) -> Plan {
    server.plan(&EVERYTHING)
}

/// Where `which` installed under `server`'s root.
fn installed_dir(server: &HfServer, which: &Dependency) -> PathBuf {
    rust_sak::ai::dirs::dir_for(server.root(), &server.source(which))
}

/// Asserts the fixture's files and the install marker are in `dir`.
#[track_caller]
fn assert_installed(dir: &Path) {
    for file in hf_server::INSTALLED {
        assert!(dir.join(file).is_file(), "{} has no {file}", dir.display());
    }
    assert!(dir.join(".installed").is_file(), "{} has no install marker", dir.display());
}

/// A callback that collects every report it is handed, returned with the collection.
fn recording() -> (Option<OnProgress>, Arc<Mutex<Vec<Progress>>>) {
    let (on_progress, seen) = progress::recording();
    (Some(on_progress), seen)
}

/// Every list a plan recipient was handed, in order — one entry per report, so "exactly once" is a length.
type PlanReports = Arc<Mutex<Vec<Vec<PlannedDependency>>>>;

/// A plan recipient that collects every list it is handed, returned with the collection.
fn recording_plan() -> (Option<OnPlan>, PlanReports) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let on_plan: OnPlan = Arc::new(move |rows: &[PlannedDependency]| {
        sink.lock().unwrap().push(rows.to_vec());
    });

    (Some(on_plan), seen)
}

/// The dependencies a machine with `adapters`, whose driver reported `cuda`, is given, in install order, with the
/// report an install of exactly those would make.
fn selected(adapters: &[GpuInfo], cuda: Option<&CudaInfo>) -> (Vec<Dependency>, SupportedProviders) {
    let chosen = setup::select(adapters, cuda);
    let has = |which: Dependency| chosen.contains(&which);
    let report = SupportedProviders::from(rust_sak::ai::Supported::detect(
        has(Dependency::Cuda) && has(Dependency::Cudnn),
        has(Dependency::TensorRt),
    ));
    (chosen, report)
}

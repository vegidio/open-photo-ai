//! The library's tests, and the dependency fixtures and recorders they share.

mod claim;
mod dependencies;
mod initialize;
mod selection;

use super::*;
use crate::deps::artifact::{CUDA, CUDNN, TENSORRT};
use crate::deps::release::Dependency as Descriptor;
use crate::providers::Provider;
use crate::setup::Plan;
use deps::test_server::{self, TestServer};
use gpu::adapter;
use instance::tests::{ACQUIRED, LOCK_FILE_NAME, REFUSED, child_attempt, child_holding, claimed};
use rust_sak::crypto::sha256_bytes;
use rust_sak::sysinfo::GpuInfo;
use std::ffi::OsStr;
use std::sync::Mutex;

/// The install directory of every dependency that could be selected, in the order they are installed.
fn all_dirs() -> Vec<&'static str> {
    deps::artifact::ALL.iter().map(|release| release.dir).collect()
}

/// The one file in the fixture big enough to stand for "this dependency really was installed here".
fn fixture_library() -> &'static str {
    test_server::EXTRACTED
        .iter()
        .max_by_key(|(_, size)| *size)
        .map(|(path, _)| *path)
        .expect("the fixture has files")
}

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

/// One descriptor serving the committed fixture from `server`, standing in for `release`'s published archive at
/// `version`.
///
/// Everything but version and source comes off the pinned row, so this carries the same directory/progress
/// name/provider a real install would. The fixture is the same three files for every dependency — only the
/// selection and destinations are under test.
fn fixture(server: &TestServer, release: &'static deps::artifact::Release, version: &str) -> Descriptor {
    Descriptor {
        name: release.name.to_string(),
        version: version.to_string(),
        dir: release.dir.to_string(),
        progress: release.progress.clone(),
        lib: test_server::fixture_lib(&release.progress),
        webgpu: None,
        provides: release.provides,
        derived: Vec::new(),
        sources: vec![test_server::fixture_source(server, version, &format!("{}_test.7z", release.name))],
        contents: deps::release::Contents::Archive,
        trust: ModelTrust::Published,
    }
}

/// The runtime alone, as a machine with no NVIDIA adapter installs it.
fn runtime_only(server: &TestServer) -> Plan {
    Plan { runtime: fixture(server, &ONNX_RUNTIME, "runtime/1.30.0"), gpu: Vec::new() }
}

/// All four, as a machine with an RTX card installs them.
fn every_dependency(server: &TestServer) -> Plan {
    Plan {
        runtime: fixture(server, &ONNX_RUNTIME, "runtime/1.30.0"),
        gpu: vec![
            fixture(server, &CUDA, "cuda/13.3.0"),
            fixture(server, &CUDNN, "cudnn/9.23.1"),
            fixture(server, &TENSORRT, "tensorrt/10.14.1"),
        ],
    }
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

/// The one plan report `report_plan` made for the machine `adapters` describes, as `Opai::initialize` makes it.
fn planned(adapters: &[GpuInfo], os: &'static str, arch: &'static str) -> Vec<PlannedDependency> {
    let (on_plan, seen) = recording_plan();
    let plan = Opai::select_at("http://example.invalid", adapters, os, arch).unwrap();

    Opai::report_plan(&plan, on_plan.as_ref());

    let mut reports = seen.lock().unwrap().clone();
    assert_eq!(reports.len(), 1, "the plan was not reported exactly once");
    reports.pop().expect("one report")
}

/// The published size of `release`'s archive for a platform, read off the pinned table rather than restated.
fn pinned_size(release: &deps::artifact::Release, os: &'static str, arch: &'static str) -> u64 {
    release.pinned_for(os, arch).unwrap().artifact.size
}

/// The names of the dependencies `select_at` chose, in install order, with the report derived from them.
fn selected(adapters: &[GpuInfo], os: &'static str, arch: &'static str) -> (Vec<String>, SupportedProviders) {
    let plan = Opai::select_at("http://example.invalid", adapters, os, arch).unwrap();
    (plan.all().map(|descriptor| descriptor.name.clone()).collect(), plan.providers())
}

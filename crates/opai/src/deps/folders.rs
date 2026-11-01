//! The ONNX Runtime and NVIDIA libraries this build runs on, named by the Hugging Face folder each is published in,
//! and what an NVIDIA GPU needs to run them.
//!
//! Installing, verifying, locating and loading them is `rust-sak`'s `ai`: a version bump is a one-line change to the
//! folder's URL here. A version folder on Hugging Face only ever holds that one version, so the URL names the bytes.

use std::path::{Path, PathBuf};

use rust_sak::ai::gpu::GpuRequirement;
use rust_sak::ai::{self, Source};

use crate::progress::Dependency as Which;

/// The ONNX Runtime 1.30.0, whose archive also carries the WebGPU plugin.
const RUNTIME: &str = "https://huggingface.co/vegidio/general/tree/main/deps/onnx/1.30.0";
/// NVIDIA CUDA 13.3.0.
const CUDA: &str = "https://huggingface.co/vegidio/general/tree/main/deps/cuda/13.3.0";
/// NVIDIA cuDNN 9.23.1, which links against the CUDA runtime.
const CUDNN: &str = "https://huggingface.co/vegidio/general/tree/main/deps/cudnn/9.23.1";
/// NVIDIA TensorRT 10.14.1, which runs on CUDA.
const TENSORRT: &str = "https://huggingface.co/vegidio/general/tree/main/deps/tensorrt/10.14.1";

/// What CUDA 13.3 and cuDNN 9.23 need: a driver for CUDA 13.0 or newer, and a Turing-or-newer device.
pub(crate) const CUDA_REQUIREMENT: GpuRequirement = GpuRequirement { min_driver: (13, 0), min_compute: (7, 5) };

/// What TensorRT 10.14 needs on top of CUDA: a device it can build engines for.
pub(crate) const TENSORRT_REQUIREMENT: GpuRequirement = GpuRequirement { min_driver: (13, 0), min_compute: (7, 5) };

/// The folder in the user's configuration directory the runtime and the NVIDIA libraries install into, shared with
/// every other application that installs from the same Hugging Face folders.
pub(crate) const ROOT: &str = "io.vinicius.ai";

/// Every dependency, in install order: the runtime, then cuDNN after the CUDA it links against, then TensorRT, which at
/// 1.4-2 GB is larger than the other three combined, so an interrupted first launch already has the two that unlock
/// CUDA.
pub(crate) const ALL: [Which; 4] = [Which::Runtime, Which::Cuda, Which::Cudnn, Which::TensorRt];

/// The folder `which` is published in.
///
/// # Panics
///
/// For [`Which::Model`], which is not one of these.
pub(crate) fn source(which: &Which) -> Source {
    let url = match which {
        Which::Runtime => RUNTIME,
        Which::Cuda => CUDA,
        Which::Cudnn => CUDNN,
        Which::TensorRt => TENSORRT,
        Which::Model(artifact) => unreachable!("{artifact} is a model, not a runtime dependency"),
    };
    Source::parse(url).expect("every folder above is a Hugging Face tree URL")
}

/// `which` as `rust-sak` installs it, named as the front ends have always shown it.
pub(crate) fn dependency(which: &Which) -> ai::Dependency {
    let name = name(which);
    match which {
        Which::Runtime => ai::Dependency::runtime(name, source(which)),
        _ => ai::Dependency::libraries(name, source(which)),
    }
}

/// The display name `rust-sak` carries in its events and errors, which [`which`] maps back.
fn name(which: &Which) -> &'static str {
    match which {
        Which::Runtime => "ONNX Runtime",
        Which::Cuda => "NVIDIA CUDA",
        Which::Cudnn => "NVIDIA cuDNN",
        Which::TensorRt => "NVIDIA TensorRT",
        Which::Model(artifact) => unreachable!("{artifact} is a model, not a runtime dependency"),
    }
}

/// The dependency `rust-sak` named `name`, one of [`ALL`].
pub(crate) fn which(name: &str) -> Option<Which> {
    ALL.into_iter().find(|which| self::name(which) == name)
}

/// The runtime's version, the last segment of its folder: what a compiled provider artifact is valid for.
pub(crate) fn runtime_version() -> String {
    source(&Which::Runtime).version().to_string()
}

/// The folder each of [`ALL`] installs into under `root`, in install order. Computed, never created: nothing here
/// touches the disk or the network.
pub(crate) fn dirs_under(root: &Path) -> Vec<PathBuf> {
    ALL.iter().map(|which| ai::dirs::dir_for(root, &source(which))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_dependency_is_a_folder_of_its_own_under_the_root() {
        let root = Path::new("/config/io.vinicius.ai");
        assert_eq!(
            dirs_under(root),
            [
                root.join("onnx/1.30.0"),
                root.join("cuda/13.3.0"),
                root.join("cudnn/9.23.1"),
                root.join("tensorrt/10.14.1"),
            ]
        );
        assert_eq!(runtime_version(), "1.30.0");
    }

    #[test]
    fn every_name_maps_back_to_its_dependency_and_keeps_the_go_apps_wording() {
        for which in ALL {
            let dependency = dependency(&which);
            assert_eq!(dependency.name, which.as_str(), "the name the front ends have always shown");
            assert_eq!(super::which(dependency.name), Some(which));
        }
        assert_eq!(dependency(&Which::Runtime).kind, ai::Kind::Runtime);
        assert_eq!(dependency(&Which::Cuda).kind, ai::Kind::Libraries);
        assert_eq!(super::which("something else"), None);
    }
}

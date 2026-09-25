//! The pinned identity of every published dependency archive, and the lookup that picks this platform's.

use crate::error::InitError;
use crate::progress::Dependency;
use crate::providers::Provider;

// The assets were named from Go's `GOOS`, and those names are fixed in releases that already exist — which is why
// `macos` is `Darwin` here.
/// An operating system the release assets are named for, spelled as the release spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Os {
    /// macOS, which the assets call `darwin`.
    Darwin,
    Linux,
    Windows,
}

/// An architecture the release assets are named for, likewise from Go's `GOARCH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arch {
    /// 64-bit x86, which the assets call `amd64` and Rust calls `x86_64`.
    Amd64,
    /// 64-bit ARM, which the assets call `arm64` and Rust calls `aarch64`.
    Arm64,
}

// A pair of enums rather than a free-form string. A string has to match what the mapper spells with nothing
// connecting the two, so a typo — `win_amd64`, `darwin_aarch64` — is not a compile error: it silently makes that
// platform unpublished, and the first person to find out is a user on it. There is no way to write one of those down
// here.
/// The platform one published archive is built for, and the key the table is looked up by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Platform {
    /// The operating system the archive is built for.
    pub(crate) os: Os,
    /// The architecture it is built for.
    pub(crate) arch: Arch,
}

impl Platform {
    /// The pair, written the way every row and every lookup writes it.
    pub(crate) const fn new(os: Os, arch: Arch) -> Self {
        Self { os, arch }
    }

    /// The platform segment of an asset's name: `darwin_arm64`.
    pub(crate) const fn segment(self) -> &'static str {
        // The one place the segment is spelled, so the names in the table and the names the lookup builds are the same
        // strings rather than two spellings that have to agree.
        match (self.os, self.arch) {
            (Os::Darwin, Arch::Amd64) => "darwin_amd64",
            (Os::Darwin, Arch::Arm64) => "darwin_arm64",
            (Os::Linux, Arch::Amd64) => "linux_amd64",
            (Os::Linux, Arch::Arm64) => "linux_arm64",
            (Os::Windows, Arch::Amd64) => "windows_amd64",
            (Os::Windows, Arch::Arm64) => "windows_arm64",
        }
    }

    /// Maps a [`std::env::consts`] pair onto the platform the assets are named for.
    ///
    /// An unmapped pair yields `None` and becomes an error naming the platform — never a default.
    pub(crate) fn from_consts(os: &str, arch: &str) -> Option<Self> {
        // Here rather than in the table because it is a translation between two vocabularies, not a property of any
        // row.
        let os = match os {
            "macos" => Os::Darwin,
            "linux" => Os::Linux,
            "windows" => Os::Windows,
            _ => return None,
        };
        let arch = match arch {
            "x86_64" => Arch::Amd64,
            "aarch64" => Arch::Arm64,
            _ => return None,
        };

        Some(Self::new(os, arch))
    }
}

/// One published archive, pinned.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Artifact {
    /// The platform this archive is built for. Its [`Platform::segment`] is what the asset's name carries.
    pub(crate) platform: Platform,
    /// SHA-256 of the `.7z` **as published** — of the archive, not of anything inside it. One value therefore covers
    /// every file the archive carries, which is the only way a runtime shipped alongside its execution providers can
    /// be verified at all.
    pub(crate) hash: &'static str,
    /// The published size in bytes, used only to spread a progress report before the response declares a length.
    pub(crate) size: u64,
    // An `Option` rather than an empty string, so "this dependency has no library" is a case a caller has to handle
    // rather than a value it can join onto a directory and get the directory back.
    //
    // It carries the version too, so it lives beside the hash of the archive it comes out of rather than in a
    // per-platform constant elsewhere: a bump that edited one and forgot the other would verify a correct archive and
    // then point at a file that no longer exists.
    /// The shared library inside the archive the loader has to be pointed at, or `None` where the archive is a
    /// directory of several hundred files rather than one loadable file.
    pub(crate) lib: Option<&'static str>,
}

// `Clone` but not `Copy`, because `Dependency` is not: its model variant names which model, and that name is composed
// at run time. Nothing copies a row anyway — the table is `&'static` and every reader borrows from it.
/// One dependency pinned to one published release.
#[derive(Debug, Clone)]
pub(crate) struct Release {
    /// How the dependency is named to a user, and in its manifest.
    pub(crate) name: &'static str,
    /// The asset family, the first segment of every asset's file name: `onnx`.
    pub(crate) prefix: &'static str,
    // Written once, for the dependency rather than for each platform, so two versions of it cannot be expressed at
    // all and a bump is one edit.
    /// The release the archives come from, and the version this build installs.
    pub(crate) tag: &'static str,
    // On the row rather than at the call site because the alternative is a `match` elsewhere that has to be kept in
    // step with this table, and the failure mode of getting it wrong is installing cuDNN's files into CUDA's
    // directory — where the next CUDA bump deletes them.
    /// The subdirectory of the application's configuration directory this dependency installs into: `runtime`,
    /// `libs/cuda`.
    pub(crate) dir: &'static str,
    // On the row for the same reason as `dir`.
    /// Which [`Dependency`] this row's progress reports name themselves as.
    pub(crate) progress: Dependency,
    // On the row for the same reason as `dir`: it is what lets the provider report be derived from the plan that was
    // actually installed instead of assigned beside it, so the two cannot disagree.
    /// The execution provider installing this dependency unlocks, or `None` where it unlocks none of its own.
    pub(crate) provides: Option<Provider>,
    /// The pinned archive for each platform the dependency is published for. A platform with no entry is one it is
    /// not built for.
    pub(crate) archives: &'static [Artifact],
}

/// The ONNX Runtime, pinned to `runtime/1.30.0`.
///
/// Ships the WebGPU plugin execution provider (`[lib]onnxruntime_providers_webgpu`) beside the runtime on every
/// platform — and, on Windows, the `dxcompiler.dll`/`dxil.dll` pair that plugin compiles its shaders with. From this
/// release on the libraries keep upstream's file names (`libonnxruntime.1.30.0.dylib`, `onnxruntime.dll`) rather than
/// the renamed ones earlier tags carried.
pub(crate) const ONNX_RUNTIME: Release = Release {
    name: "onnx-runtime",
    prefix: "onnx",
    // One thing outside this file has to move with the tag: the `api-28` feature on `ort` in `crates/opai/Cargo.toml`.
    // `ort` refuses to load a runtime older than the minor version that feature names, so bumping this tag without
    // bumping the feature leaves the two disagreeing — and the disagreement is a startup error on every machine, not a
    // compile failure here.
    tag: "runtime/1.30.0",
    dir: "runtime",
    progress: Dependency::Runtime,
    // The runtime is what an execution provider runs on, not one of them.
    provides: None,
    // Re-derivable with:
    //
    //     gh api repos/vegidio/open-photo-ai/releases/tags/runtime%2F1.30.0 \
    //       --jq '.assets[] | "\(.name) \(.size) \(.digest)"'
    archives: &[
        Artifact {
            platform: Platform::new(Os::Darwin, Arch::Arm64),
            hash: "f58971f0f556567e9b7fcce6a0110c5072ff003d9a1bb1214060b188f5d11178",
            size: 10_043_605,
            lib: Some("libonnxruntime.1.30.0.dylib"),
        },
        Artifact {
            platform: Platform::new(Os::Linux, Arch::Amd64),
            hash: "2a9fadb295f66fca1480bad457355c7044bf14ca06927468b991996764ad5d37",
            size: 190_133_628,
            lib: Some("libonnxruntime.so.1.30.0"),
        },
        Artifact {
            platform: Platform::new(Os::Linux, Arch::Arm64),
            hash: "50c3dc45b758105bec33f640c3db9f4efad97c99b3f99c2be52e00cfe89b6811",
            size: 9_702_215,
            lib: Some("libonnxruntime.so.1.30.0"),
        },
        Artifact {
            platform: Platform::new(Os::Windows, Arch::Amd64),
            hash: "cc13165ce3df6747a561db8bc06ebdd05bf21135808171a5f76245d8bdb10a63",
            size: 144_411_038,
            lib: Some("onnxruntime.dll"),
        },
        Artifact {
            platform: Platform::new(Os::Windows, Arch::Arm64),
            hash: "07d64750a60aee7e84443e21471a9811fb8114f50b26e19f8daf336b84028327",
            size: 12_250_575,
            lib: Some("onnxruntime.dll"),
        },
    ],
};

/// The CUDA runtime libraries, pinned to `cuda/13.3.0`.
///
/// Published for `linux_amd64`, `linux_arm64` and `windows_amd64` only — there is no NVIDIA hardware to serve on
/// macOS, and no `windows_arm64` build exists. A platform with no row here is not an error for a GPU library; it is
/// a provider that is reported unsupported and never attempted.
pub(crate) const CUDA: Release = Release {
    name: "cuda",
    prefix: "cuda",
    tag: "cuda/13.3.0",
    dir: "libs/cuda",
    progress: Dependency::Cuda,
    provides: Some(Provider::Cuda),
    // Re-derivable with:
    //
    //     gh api repos/vegidio/open-photo-ai/releases/tags/cuda%2F13.3.0 \
    //       --jq '.assets[] | "\(.name) \(.size) \(.digest)"'
    archives: &[
        Artifact {
            platform: Platform::new(Os::Linux, Arch::Amd64),
            hash: "fb4eb6ef362973c6cdefeae43e09cea05270288acbdd47cc0fc4f50ca6bc47c0",
            size: 587_322_552,
            lib: None,
        },
        Artifact {
            platform: Platform::new(Os::Linux, Arch::Arm64),
            hash: "ed5eb63005e30a098c270b2020b66eab87a5142d9936886fe820c19693487a24",
            size: 697_083_065,
            lib: None,
        },
        Artifact {
            platform: Platform::new(Os::Windows, Arch::Amd64),
            hash: "b85d858e4fd97bab2c808c7ad7106a89dc8dfba535a0f20f63c4decc4d24452b",
            size: 585_185_362,
            lib: None,
        },
    ],
};

/// cuDNN, pinned to `cudnn/9.23.1`. Installed after CUDA, whose runtime its libraries link against.
///
/// Published for the same three platforms as [`CUDA`].
pub(crate) const CUDNN: Release = Release {
    name: "cudnn",
    prefix: "cudnn",
    tag: "cudnn/9.23.1",
    dir: "libs/cudnn",
    progress: Dependency::Cudnn,
    // cuDNN unlocks nothing of its own: it is a prerequisite CUDA's provider needs on disk beside it.
    provides: None,
    archives: &[
        Artifact {
            platform: Platform::new(Os::Linux, Arch::Amd64),
            hash: "18e84817dd836046087ece4b6776fea066440854c48b6b6e6c4a388b43df4174",
            size: 407_298_251,
            lib: None,
        },
        Artifact {
            platform: Platform::new(Os::Linux, Arch::Arm64),
            hash: "92537e1ed61579840b12282c70633f0ef6bfd3929d38943e31ebcd6b699b62d6",
            size: 507_936_847,
            lib: None,
        },
        Artifact {
            platform: Platform::new(Os::Windows, Arch::Amd64),
            hash: "a26f54c17ea990e59a6f7232bdebf39f58b5debd9a0c8ec9a6573885ae27edbd",
            size: 354_760_387,
            lib: None,
        },
    ],
};

/// TensorRT, pinned to `tensorrt/10.14.1`. Installed last: at 1.4–2 GB it is larger than the other three put
/// together, so an interrupted first launch has the two that unlock CUDA already on disk.
///
/// Published for the same three platforms as [`CUDA`].
pub(crate) const TENSORRT: Release = Release {
    name: "tensorrt",
    prefix: "tensorrt",
    tag: "tensorrt/10.14.1",
    dir: "libs/tensorrt",
    progress: Dependency::TensorRt,
    provides: Some(Provider::TensorRt),
    archives: &[
        Artifact {
            platform: Platform::new(Os::Linux, Arch::Amd64),
            hash: "d2a29e4fbc78445ae715ff7ff6fde6f59ba6425a8b0017753f3a776096c59376",
            size: 1_780_420_117,
            lib: None,
        },
        Artifact {
            platform: Platform::new(Os::Linux, Arch::Arm64),
            hash: "28a7699c9208d6d1a5f77f65bd2dad9547a6245599ae5192c8a1baf4ef1496a0",
            size: 1_993_059_105,
            lib: None,
        },
        Artifact {
            platform: Platform::new(Os::Windows, Arch::Amd64),
            hash: "1156bf236dd66aa7c4772f8144599b81061512d035d05f02c037e1a1ad20370b",
            size: 1_399_566_073,
            lib: None,
        },
    ],
};

// The one list. `dir` lives on the row rather than at a call site for the reason given beside `Release::dir`, and this
// constant is what stops that argument from being undone by a second list elsewhere: a dependency added to the table
// is on the loader's search path by construction, rather than only once somebody remembers to add it there too.
/// Every dependency this application can install, in the order they are installed — which is also the order their
/// directories go on the dynamic loader's search path.
pub(crate) const ALL: &[&Release] = &[&ONNX_RUNTIME, &CUDA, &CUDNN, &TENSORRT];

/// A release resolved to the one archive published for a given platform.
#[derive(Debug, Clone)]
pub(crate) struct Pinned {
    /// The release asset's file name: `onnx_darwin_arm64.7z`.
    pub(crate) asset: String,
    /// The release it belongs to: `runtime/1.30.0`.
    pub(crate) tag: &'static str,
    /// The archive's pinned identity.
    pub(crate) artifact: Artifact,
}

impl Release {
    /// Resolves this release to the archive published for a platform — [`std::env::consts`] in production, and an
    /// explicit pair under test so every pinned platform is checked on whichever runner the suite happens to be on.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::UnsupportedPlatform`] when nothing is published for it.
    pub(crate) fn pinned_for(&self, os: &'static str, arch: &'static str) -> Result<Pinned, InitError> {
        // A `Result` rather than the reference's lookup, which answers a missing platform with a zero value: its
        // dependency installs against an empty expected hash — which means "do not verify" — and nothing says so.
        let unsupported = || InitError::UnsupportedPlatform { dependency: self.name, os, arch };

        let platform = Platform::from_consts(os, arch).ok_or_else(unsupported)?;
        let artifact = self.archives.iter().find(|artifact| artifact.platform == platform).ok_or_else(unsupported)?;

        Ok(Pinned { asset: format!("{}_{}.7z", self.prefix, platform.segment()), tag: self.tag, artifact: *artifact })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pinned_platform_resolves_to_its_own_asset() {
        let cases = [
            ("macos", "aarch64", "onnx_darwin_arm64.7z"),
            ("linux", "x86_64", "onnx_linux_amd64.7z"),
            ("linux", "aarch64", "onnx_linux_arm64.7z"),
            ("windows", "x86_64", "onnx_windows_amd64.7z"),
            ("windows", "aarch64", "onnx_windows_arm64.7z"),
        ];

        for (os, arch, asset) in cases {
            let pinned = ONNX_RUNTIME.pinned_for(os, arch).unwrap();
            assert_eq!(pinned.asset, asset, "{os}/{arch}");
            assert_eq!(pinned.tag, "runtime/1.30.0");
        }

        assert_eq!(cases.len(), ONNX_RUNTIME.archives.len(), "a pinned platform is untested");
    }

    #[test]
    fn a_platform_with_no_published_archive_is_an_error_not_an_empty_hash() {
        // The failure this guards: a missing entry answered with a zero value, where an empty expected hash means
        // "do not verify".
        for (os, arch) in [("macos", "x86_64"), ("freebsd", "x86_64"), ("linux", "riscv64")] {
            let error = ONNX_RUNTIME.pinned_for(os, arch).unwrap_err();
            match error {
                InitError::UnsupportedPlatform { dependency, os: o, arch: a, .. } => {
                    assert_eq!(dependency, "onnx-runtime");
                    assert_eq!((o, a), (os, arch), "the error must name the platform it rejected");
                }
                other => panic!("expected UnsupportedPlatform for {os}/{arch}, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_running_platform_is_one_of_the_pinned_ones() {
        // Every target this project ships for is in the table; a build on one that is not should fail here rather
        // than at a user's first launch.
        let pinned = ONNX_RUNTIME.pinned_for(std::env::consts::OS, std::env::consts::ARCH).unwrap();
        assert_eq!(pinned.artifact.hash.len(), 64);
        assert!(pinned.artifact.size > 0);
        assert!(pinned.artifact.lib.is_some());
    }

    /// Every GPU library, with the asset prefix each one's archives are named from.
    const GPU_LIBRARIES: &[(&Release, &str)] = &[(&CUDA, "cuda"), (&CUDNN, "cudnn"), (&TENSORRT, "tensorrt")];

    #[test]
    fn each_gpu_library_resolves_to_its_own_asset_on_every_published_platform() {
        for (release, prefix) in GPU_LIBRARIES {
            let cases = [
                ("linux", "x86_64", "linux_amd64"),
                ("linux", "aarch64", "linux_arm64"),
                ("windows", "x86_64", "windows_amd64"),
            ];

            for (os, arch, platform) in cases {
                let pinned = release.pinned_for(os, arch).unwrap();
                assert_eq!(pinned.asset, format!("{prefix}_{platform}.7z"), "{prefix} on {os}/{arch}");
                assert_eq!(pinned.tag, release.tag);
                // No single library the loader is pointed at: a CUDA tree is several hundred files, and it is the
                // directory that goes on the search path.
                assert_eq!(pinned.artifact.lib, None, "{prefix} named a library");
            }

            assert_eq!(cases.len(), release.archives.len(), "{prefix} has a pinned platform that is untested");
        }
    }

    #[test]
    fn each_gpu_library_installs_into_a_directory_of_its_own() {
        // No two share one, and none shares the runtime's: replacing one must not delete or keep another's files.
        let dirs: Vec<&str> = ALL.iter().map(|release| release.dir).collect();

        assert_eq!(dirs, vec!["runtime", "libs/cuda", "libs/cudnn", "libs/tensorrt"]);
        assert_eq!(
            dirs.iter().collect::<std::collections::BTreeSet<_>>().len(),
            dirs.len(),
            "two dependencies install into the same directory"
        );
    }

    #[test]
    fn a_gpu_library_names_the_progress_variant_its_reports_carry() {
        assert_eq!(CUDA.progress, Dependency::Cuda);
        assert_eq!(CUDNN.progress, Dependency::Cudnn);
        assert_eq!(TENSORRT.progress, Dependency::TensorRt);
    }

    #[test]
    fn each_row_names_the_provider_installing_it_unlocks_and_no_other() {
        // The row is what the provider report is derived from, so getting this wrong is not a mismatched label — it
        // is claiming a provider whose library was never installed.
        assert_eq!(ONNX_RUNTIME.provides, None, "the runtime is what a provider runs on, not one of them");
        assert_eq!(CUDA.provides, Some(Provider::Cuda));
        assert_eq!(CUDNN.provides, None, "cuDNN is a prerequisite of CUDA's provider, not a provider of its own");
        assert_eq!(TENSORRT.provides, Some(Provider::TensorRt));
    }

    #[test]
    fn a_gpu_library_is_unpublished_rather_than_empty_on_windows_arm64_and_macos() {
        // The platforms an NVIDIA card can actually be detected on with nothing published for it, plus macOS where
        // there is no such card at all. Both must be an error rather than a zero value: an empty expected hash means
        // "do not verify".
        for (release, name) in [(&CUDA, "cuda"), (&CUDNN, "cudnn"), (&TENSORRT, "tensorrt")] {
            for (os, arch) in [("windows", "aarch64"), ("macos", "aarch64"), ("macos", "x86_64")] {
                let error = release.pinned_for(os, arch).unwrap_err();
                match error {
                    InitError::UnsupportedPlatform { dependency, os: o, arch: a, .. } => {
                        assert_eq!(dependency, name);
                        assert_eq!((o, a), (os, arch), "the error must name the platform it rejected");
                    }
                    other => panic!("expected UnsupportedPlatform for {name} on {os}/{arch}, got {other:?}"),
                }
            }
        }
    }

    #[test]
    fn every_pinned_platform_is_one_the_mapper_can_actually_produce() {
        // A typo in a row is a compile error rather than a platform that is quietly unpublished, so what is left to
        // check is the other half: that the mapper is still total over the platforms the table pins. A `Platform`
        // no `from_consts` answer ever equals is an archive nothing can reach, and the first person to find out
        // would be a user on it getting `UnsupportedPlatform`.
        let reachable: Vec<Platform> = ["macos", "linux", "windows"]
            .into_iter()
            .flat_map(|os| ["x86_64", "aarch64"].into_iter().filter_map(move |arch| Platform::from_consts(os, arch)))
            .collect();

        for release in ALL {
            for artifact in release.archives {
                assert!(
                    reachable.contains(&artifact.platform),
                    "{} pins {:?}, which `Platform::from_consts` never produces — the archive is unreachable",
                    release.name,
                    artifact.platform
                );
            }
        }
    }

    #[test]
    fn every_platform_spells_the_segment_the_published_assets_already_carry() {
        // The release assets exist and their names cannot change, so this is the one mapping in the crate that is
        // pinned by history rather than by choice.
        let cases = [
            ("macos", "aarch64", "darwin_arm64"),
            ("linux", "x86_64", "linux_amd64"),
            ("linux", "aarch64", "linux_arm64"),
            ("windows", "x86_64", "windows_amd64"),
            ("windows", "aarch64", "windows_arm64"),
        ];

        for (os, arch, segment) in cases {
            assert_eq!(Platform::from_consts(os, arch).unwrap().segment(), segment, "{os}/{arch}");
        }

        // `darwin_amd64` is spellable and nothing is published for it, which is the point: the type says what a
        // platform *is*, and the table says what is built for it.
        assert_eq!(Platform::new(Os::Darwin, Arch::Amd64).segment(), "darwin_amd64");
    }

    #[test]
    fn a_platform_outside_the_two_vocabularies_maps_to_nothing() {
        for (os, arch) in [("freebsd", "x86_64"), ("linux", "riscv64"), ("", "")] {
            assert_eq!(Platform::from_consts(os, arch), None, "{os}/{arch}");
        }
    }

    #[test]
    fn every_pinned_hash_is_a_full_sha256() {
        for release in ALL {
            for artifact in release.archives {
                assert_eq!(artifact.hash.len(), 64, "{} {}", release.name, artifact.platform.segment());
                assert!(
                    artifact.hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                    "{} {} is not lowercase hex",
                    release.name,
                    artifact.platform.segment()
                );
                assert!(artifact.size > 0, "{} {}", release.name, artifact.platform.segment());
            }
        }
    }
}

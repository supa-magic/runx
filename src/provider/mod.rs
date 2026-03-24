pub mod deno;
pub mod go;
pub mod node;
pub mod python;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::platform::Target;
use crate::version::VersionSpec;

pub use deno::DenoProvider;
pub use go::GoProvider;
pub use node::NodeProvider;
pub use python::PythonProvider;

/// Look up a provider by tool name.
///
/// Returns the appropriate `Provider` implementation for the given tool,
/// or an error if the tool is not supported.
pub fn get_provider(name: &str) -> Result<Box<dyn Provider>, ProviderError> {
    match name {
        "node" | "nodejs" => Ok(Box::new(NodeProvider)),
        "python" | "python3" => Ok(Box::new(PythonProvider)),
        "go" | "golang" => Ok(Box::new(GoProvider)),
        "deno" => Ok(Box::new(DenoProvider)),
        other => Err(ProviderError::UnknownTool {
            name: other.to_string(),
        }),
    }
}

// --- Shared helpers ---

/// Fetch JSON from a URL using blocking HTTP inside an async context.
///
/// Uses `tokio::task::block_in_place` to bridge blocking `reqwest` into
/// the tokio runtime. All providers use this to avoid duplicating HTTP logic.
pub fn fetch_json(url: &str, tool: &'static str) -> Result<String, ProviderError> {
    tokio::task::block_in_place(|| {
        reqwest::blocking::Client::new()
            .get(url)
            .header("User-Agent", "runx")
            .header("Accept", "application/json")
            .send()
            .map_err(|e| ProviderError::ResolutionFailed {
                tool: tool.to_string(),
                reason: format!("{e:#}"),
            })?
            .text()
            .map_err(|e| ProviderError::ResolutionFailed {
                tool: tool.to_string(),
                reason: format!("{e:#}"),
            })
    })
}

/// Collect unique stable versions from an iterator of optional versions.
///
/// Filters out pre-release versions and duplicates. Uses a HashSet
/// for O(n) deduplication instead of O(n^2) Vec::contains.
pub fn collect_stable_versions(
    versions: impl Iterator<Item = Option<semver::Version>>,
) -> Vec<semver::Version> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for ver in versions.flatten() {
        if ver.pre.is_empty() && seen.insert(ver.clone()) {
            result.push(ver);
        }
    }
    result
}

/// Resolve a version spec against a list of candidates.
///
/// Shared logic used by all providers: fetch candidates, resolve, or return VersionNotFound.
pub fn resolve_from_candidates(
    candidates: &[semver::Version],
    spec: &VersionSpec,
    tool: &'static str,
) -> Result<semver::Version, ProviderError> {
    spec.resolve(candidates)
        .cloned()
        .ok_or_else(|| ProviderError::VersionNotFound {
            tool: tool.to_string(),
            spec: spec.to_string(),
        })
}

// --- Types ---

/// Metadata about a resolved tool version ready for download.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct ResolvedTool {
    /// The tool name (e.g., "node").
    pub name: String,
    /// The exact resolved version.
    pub version: semver::Version,
    /// Download URL for the binary archive.
    pub download_url: String,
    /// Expected archive format.
    pub archive_format: ArchiveFormat,
}

/// Supported archive formats for tool downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    TarGz,
    #[allow(unused)]
    TarXz,
    Zip,
}

/// Trait that all tool providers must implement.
///
/// Each provider knows how to resolve versions, construct download URLs,
/// and describe the binary layout and environment variables for a specific tool.
///
/// **Note:** `bin_paths` returns paths relative to the install directory
/// after archive extraction. These must be directories (not individual files)
/// since they become `PATH` entries. Providers may override the default
/// `archive_format` from the platform if the tool uses a different format.
pub trait Provider {
    /// The tool name (e.g., "node", "python", "go").
    fn name(&self) -> &str;

    /// Resolve a version spec to an exact version by querying upstream.
    fn resolve_version(
        &self,
        spec: &VersionSpec,
        target: &Target,
    ) -> Result<semver::Version, ProviderError>;

    /// Construct the download URL for a specific version and target.
    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError>;

    /// Return the archive format used for the given target.
    fn archive_format(&self, target: &Target) -> ArchiveFormat;

    /// Return directory paths relative to the install directory for PATH.
    ///
    /// These must be directories, not individual executables.
    fn bin_paths(&self, version: &semver::Version, target: &Target) -> Vec<PathBuf>;

    /// Return environment variables to set for this tool.
    fn env_vars(&self, install_dir: &std::path::Path) -> HashMap<String, String>;
}

/// Errors that occur during provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The requested tool is not supported.
    #[error("unknown tool `{name}`. Supported tools: node, python, go, deno")]
    UnknownTool { name: String },

    /// No version matched the given spec.
    #[error("no {tool} version found matching `{spec}`")]
    VersionNotFound { tool: String, spec: String },

    /// The tool does not support this platform/architecture combination.
    #[allow(unused)]
    #[error("{tool} does not support target `{target}`")]
    UnsupportedTarget { tool: String, target: String },

    /// Network or API error during version resolution.
    #[error("failed to resolve {tool} version: {reason}")]
    ResolutionFailed { tool: String, reason: String },
}

/// Shared test helpers for provider tests.
#[cfg(test)]
pub mod test_helpers {
    use crate::platform::{Arch, Platform, Target};

    pub fn macos_arm64() -> Target {
        Target::new(Platform::MacOS, Arch::Aarch64)
    }

    pub fn linux_x64() -> Target {
        Target::new(Platform::Linux, Arch::X86_64)
    }

    pub fn linux_arm64() -> Target {
        Target::new(Platform::Linux, Arch::Aarch64)
    }

    pub fn windows_x64() -> Target {
        Target::new(Platform::Windows, Arch::X86_64)
    }

    pub fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::platform::{Arch, Platform, Target};

    use super::*;

    #[test]
    fn test_provider_error_display_version_not_found() {
        let err = ProviderError::VersionNotFound {
            tool: "node".to_string(),
            spec: "99".to_string(),
        };
        assert_eq!(err.to_string(), "no node version found matching `99`");
    }

    #[test]
    fn test_provider_error_display_unsupported_target() {
        let err = ProviderError::UnsupportedTarget {
            tool: "bun".to_string(),
            target: "Windows-x86_64".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "bun does not support target `Windows-x86_64`"
        );
    }

    #[test]
    fn test_provider_error_display_resolution_failed() {
        let err = ProviderError::ResolutionFailed {
            tool: "python".to_string(),
            reason: "network timeout".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to resolve python version: network timeout"
        );
    }

    #[test]
    fn test_archive_format_equality() {
        assert_eq!(ArchiveFormat::TarGz, ArchiveFormat::TarGz);
        assert_ne!(ArchiveFormat::TarGz, ArchiveFormat::Zip);
        assert_ne!(ArchiveFormat::TarXz, ArchiveFormat::Zip);
        assert_eq!(ArchiveFormat::TarXz, ArchiveFormat::TarXz);
    }

    #[test]
    fn test_provider_error_is_std_error() {
        let err = ProviderError::VersionNotFound {
            tool: "node".to_string(),
            spec: "99".to_string(),
        };
        let boxed: Box<dyn std::error::Error> = Box::new(err);
        assert!(boxed.source().is_none());
    }

    #[test]
    fn test_collect_stable_versions() {
        let versions = collect_stable_versions(
            vec![
                Some(semver::Version::new(1, 0, 0)),
                None,
                Some(semver::Version::new(2, 0, 0)),
                Some(semver::Version::new(1, 0, 0)), // duplicate
            ]
            .into_iter(),
        );
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_collect_stable_versions_filters_prerelease() {
        let pre = semver::Version::parse("1.0.0-alpha.1").unwrap();
        let stable = semver::Version::new(1, 0, 0);
        let versions = collect_stable_versions(vec![Some(pre), Some(stable.clone())].into_iter());
        assert_eq!(versions, vec![stable]);
    }

    /// Dummy provider to verify the trait is implementable.
    struct DummyProvider;

    impl Provider for DummyProvider {
        fn name(&self) -> &str {
            "dummy"
        }

        fn resolve_version(
            &self,
            _spec: &VersionSpec,
            _target: &Target,
        ) -> Result<semver::Version, ProviderError> {
            Ok(semver::Version::new(1, 0, 0))
        }

        fn download_url(
            &self,
            _version: &semver::Version,
            _target: &Target,
        ) -> Result<String, ProviderError> {
            Ok("https://example.com/dummy-1.0.0.tar.gz".to_string())
        }

        fn archive_format(&self, _target: &Target) -> ArchiveFormat {
            ArchiveFormat::TarGz
        }

        fn bin_paths(&self, _version: &semver::Version, _target: &Target) -> Vec<PathBuf> {
            vec![PathBuf::from("bin/dummy")]
        }

        fn env_vars(&self, install_dir: &std::path::Path) -> HashMap<String, String> {
            HashMap::from([(
                "DUMMY_HOME".to_string(),
                install_dir.to_string_lossy().to_string(),
            )])
        }
    }

    #[test]
    fn test_dummy_provider_implements_trait() {
        let provider = DummyProvider;
        let target = Target::new(Platform::MacOS, Arch::Aarch64);
        let spec = VersionSpec::Latest;

        assert_eq!(provider.name(), "dummy");
        let version = provider.resolve_version(&spec, &target).unwrap();
        assert_eq!(version, semver::Version::new(1, 0, 0));
        let url = provider.download_url(&version, &target).unwrap();
        assert!(url.contains("dummy"));
        assert_eq!(provider.archive_format(&target), ArchiveFormat::TarGz);
        assert!(!provider.bin_paths(&version, &target).is_empty());
        assert!(
            provider
                .env_vars(std::path::Path::new("/tmp/dummy"))
                .contains_key("DUMMY_HOME")
        );
    }
}

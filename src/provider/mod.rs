pub mod go;
pub mod node;
pub mod python;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::platform::Target;
use crate::version::VersionSpec;

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
        other => Err(ProviderError::UnknownTool {
            name: other.to_string(),
        }),
    }
}

/// Metadata about a resolved tool version ready for download.
#[derive(Debug, Clone)]
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
    TarXz,
    Zip,
}

/// Trait that all tool providers must implement.
///
/// Each provider knows how to resolve versions, construct download URLs,
/// and describe the binary layout and environment variables for a specific tool.
pub trait Provider {
    /// The tool name (e.g., "node", "python", "go").
    fn name(&self) -> &str;

    /// Resolve a version spec to an exact version by querying upstream.
    ///
    /// For example, `@18` might resolve to `18.19.1` by checking
    /// the tool's version index.
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

    /// Return paths to binary executables relative to the install directory.
    ///
    /// For Node.js this might return `["bin/node", "bin/npm", "bin/npx"]` on Unix
    /// or `["node.exe", "npm.cmd", "npx.cmd"]` on Windows.
    fn bin_paths(&self, version: &semver::Version, target: &Target) -> Vec<PathBuf>;

    /// Return environment variables to set for this tool.
    ///
    /// Keys are env var names (e.g., `NODE_HOME`), values are paths
    /// relative to the install directory.
    fn env_vars(&self, install_dir: &std::path::Path) -> HashMap<String, String>;
}

/// Errors that occur during provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The requested tool is not supported.
    #[error("unknown tool `{name}`. Supported tools: node, python, go")]
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
        // Verify it can be used as Box<dyn Error>
        let boxed: Box<dyn std::error::Error> = Box::new(err);
        assert!(boxed.source().is_none());
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
            let mut vars = HashMap::new();
            vars.insert(
                "DUMMY_HOME".to_string(),
                install_dir.to_string_lossy().to_string(),
            );
            vars
        }
    }

    #[test]
    fn test_dummy_provider_implements_trait() {
        let provider = DummyProvider;
        let target = Target {
            platform: Platform::MacOS,
            arch: Arch::Aarch64,
        };
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

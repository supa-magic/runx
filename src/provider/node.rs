use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::platform::{Arch, Platform, Target};
use crate::version::VersionSpec;

use super::{ArchiveFormat, Provider, ProviderError};

/// Node.js version entry from the official distribution index.
#[derive(Debug, Deserialize)]
struct NodeVersion {
    /// Version string (e.g., "v18.19.1").
    version: String,
    /// Whether this is an LTS release (false or string like "Hydrogen").
    /// Parsed for potential future LTS-only filtering.
    #[allow(unused)]
    lts: serde_json::Value,
}

/// Node.js tool provider.
///
/// Resolves versions from `https://nodejs.org/dist/index.json` and
/// constructs download URLs for the official Node.js binary distributions.
pub struct NodeProvider;

impl NodeProvider {
    /// The Node.js version index URL.
    const INDEX_URL: &str = "https://nodejs.org/dist/index.json";

    /// Fetch and parse the Node.js version index.
    ///
    /// Uses `tokio::task::spawn_blocking` internally because `reqwest::blocking`
    /// cannot run directly inside a tokio async context.
    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = tokio::task::block_in_place(|| {
            reqwest::blocking::get(Self::INDEX_URL)
                .map_err(|e| ProviderError::ResolutionFailed {
                    tool: "node".to_string(),
                    reason: format!("{e:#}"),
                })?
                .text()
                .map_err(|e| ProviderError::ResolutionFailed {
                    tool: "node".to_string(),
                    reason: format!("{e:#}"),
                })
        })?;

        Self::parse_versions(&body)
    }

    /// Parse the Node.js version index JSON into a list of stable semver versions.
    ///
    /// Filters out pre-release versions and strips the `v` prefix from version strings.
    fn parse_versions(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        let entries: Vec<NodeVersion> =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "node".to_string(),
                reason: format!("failed to parse version index: {e}"),
            })?;

        let mut versions = Vec::new();
        for entry in &entries {
            let ver_str = entry.version.strip_prefix('v').unwrap_or(&entry.version);
            if let Ok(v) = semver::Version::parse(ver_str)
                && v.pre.is_empty()
            {
                versions.push(v);
            }
        }

        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "node".to_string(),
                reason: "no stable versions found in index".to_string(),
            });
        }

        Ok(versions)
    }

    /// Construct the directory name inside the archive.
    ///
    /// Node.js archives contain a top-level directory like `node-v18.19.1-darwin-arm64/`.
    fn archive_dir_name(version: &semver::Version, target: &Target) -> String {
        let os = match target.platform {
            Platform::MacOS => "darwin",
            Platform::Linux => "linux",
            Platform::Windows => "win",
        };
        let arch = match target.arch {
            Arch::X86_64 => "x64",
            Arch::Aarch64 => "arm64",
        };
        format!("node-v{version}-{os}-{arch}")
    }
}

impl Provider for NodeProvider {
    fn name(&self) -> &str {
        "node"
    }

    fn resolve_version(
        &self,
        spec: &VersionSpec,
        _target: &Target,
    ) -> Result<semver::Version, ProviderError> {
        let candidates = Self::fetch_versions()?;
        spec.resolve(&candidates)
            .cloned()
            .ok_or_else(|| ProviderError::VersionNotFound {
                tool: "node".to_string(),
                spec: spec.to_string(),
            })
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let os = match target.platform {
            Platform::MacOS => "darwin",
            Platform::Linux => "linux",
            Platform::Windows => "win",
        };
        let arch = match target.arch {
            Arch::X86_64 => "x64",
            Arch::Aarch64 => "arm64",
        };
        let ext = match target.platform {
            Platform::Windows => "zip",
            _ => "tar.gz",
        };
        Ok(format!(
            "https://nodejs.org/dist/v{version}/node-v{version}-{os}-{arch}.{ext}"
        ))
    }

    fn archive_format(&self, target: &Target) -> ArchiveFormat {
        target.platform.default_archive_format()
    }

    fn bin_paths(&self, version: &semver::Version, target: &Target) -> Vec<PathBuf> {
        let dir_name = Self::archive_dir_name(version, target);
        // PATH expects directories, not individual files.
        // On Unix: node-v18.19.1-darwin-arm64/bin/
        // On Windows: node-v18.19.1-win-x64/ (node.exe, npm.cmd, npx.cmd are in the root)
        match target.platform {
            Platform::Windows => vec![PathBuf::from(&dir_name)],
            _ => vec![PathBuf::from(&dir_name).join("bin")],
        }
    }

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert(
            "NODE_HOME".to_string(),
            install_dir.to_string_lossy().to_string(),
        );
        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn macos_arm64() -> Target {
        Target::new(Platform::MacOS, Arch::Aarch64)
    }

    fn linux_x64() -> Target {
        Target::new(Platform::Linux, Arch::X86_64)
    }

    fn windows_x64() -> Target {
        Target::new(Platform::Windows, Arch::X86_64)
    }

    fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }

    // --- Provider trait ---

    #[test]
    fn test_name() {
        assert_eq!(NodeProvider.name(), "node");
    }

    // --- Download URL ---

    #[test]
    fn test_download_url_macos_arm64() {
        let url = NodeProvider
            .download_url(&v("18.19.1"), &macos_arm64())
            .unwrap();
        assert_eq!(
            url,
            "https://nodejs.org/dist/v18.19.1/node-v18.19.1-darwin-arm64.tar.gz"
        );
    }

    #[test]
    fn test_download_url_linux_x64() {
        let url = NodeProvider
            .download_url(&v("20.11.0"), &linux_x64())
            .unwrap();
        assert_eq!(
            url,
            "https://nodejs.org/dist/v20.11.0/node-v20.11.0-linux-x64.tar.gz"
        );
    }

    #[test]
    fn test_download_url_windows() {
        let url = NodeProvider
            .download_url(&v("18.19.1"), &windows_x64())
            .unwrap();
        assert_eq!(
            url,
            "https://nodejs.org/dist/v18.19.1/node-v18.19.1-win-x64.zip"
        );
    }

    // --- Archive format ---

    #[test]
    fn test_archive_format_unix() {
        assert_eq!(
            NodeProvider.archive_format(&macos_arm64()),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            NodeProvider.archive_format(&linux_x64()),
            ArchiveFormat::TarGz
        );
    }

    #[test]
    fn test_archive_format_windows() {
        assert_eq!(
            NodeProvider.archive_format(&windows_x64()),
            ArchiveFormat::Zip
        );
    }

    // --- Bin paths ---

    #[test]
    fn test_bin_paths_unix() {
        let paths = NodeProvider.bin_paths(&v("18.19.1"), &macos_arm64());
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("node-v18.19.1-darwin-arm64/bin"));
    }

    #[test]
    fn test_bin_paths_windows() {
        let paths = NodeProvider.bin_paths(&v("18.19.1"), &windows_x64());
        assert_eq!(paths.len(), 1);
        // Windows: directory root contains node.exe, npm.cmd, npx.cmd
        assert_eq!(paths[0], PathBuf::from("node-v18.19.1-win-x64"));
    }

    // --- Env vars ---

    #[test]
    fn test_env_vars() {
        let vars = NodeProvider.env_vars(Path::new("/cache/node/18.19.1"));
        assert_eq!(vars.get("NODE_HOME").unwrap(), "/cache/node/18.19.1");
        assert_eq!(vars.len(), 1);
    }

    // --- Archive dir name ---

    #[test]
    fn test_archive_dir_name() {
        assert_eq!(
            NodeProvider::archive_dir_name(&v("18.19.1"), &macos_arm64()),
            "node-v18.19.1-darwin-arm64"
        );
        assert_eq!(
            NodeProvider::archive_dir_name(&v("20.11.0"), &linux_x64()),
            "node-v20.11.0-linux-x64"
        );
        assert_eq!(
            NodeProvider::archive_dir_name(&v("18.19.1"), &windows_x64()),
            "node-v18.19.1-win-x64"
        );
    }

    // --- Linux arm64 target ---

    fn linux_arm64() -> Target {
        Target::new(Platform::Linux, Arch::Aarch64)
    }

    #[test]
    fn test_download_url_linux_arm64() {
        let url = NodeProvider
            .download_url(&v("20.11.0"), &linux_arm64())
            .unwrap();
        assert_eq!(
            url,
            "https://nodejs.org/dist/v20.11.0/node-v20.11.0-linux-arm64.tar.gz"
        );
    }

    #[test]
    fn test_bin_paths_linux_arm64() {
        let paths = NodeProvider.bin_paths(&v("20.11.0"), &linux_arm64());
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("node-v20.11.0-linux-arm64/bin"));
    }

    // --- parse_versions ---

    #[test]
    fn test_parse_versions_basic() {
        let json = r#"[
            {"version": "v20.11.0", "lts": "Iron"},
            {"version": "v18.19.1", "lts": "Hydrogen"},
            {"version": "v21.6.1", "lts": false}
        ]"#;
        let versions = NodeProvider::parse_versions(json).unwrap();
        assert_eq!(versions.len(), 3);
        assert!(versions.contains(&v("20.11.0")));
        assert!(versions.contains(&v("18.19.1")));
        assert!(versions.contains(&v("21.6.1")));
    }

    #[test]
    fn test_parse_versions_filters_prerelease() {
        let json = r#"[
            {"version": "v20.0.0", "lts": false},
            {"version": "v20.0.0-rc.1", "lts": false}
        ]"#;
        let versions = NodeProvider::parse_versions(json).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], v("20.0.0"));
    }

    #[test]
    fn test_parse_versions_empty_returns_error() {
        let json = r#"[]"#;
        let result = NodeProvider::parse_versions(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_versions_invalid_json_returns_error() {
        let result = NodeProvider::parse_versions("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_versions_strips_v_prefix() {
        let json = r#"[{"version": "v18.0.0", "lts": false}]"#;
        let versions = NodeProvider::parse_versions(json).unwrap();
        assert_eq!(versions[0], v("18.0.0"));
    }

    #[test]
    fn test_parse_versions_skips_unparseable() {
        let json = r#"[
            {"version": "v18.0.0", "lts": false},
            {"version": "not-a-version", "lts": false}
        ]"#;
        let versions = NodeProvider::parse_versions(json).unwrap();
        assert_eq!(versions.len(), 1);
    }

    // --- get_provider ---

    #[test]
    fn test_get_provider_node() {
        let provider = super::super::get_provider("node").unwrap();
        assert_eq!(provider.name(), "node");
    }

    #[test]
    fn test_get_provider_nodejs_alias() {
        let provider = super::super::get_provider("nodejs").unwrap();
        assert_eq!(provider.name(), "node");
    }

    #[test]
    fn test_get_provider_unknown() {
        let result = super::super::get_provider("unknown");
        assert!(result.is_err());
    }
}

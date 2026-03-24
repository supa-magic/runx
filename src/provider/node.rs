use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::platform::{Arch, Platform, Target};
use crate::version::VersionSpec;

use super::{
    ArchiveFormat, Provider, ProviderError, collect_stable_versions, fetch_json,
    resolve_from_candidates,
};

/// Node.js version entry from the official distribution index.
#[derive(Debug, Deserialize)]
struct NodeVersion {
    version: String,
    #[allow(unused)]
    lts: serde_json::Value,
}

/// Node.js tool provider.
///
/// Resolves versions from `https://nodejs.org/dist/index.json` and
/// constructs download URLs for the official Node.js binary distributions.
pub struct NodeProvider;

impl NodeProvider {
    const INDEX_URL: &str = "https://nodejs.org/dist/index.json";

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = fetch_json(Self::INDEX_URL, "node")?;
        Self::parse_versions(&body)
    }

    /// Parse the Node.js version index JSON into a list of stable semver versions.
    fn parse_versions(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        let entries: Vec<NodeVersion> =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "node".to_string(),
                reason: format!("failed to parse version index: {e}"),
            })?;

        let versions = collect_stable_versions(entries.iter().map(|entry| {
            let ver_str = entry.version.strip_prefix('v').unwrap_or(&entry.version);
            semver::Version::parse(ver_str).ok()
        }));

        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "node".to_string(),
                reason: "no stable versions found in index".to_string(),
            });
        }

        Ok(versions)
    }

    /// Construct the directory name inside the archive.
    fn archive_dir_name(version: &semver::Version, target: &Target) -> String {
        let os = target.platform.as_download_str();
        let arch = target.arch.as_download_str();
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
        resolve_from_candidates(&candidates, spec, "node")
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let os = target.platform.as_download_str();
        let arch = target.arch.as_download_str();
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
        match target.platform {
            Platform::Windows => vec![PathBuf::from(&dir_name)],
            _ => vec![PathBuf::from(&dir_name).join("bin")],
        }
    }

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        HashMap::from([(
            "NODE_HOME".to_string(),
            install_dir.to_string_lossy().to_string(),
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(NodeProvider.name(), "node");
    }

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
    fn test_download_url_windows() {
        let url = NodeProvider
            .download_url(&v("18.19.1"), &windows_x64())
            .unwrap();
        assert_eq!(
            url,
            "https://nodejs.org/dist/v18.19.1/node-v18.19.1-win-x64.zip"
        );
    }

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

    #[test]
    fn test_bin_paths_unix() {
        let paths = NodeProvider.bin_paths(&v("18.19.1"), &macos_arm64());
        assert_eq!(paths, vec![PathBuf::from("node-v18.19.1-darwin-arm64/bin")]);
    }

    #[test]
    fn test_bin_paths_windows() {
        let paths = NodeProvider.bin_paths(&v("18.19.1"), &windows_x64());
        assert_eq!(paths, vec![PathBuf::from("node-v18.19.1-win-x64")]);
    }

    #[test]
    fn test_bin_paths_linux_arm64() {
        let paths = NodeProvider.bin_paths(&v("20.11.0"), &linux_arm64());
        assert_eq!(paths, vec![PathBuf::from("node-v20.11.0-linux-arm64/bin")]);
    }

    #[test]
    fn test_env_vars() {
        let vars = NodeProvider.env_vars(Path::new("/cache/node/18.19.1"));
        assert_eq!(vars.get("NODE_HOME").unwrap(), "/cache/node/18.19.1");
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_archive_dir_name() {
        assert_eq!(
            NodeProvider::archive_dir_name(&v("18.19.1"), &macos_arm64()),
            "node-v18.19.1-darwin-arm64"
        );
        assert_eq!(
            NodeProvider::archive_dir_name(&v("18.19.1"), &windows_x64()),
            "node-v18.19.1-win-x64"
        );
    }

    #[test]
    fn test_parse_versions_basic() {
        let json = r#"[
            {"version": "v20.11.0", "lts": "Iron"},
            {"version": "v18.19.1", "lts": "Hydrogen"},
            {"version": "v21.6.1", "lts": false}
        ]"#;
        let versions = NodeProvider::parse_versions(json).unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn test_parse_versions_filters_prerelease() {
        let json = r#"[
            {"version": "v20.0.0", "lts": false},
            {"version": "v20.0.0-rc.1", "lts": false}
        ]"#;
        let versions = NodeProvider::parse_versions(json).unwrap();
        assert_eq!(versions.len(), 1);
    }

    #[test]
    fn test_parse_versions_empty_returns_error() {
        assert!(NodeProvider::parse_versions("[]").is_err());
    }

    #[test]
    fn test_parse_versions_invalid_json() {
        assert!(NodeProvider::parse_versions("not json").is_err());
    }

    #[test]
    fn test_get_provider_node() {
        assert_eq!(super::super::get_provider("node").unwrap().name(), "node");
    }

    #[test]
    fn test_get_provider_nodejs_alias() {
        assert_eq!(super::super::get_provider("nodejs").unwrap().name(), "node");
    }
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::platform::{Arch, Platform, Target};
use crate::version::VersionSpec;

use super::{
    ArchiveFormat, Provider, ProviderError, collect_stable_versions, fetch_json,
    resolve_from_candidates,
};

/// Go version entry from the official download API.
#[derive(Debug, Deserialize)]
struct GoVersion {
    version: String,
    stable: bool,
    #[allow(unused)]
    files: Vec<GoFile>,
}

/// Go download file entry (parsed for serde completeness).
#[derive(Debug, Deserialize)]
#[allow(unused)]
struct GoFile {
    filename: String,
    os: String,
    arch: String,
    kind: String,
}

/// Go tool provider.
///
/// Resolves versions from `https://go.dev/dl/?mode=json` and constructs
/// download URLs for the official Go binary distributions.
pub struct GoProvider;

impl GoProvider {
    const INDEX_URL: &str = "https://go.dev/dl/?mode=json";

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = fetch_json(Self::INDEX_URL, "go")?;
        Self::parse_versions(&body)
    }

    /// Parse the Go version index JSON into a list of stable semver versions.
    fn parse_versions(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        let releases: Vec<GoVersion> =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "go".to_string(),
                reason: format!("failed to parse version index: {e}"),
            })?;

        let versions = collect_stable_versions(
            releases
                .iter()
                .filter(|r| r.stable)
                .map(|r| Self::parse_go_version(&r.version)),
        );

        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "go".to_string(),
                reason: "no stable versions found in index".to_string(),
            });
        }

        Ok(versions)
    }

    /// Parse a Go version string like "go1.21.6" into a semver Version.
    ///
    /// Go versions omit the patch sometimes: `go1.21` means `1.21.0`.
    fn parse_go_version(s: &str) -> Option<semver::Version> {
        let ver_str = s.strip_prefix("go")?;

        if let Ok(v) = semver::Version::parse(ver_str) {
            return Some(v);
        }

        // Handle missing patch: "1.21" → "1.21.0"
        if let Some((major_str, minor_str)) = ver_str.split_once('.')
            && !minor_str.contains('.')
        {
            let major = major_str.parse::<u64>().ok()?;
            let minor = minor_str.parse::<u64>().ok()?;
            return Some(semver::Version::new(major, minor, 0));
        }

        None
    }

    /// Map platform to Go's naming convention.
    fn go_os(platform: Platform) -> &'static str {
        match platform {
            Platform::MacOS => "darwin",
            Platform::Linux => "linux",
            Platform::Windows => "windows",
        }
    }

    /// Map architecture to Go's naming convention (amd64, not x64).
    fn go_arch(arch: Arch) -> &'static str {
        match arch {
            Arch::X86_64 => "amd64",
            Arch::Aarch64 => "arm64",
        }
    }
}

impl Provider for GoProvider {
    fn name(&self) -> &str {
        "go"
    }

    fn resolve_version(
        &self,
        spec: &VersionSpec,
        _target: &Target,
    ) -> Result<semver::Version, ProviderError> {
        let candidates = Self::fetch_versions()?;
        resolve_from_candidates(&candidates, spec, "go")
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let os = Self::go_os(target.platform);
        let arch = Self::go_arch(target.arch);
        let ext = match target.platform {
            Platform::Windows => "zip",
            _ => "tar.gz",
        };
        Ok(format!("https://go.dev/dl/go{version}.{os}-{arch}.{ext}"))
    }

    fn archive_format(&self, target: &Target) -> ArchiveFormat {
        target.platform.default_archive_format()
    }

    fn bin_paths(&self, _version: &semver::Version, _target: &Target) -> Vec<PathBuf> {
        vec![PathBuf::from("go").join("bin")]
    }

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        HashMap::from([(
            "GOROOT".to_string(),
            install_dir.join("go").to_string_lossy().to_string(),
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(GoProvider.name(), "go");
    }

    #[test]
    fn test_download_url_macos_arm64() {
        let url = GoProvider
            .download_url(&v("1.21.6"), &macos_arm64())
            .unwrap();
        assert_eq!(url, "https://go.dev/dl/go1.21.6.darwin-arm64.tar.gz");
    }

    #[test]
    fn test_download_url_linux_x64() {
        let url = GoProvider.download_url(&v("1.22.0"), &linux_x64()).unwrap();
        assert_eq!(url, "https://go.dev/dl/go1.22.0.linux-amd64.tar.gz");
    }

    #[test]
    fn test_download_url_linux_arm64() {
        let url = GoProvider
            .download_url(&v("1.21.6"), &linux_arm64())
            .unwrap();
        assert_eq!(url, "https://go.dev/dl/go1.21.6.linux-arm64.tar.gz");
    }

    #[test]
    fn test_download_url_windows() {
        let url = GoProvider
            .download_url(&v("1.21.6"), &windows_x64())
            .unwrap();
        assert_eq!(url, "https://go.dev/dl/go1.21.6.windows-amd64.zip");
    }

    #[test]
    fn test_archive_format_unix() {
        assert_eq!(
            GoProvider.archive_format(&macos_arm64()),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            GoProvider.archive_format(&linux_x64()),
            ArchiveFormat::TarGz
        );
    }

    #[test]
    fn test_archive_format_windows() {
        assert_eq!(
            GoProvider.archive_format(&windows_x64()),
            ArchiveFormat::Zip
        );
    }

    #[test]
    fn test_bin_paths() {
        let paths = GoProvider.bin_paths(&v("1.21.6"), &macos_arm64());
        assert_eq!(paths, vec![PathBuf::from("go/bin")]);
    }

    #[test]
    fn test_env_vars() {
        let vars = GoProvider.env_vars(Path::new("/cache/go/1.21.6"));
        assert_eq!(vars.get("GOROOT").unwrap(), "/cache/go/1.21.6/go");
    }

    #[test]
    fn test_parse_go_version_full() {
        assert_eq!(GoProvider::parse_go_version("go1.21.6"), Some(v("1.21.6")));
    }

    #[test]
    fn test_parse_go_version_no_patch() {
        assert_eq!(GoProvider::parse_go_version("go1.21"), Some(v("1.21.0")));
    }

    #[test]
    fn test_parse_go_version_no_prefix() {
        assert!(GoProvider::parse_go_version("1.21.6").is_none());
    }

    #[test]
    fn test_parse_go_version_invalid() {
        assert!(GoProvider::parse_go_version("gonotaversion").is_none());
    }

    #[test]
    fn test_parse_versions_basic() {
        let json = r#"[
            {"version": "go1.22.0", "stable": true, "files": []},
            {"version": "go1.21.6", "stable": true, "files": []}
        ]"#;
        let versions = GoProvider::parse_versions(json).unwrap();
        assert_eq!(versions.len(), 2);
        assert!(versions.contains(&v("1.22.0")));
        assert!(versions.contains(&v("1.21.6")));
    }

    #[test]
    fn test_parse_versions_filters_unstable() {
        let json = r#"[
            {"version": "go1.22.0", "stable": true, "files": []},
            {"version": "go1.23rc1", "stable": false, "files": []}
        ]"#;
        let versions = GoProvider::parse_versions(json).unwrap();
        assert_eq!(versions.len(), 1);
    }

    #[test]
    fn test_parse_versions_deduplicates() {
        let json = r#"[
            {"version": "go1.21.6", "stable": true, "files": []},
            {"version": "go1.21.6", "stable": true, "files": []}
        ]"#;
        assert_eq!(GoProvider::parse_versions(json).unwrap().len(), 1);
    }

    #[test]
    fn test_parse_versions_empty_returns_error() {
        assert!(GoProvider::parse_versions("[]").is_err());
    }

    #[test]
    fn test_parse_versions_invalid_json() {
        assert!(GoProvider::parse_versions("not json").is_err());
    }

    #[test]
    fn test_go_os() {
        assert_eq!(GoProvider::go_os(Platform::MacOS), "darwin");
        assert_eq!(GoProvider::go_os(Platform::Linux), "linux");
        assert_eq!(GoProvider::go_os(Platform::Windows), "windows");
    }

    #[test]
    fn test_go_arch() {
        assert_eq!(GoProvider::go_arch(Arch::X86_64), "amd64");
        assert_eq!(GoProvider::go_arch(Arch::Aarch64), "arm64");
    }

    #[test]
    fn test_get_provider_go() {
        assert_eq!(super::super::get_provider("go").unwrap().name(), "go");
    }

    #[test]
    fn test_get_provider_golang_alias() {
        assert_eq!(super::super::get_provider("golang").unwrap().name(), "go");
    }
}

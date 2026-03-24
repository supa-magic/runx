use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::platform::{Arch, Platform, Target};
use crate::version::VersionSpec;

use super::{ArchiveFormat, Provider, ProviderError};

/// GitHub release entry from the python-build-standalone repository.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

/// GitHub release asset.
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Python tool provider using python-build-standalone releases.
///
/// Resolves versions from `https://api.github.com/repos/indygreg/python-build-standalone/releases`
/// and constructs download URLs for portable, pre-built Python binaries.
pub struct PythonProvider;

impl PythonProvider {
    /// GitHub API URL for python-build-standalone releases.
    const RELEASES_URL: &str =
        "https://api.github.com/repos/indygreg/python-build-standalone/releases";

    /// Fetch releases from GitHub and extract available Python versions.
    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = tokio::task::block_in_place(|| {
            reqwest::blocking::Client::new()
                .get(Self::RELEASES_URL)
                .query(&[("per_page", "30")])
                .header("User-Agent", "runx")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
                .map_err(|e| ProviderError::ResolutionFailed {
                    tool: "python".to_string(),
                    reason: format!("{e:#}"),
                })?
                .text()
                .map_err(|e| ProviderError::ResolutionFailed {
                    tool: "python".to_string(),
                    reason: format!("{e:#}"),
                })
        })?;

        Self::parse_releases(&body)
    }

    /// Parse GitHub releases JSON into a list of available Python versions.
    ///
    /// Extracts version numbers from release tag names (e.g., "20240224" tags
    /// contain assets like "cpython-3.11.8+20240224-...").
    fn parse_releases(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        let releases: Vec<GitHubRelease> =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "python".to_string(),
                reason: format!("failed to parse releases: {e}"),
            })?;

        let mut versions = Vec::new();
        for release in &releases {
            for asset in &release.assets {
                if let Some(ver) = Self::extract_version_from_asset(&asset.name) {
                    if ver.pre.is_empty() && !versions.contains(&ver) {
                        versions.push(ver);
                    }
                }
            }
        }

        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "python".to_string(),
                reason: "no Python versions found in releases".to_string(),
            });
        }

        Ok(versions)
    }

    /// Extract a Python version from an asset filename.
    ///
    /// Asset names look like: `cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz`
    fn extract_version_from_asset(name: &str) -> Option<semver::Version> {
        // Must start with "cpython-" and contain "install_only"
        if !name.starts_with("cpython-") || !name.contains("install_only") {
            return None;
        }

        // Extract version: "cpython-3.11.8+20240224-..." → "3.11.8"
        let after_prefix = name.strip_prefix("cpython-")?;
        let version_str = after_prefix.split('+').next()?;
        semver::Version::parse(version_str).ok()
    }

    /// Construct the asset name pattern for a given version and target.
    fn asset_pattern(version: &semver::Version, target: &Target) -> String {
        let arch = match target.arch {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        };
        let os_triple = match target.platform {
            Platform::MacOS => "apple-darwin",
            Platform::Linux => "unknown-linux-gnu",
            Platform::Windows => "pc-windows-msvc-shared",
        };
        format!("cpython-{version}+*-{arch}-{os_triple}-install_only")
    }

    /// Find the download URL for a specific version and target from releases.
    fn find_download_url(
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let body = tokio::task::block_in_place(|| {
            reqwest::blocking::Client::new()
                .get(Self::RELEASES_URL)
                .query(&[("per_page", "30")])
                .header("User-Agent", "runx")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
                .map_err(|e| ProviderError::ResolutionFailed {
                    tool: "python".to_string(),
                    reason: format!("{e:#}"),
                })?
                .text()
                .map_err(|e| ProviderError::ResolutionFailed {
                    tool: "python".to_string(),
                    reason: format!("{e:#}"),
                })
        })?;

        let releases: Vec<GitHubRelease> =
            serde_json::from_str(&body).map_err(|e| ProviderError::ResolutionFailed {
                tool: "python".to_string(),
                reason: format!("failed to parse releases: {e}"),
            })?;

        Self::find_url_in_releases(&releases, version, target)
    }

    /// Search parsed releases for a matching asset URL.
    ///
    /// Matches assets by version prefix and platform/arch suffix,
    /// filtering to `install_only` tar.gz archives (not .sha256 checksums).
    fn find_url_in_releases(
        releases: &[GitHubRelease],
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let arch = match target.arch {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        };
        let os_triple = match target.platform {
            Platform::MacOS => "apple-darwin",
            Platform::Linux => "unknown-linux-gnu",
            Platform::Windows => "pc-windows-msvc-shared",
        };
        let version_prefix = format!("cpython-{version}+");
        let suffix = format!("-{arch}-{os_triple}-install_only.tar.gz");

        for release in releases {
            for asset in &release.assets {
                if asset.name.starts_with(&version_prefix) && asset.name.ends_with(&suffix) {
                    return Ok(asset.browser_download_url.clone());
                }
            }
        }

        Err(ProviderError::VersionNotFound {
            tool: "python".to_string(),
            spec: format!("{version} for {target}"),
        })
    }
}

impl Provider for PythonProvider {
    fn name(&self) -> &str {
        "python"
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
                tool: "python".to_string(),
                spec: spec.to_string(),
            })
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        Self::find_download_url(version, target)
    }

    fn archive_format(&self, _target: &Target) -> ArchiveFormat {
        // python-build-standalone always uses tar.gz (even on Windows)
        ArchiveFormat::TarGz
    }

    fn bin_paths(&self, _version: &semver::Version, target: &Target) -> Vec<PathBuf> {
        // python-build-standalone extracts to a "python/" directory with
        // bin/ (Unix) or Scripts/ + python.exe (Windows) inside
        match target.platform {
            Platform::Windows => vec![
                PathBuf::from("python"),
                PathBuf::from("python").join("Scripts"),
            ],
            _ => vec![PathBuf::from("python").join("bin")],
        }
    }

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert(
            "PYTHONHOME".to_string(),
            install_dir.join("python").to_string_lossy().to_string(),
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

    fn linux_arm64() -> Target {
        Target::new(Platform::Linux, Arch::Aarch64)
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
        assert_eq!(PythonProvider.name(), "python");
    }

    // --- Archive format ---

    #[test]
    fn test_archive_format_always_tar_gz() {
        assert_eq!(
            PythonProvider.archive_format(&macos_arm64()),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            PythonProvider.archive_format(&linux_x64()),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            PythonProvider.archive_format(&windows_x64()),
            ArchiveFormat::TarGz
        );
    }

    // --- Bin paths ---

    #[test]
    fn test_bin_paths_unix() {
        let paths = PythonProvider.bin_paths(&v("3.11.8"), &macos_arm64());
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("python/bin"));
    }

    #[test]
    fn test_bin_paths_linux() {
        let paths = PythonProvider.bin_paths(&v("3.11.8"), &linux_x64());
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("python/bin"));
    }

    #[test]
    fn test_bin_paths_windows() {
        let paths = PythonProvider.bin_paths(&v("3.11.8"), &windows_x64());
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("python"));
        assert_eq!(paths[1], PathBuf::from("python/Scripts"));
    }

    // --- Env vars ---

    #[test]
    fn test_env_vars() {
        let vars = PythonProvider.env_vars(Path::new("/cache/python/3.11.8"));
        assert_eq!(
            vars.get("PYTHONHOME").unwrap(),
            "/cache/python/3.11.8/python"
        );
        assert_eq!(vars.len(), 1);
    }

    // --- Asset pattern ---

    #[test]
    fn test_asset_pattern_macos_arm64() {
        let pattern = PythonProvider::asset_pattern(&v("3.11.8"), &macos_arm64());
        assert!(pattern.contains("cpython-3.11.8+"));
        assert!(pattern.contains("aarch64-apple-darwin"));
        assert!(pattern.contains("install_only"));
    }

    #[test]
    fn test_asset_pattern_linux_x64() {
        let pattern = PythonProvider::asset_pattern(&v("3.12.1"), &linux_x64());
        assert!(pattern.contains("cpython-3.12.1+"));
        assert!(pattern.contains("x86_64-unknown-linux-gnu"));
        assert!(pattern.contains("install_only"));
    }

    #[test]
    fn test_asset_pattern_windows() {
        let pattern = PythonProvider::asset_pattern(&v("3.11.8"), &windows_x64());
        assert!(pattern.contains("x86_64-pc-windows-msvc-shared"));
    }

    // --- extract_version_from_asset ---

    #[test]
    fn test_extract_version_from_valid_asset() {
        let name = "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz";
        let ver = PythonProvider::extract_version_from_asset(name).unwrap();
        assert_eq!(ver, v("3.11.8"));
    }

    #[test]
    fn test_extract_version_from_linux_asset() {
        let name = "cpython-3.12.1+20240107-x86_64-unknown-linux-gnu-install_only.tar.gz";
        let ver = PythonProvider::extract_version_from_asset(name).unwrap();
        assert_eq!(ver, v("3.12.1"));
    }

    #[test]
    fn test_extract_version_ignores_non_install_only() {
        let name = "cpython-3.11.8+20240224-aarch64-apple-darwin-debug.tar.gz";
        assert!(PythonProvider::extract_version_from_asset(name).is_none());
    }

    #[test]
    fn test_extract_version_ignores_non_cpython() {
        let name = "SHA256SUMS";
        assert!(PythonProvider::extract_version_from_asset(name).is_none());
    }

    #[test]
    fn test_extract_version_ignores_checksum_files() {
        let name = "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz.sha256";
        // This actually starts with cpython- and contains install_only, but
        // the version extraction should still work — it's the URL matching
        // in find_download_url that filters by suffix.
        let ver = PythonProvider::extract_version_from_asset(name);
        assert!(ver.is_some()); // Version is extractable, filtering happens elsewhere
    }

    // --- parse_releases ---

    #[test]
    fn test_parse_releases_basic() {
        let json = r#"[{
            "tag_name": "20240224",
            "assets": [
                {
                    "name": "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz",
                    "browser_download_url": "https://example.com/cpython-3.11.8.tar.gz"
                },
                {
                    "name": "cpython-3.12.2+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz",
                    "browser_download_url": "https://example.com/cpython-3.12.2.tar.gz"
                },
                {
                    "name": "SHA256SUMS",
                    "browser_download_url": "https://example.com/SHA256SUMS"
                }
            ]
        }]"#;

        let versions = PythonProvider::parse_releases(json).unwrap();
        assert!(versions.contains(&v("3.11.8")));
        assert!(versions.contains(&v("3.12.2")));
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_parse_releases_deduplicates() {
        let json = r#"[{
            "tag_name": "20240224",
            "assets": [
                {
                    "name": "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz",
                    "browser_download_url": "https://example.com/1.tar.gz"
                },
                {
                    "name": "cpython-3.11.8+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz",
                    "browser_download_url": "https://example.com/2.tar.gz"
                }
            ]
        }]"#;

        let versions = PythonProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], v("3.11.8"));
    }

    #[test]
    fn test_parse_releases_empty_returns_error() {
        let json = r#"[]"#;
        assert!(PythonProvider::parse_releases(json).is_err());
    }

    #[test]
    fn test_parse_releases_invalid_json_returns_error() {
        assert!(PythonProvider::parse_releases("not json").is_err());
    }

    // --- parse_releases: prerelease filtering ---

    #[test]
    fn test_parse_releases_excludes_prerelease() {
        let json = r#"[{
            "tag_name": "20240224",
            "assets": [
                {
                    "name": "cpython-3.13.0a1+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz",
                    "browser_download_url": "https://example.com/alpha.tar.gz"
                },
                {
                    "name": "cpython-3.12.2+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz",
                    "browser_download_url": "https://example.com/stable.tar.gz"
                }
            ]
        }]"#;

        let versions = PythonProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], v("3.12.2"));
    }

    #[test]
    fn test_parse_releases_multiple_releases() {
        let json = r#"[
            {
                "tag_name": "20240224",
                "assets": [{
                    "name": "cpython-3.12.2+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz",
                    "browser_download_url": "https://example.com/3.12.2.tar.gz"
                }]
            },
            {
                "tag_name": "20240107",
                "assets": [{
                    "name": "cpython-3.11.7+20240107-x86_64-unknown-linux-gnu-install_only.tar.gz",
                    "browser_download_url": "https://example.com/3.11.7.tar.gz"
                }]
            }
        ]"#;

        let versions = PythonProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 2);
        assert!(versions.contains(&v("3.12.2")));
        assert!(versions.contains(&v("3.11.7")));
    }

    // --- extract_version_from_asset: edge cases ---

    #[test]
    fn test_extract_version_malformed_version() {
        let name = "cpython-notaversion+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz";
        assert!(PythonProvider::extract_version_from_asset(name).is_none());
    }

    // --- find_url_in_releases ---

    #[test]
    fn test_find_url_in_releases_macos_arm64() {
        let releases = vec![GitHubRelease {
            tag_name: "20240224".to_string(),
            assets: vec![
                GitHubAsset {
                    name: "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz"
                        .to_string(),
                    browser_download_url: "https://example.com/macos.tar.gz".to_string(),
                },
                GitHubAsset {
                    name: "cpython-3.11.8+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz"
                        .to_string(),
                    browser_download_url: "https://example.com/linux.tar.gz".to_string(),
                },
            ],
        }];

        let url =
            PythonProvider::find_url_in_releases(&releases, &v("3.11.8"), &macos_arm64()).unwrap();
        assert_eq!(url, "https://example.com/macos.tar.gz");
    }

    #[test]
    fn test_find_url_in_releases_linux_x64() {
        let releases = vec![GitHubRelease {
            tag_name: "20240224".to_string(),
            assets: vec![GitHubAsset {
                name: "cpython-3.12.1+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz"
                    .to_string(),
                browser_download_url: "https://example.com/linux.tar.gz".to_string(),
            }],
        }];

        let url =
            PythonProvider::find_url_in_releases(&releases, &v("3.12.1"), &linux_x64()).unwrap();
        assert_eq!(url, "https://example.com/linux.tar.gz");
    }

    #[test]
    fn test_find_url_in_releases_linux_arm64() {
        let releases = vec![GitHubRelease {
            tag_name: "20240224".to_string(),
            assets: vec![GitHubAsset {
                name: "cpython-3.11.8+20240224-aarch64-unknown-linux-gnu-install_only.tar.gz"
                    .to_string(),
                browser_download_url: "https://example.com/linux-arm64.tar.gz".to_string(),
            }],
        }];

        let url =
            PythonProvider::find_url_in_releases(&releases, &v("3.11.8"), &linux_arm64()).unwrap();
        assert_eq!(url, "https://example.com/linux-arm64.tar.gz");
    }

    #[test]
    fn test_find_url_in_releases_windows() {
        let releases = vec![GitHubRelease {
            tag_name: "20240224".to_string(),
            assets: vec![GitHubAsset {
                name: "cpython-3.11.8+20240224-x86_64-pc-windows-msvc-shared-install_only.tar.gz"
                    .to_string(),
                browser_download_url: "https://example.com/windows.tar.gz".to_string(),
            }],
        }];

        let url =
            PythonProvider::find_url_in_releases(&releases, &v("3.11.8"), &windows_x64()).unwrap();
        assert_eq!(url, "https://example.com/windows.tar.gz");
    }

    #[test]
    fn test_find_url_in_releases_not_found() {
        let releases = vec![GitHubRelease {
            tag_name: "20240224".to_string(),
            assets: vec![GitHubAsset {
                name: "cpython-3.11.8+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz"
                    .to_string(),
                browser_download_url: "https://example.com/linux.tar.gz".to_string(),
            }],
        }];

        let result = PythonProvider::find_url_in_releases(&releases, &v("3.99.0"), &linux_x64());
        assert!(result.is_err());
    }

    #[test]
    fn test_find_url_in_releases_ignores_sha256() {
        let releases = vec![GitHubRelease {
            tag_name: "20240224".to_string(),
            assets: vec![
                GitHubAsset {
                    name: "cpython-3.11.8+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz.sha256".to_string(),
                    browser_download_url: "https://example.com/checksum".to_string(),
                },
                GitHubAsset {
                    name: "cpython-3.11.8+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz".to_string(),
                    browser_download_url: "https://example.com/real.tar.gz".to_string(),
                },
            ],
        }];

        let url =
            PythonProvider::find_url_in_releases(&releases, &v("3.11.8"), &linux_x64()).unwrap();
        assert_eq!(url, "https://example.com/real.tar.gz");
    }

    // --- get_provider ---

    #[test]
    fn test_get_provider_python() {
        let provider = super::super::get_provider("python").unwrap();
        assert_eq!(provider.name(), "python");
    }

    #[test]
    fn test_get_provider_python3_alias() {
        let provider = super::super::get_provider("python3").unwrap();
        assert_eq!(provider.name(), "python");
    }
}

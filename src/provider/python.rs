use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;

use crate::platform::{Arch, Platform, Target};
use crate::version::VersionSpec;

use super::{
    ArchiveFormat, Provider, ProviderError, collect_stable_versions, fetch_json,
    resolve_from_candidates,
};

/// GitHub release entry from the python-build-standalone repository.
#[derive(Debug, Clone, Deserialize)]
struct GitHubRelease {
    #[allow(unused)]
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

/// GitHub release asset.
#[derive(Debug, Clone, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Cached releases to avoid duplicate HTTP requests within a single invocation.
/// `resolve_version` and `download_url` both need the releases data.
/// Cached releases to avoid duplicate HTTP requests.
static CACHED_RELEASES: Mutex<Option<Vec<GitHubRelease>>> = Mutex::new(None);

/// Python tool provider using python-build-standalone releases.
///
/// Caches the GitHub releases response to avoid redundant HTTP requests
/// when `resolve_version` and `download_url` are called in sequence.
pub struct PythonProvider;

impl PythonProvider {
    const RELEASES_URL: &str =
        "https://api.github.com/repos/astral-sh/python-build-standalone/releases?per_page=30";

    /// Get releases, fetching from GitHub only on the first call.
    /// Caches the result in a static Mutex to avoid duplicate HTTP requests
    /// when resolve_version and download_url are called in sequence.
    fn get_releases() -> Result<Vec<GitHubRelease>, ProviderError> {
        let mut cache = CACHED_RELEASES.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(releases) = cache.as_ref() {
            return Ok(releases.clone());
        }
        let body = fetch_json(Self::RELEASES_URL, "python")?;
        let releases: Vec<GitHubRelease> =
            serde_json::from_str(&body).map_err(|e| ProviderError::ResolutionFailed {
                tool: "python".to_string(),
                reason: format!("failed to parse releases: {e}"),
            })?;
        *cache = Some(releases.clone());
        Ok(releases)
    }

    /// Extract available Python versions from parsed releases (for testing).
    #[cfg(test)]
    fn parse_releases(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        let releases: Vec<GitHubRelease> =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "python".to_string(),
                reason: format!("failed to parse releases: {e}"),
            })?;

        let versions = Self::versions_from_releases(&releases);

        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "python".to_string(),
                reason: "no Python versions found in releases".to_string(),
            });
        }

        Ok(versions)
    }

    /// Extract unique stable versions from a list of releases.
    fn versions_from_releases(releases: &[GitHubRelease]) -> Vec<semver::Version> {
        collect_stable_versions(
            releases
                .iter()
                .flat_map(|r| r.assets.iter())
                .map(|asset| Self::extract_version_from_asset(&asset.name)),
        )
    }

    /// Extract a Python version from an asset filename.
    fn extract_version_from_asset(name: &str) -> Option<semver::Version> {
        if !name.starts_with("cpython-")
            || !name.contains("install_only")
            || name.ends_with(".sha256")
        {
            return None;
        }
        let after_prefix = name.strip_prefix("cpython-")?;
        let version_str = after_prefix.split('+').next()?;
        semver::Version::parse(version_str).ok()
    }

    /// Search parsed releases for a matching asset URL.
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
        let releases = Self::get_releases()?;
        let versions = Self::versions_from_releases(&releases);
        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "python".to_string(),
                reason: "no Python versions found".to_string(),
            });
        }
        resolve_from_candidates(&versions, spec, "python")
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let releases = Self::get_releases()?;
        Self::find_url_in_releases(&releases, version, target)
    }

    fn archive_format(&self, _target: &Target) -> ArchiveFormat {
        ArchiveFormat::TarGz
    }

    fn bin_paths(&self, _version: &semver::Version, target: &Target) -> Vec<PathBuf> {
        match target.platform {
            Platform::Windows => vec![
                PathBuf::from("python"),
                PathBuf::from("python").join("Scripts"),
            ],
            _ => vec![PathBuf::from("python").join("bin")],
        }
    }

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        HashMap::from([(
            "PYTHONHOME".to_string(),
            install_dir.join("python").to_string_lossy().to_string(),
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(PythonProvider.name(), "python");
    }

    #[test]
    fn test_archive_format_always_tar_gz() {
        assert_eq!(
            PythonProvider.archive_format(&macos_arm64()),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            PythonProvider.archive_format(&windows_x64()),
            ArchiveFormat::TarGz
        );
    }

    #[test]
    fn test_bin_paths_unix() {
        let paths = PythonProvider.bin_paths(&v("3.11.8"), &macos_arm64());
        assert_eq!(paths, vec![PathBuf::from("python/bin")]);
    }

    #[test]
    fn test_bin_paths_windows() {
        let paths = PythonProvider.bin_paths(&v("3.11.8"), &windows_x64());
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_env_vars() {
        let vars = PythonProvider.env_vars(Path::new("/cache/python/3.11.8"));
        assert_eq!(
            vars.get("PYTHONHOME").unwrap(),
            "/cache/python/3.11.8/python"
        );
    }

    #[test]
    fn test_extract_version_valid() {
        let name = "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz";
        assert_eq!(
            PythonProvider::extract_version_from_asset(name),
            Some(v("3.11.8"))
        );
    }

    #[test]
    fn test_extract_version_ignores_non_install_only() {
        let name = "cpython-3.11.8+20240224-aarch64-apple-darwin-debug.tar.gz";
        assert!(PythonProvider::extract_version_from_asset(name).is_none());
    }

    #[test]
    fn test_extract_version_ignores_sha256() {
        let name = "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz.sha256";
        assert!(PythonProvider::extract_version_from_asset(name).is_none());
    }

    #[test]
    fn test_extract_version_malformed() {
        let name = "cpython-notaversion+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz";
        assert!(PythonProvider::extract_version_from_asset(name).is_none());
    }

    #[test]
    fn test_parse_releases_basic() {
        let json = r#"[{
            "tag_name": "20240224",
            "assets": [
                {"name": "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz", "browser_download_url": "https://example.com/1.tar.gz"},
                {"name": "cpython-3.12.2+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz", "browser_download_url": "https://example.com/2.tar.gz"},
                {"name": "SHA256SUMS", "browser_download_url": "https://example.com/SHA256SUMS"}
            ]
        }]"#;
        let versions = PythonProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_parse_releases_deduplicates() {
        let json = r#"[{
            "tag_name": "20240224",
            "assets": [
                {"name": "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz", "browser_download_url": "https://example.com/1.tar.gz"},
                {"name": "cpython-3.11.8+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz", "browser_download_url": "https://example.com/2.tar.gz"}
            ]
        }]"#;
        let versions = PythonProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 1);
    }

    #[test]
    fn test_parse_releases_excludes_prerelease() {
        let json = r#"[{
            "tag_name": "20240224",
            "assets": [
                {"name": "cpython-3.13.0a1+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz", "browser_download_url": "https://example.com/alpha.tar.gz"},
                {"name": "cpython-3.12.2+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz", "browser_download_url": "https://example.com/stable.tar.gz"}
            ]
        }]"#;
        let versions = PythonProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 1);
    }

    #[test]
    fn test_parse_releases_empty_returns_error() {
        assert!(PythonProvider::parse_releases("[]").is_err());
    }

    #[test]
    fn test_parse_releases_invalid_json() {
        assert!(PythonProvider::parse_releases("not json").is_err());
    }

    #[test]
    fn test_find_url_macos_arm64() {
        let releases = vec![GitHubRelease {
            tag_name: "20240224".to_string(),
            assets: vec![GitHubAsset {
                name: "cpython-3.11.8+20240224-aarch64-apple-darwin-install_only.tar.gz"
                    .to_string(),
                browser_download_url: "https://example.com/macos.tar.gz".to_string(),
            }],
        }];
        let url =
            PythonProvider::find_url_in_releases(&releases, &v("3.11.8"), &macos_arm64()).unwrap();
        assert_eq!(url, "https://example.com/macos.tar.gz");
    }

    #[test]
    fn test_find_url_not_found() {
        let releases = vec![GitHubRelease {
            tag_name: "20240224".to_string(),
            assets: vec![GitHubAsset {
                name: "cpython-3.11.8+20240224-x86_64-unknown-linux-gnu-install_only.tar.gz"
                    .to_string(),
                browser_download_url: "https://example.com/linux.tar.gz".to_string(),
            }],
        }];
        assert!(
            PythonProvider::find_url_in_releases(&releases, &v("3.99.0"), &linux_x64()).is_err()
        );
    }

    #[test]
    fn test_find_url_ignores_sha256() {
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

    #[test]
    fn test_get_provider_python() {
        assert_eq!(
            super::super::get_provider("python").unwrap().name(),
            "python"
        );
    }

    #[test]
    fn test_get_provider_python3_alias() {
        assert_eq!(
            super::super::get_provider("python3").unwrap().name(),
            "python"
        );
    }

    #[test]
    fn test_temp_env_dirs_empty() {
        assert!(PythonProvider.temp_env_dirs().is_empty());
    }
}

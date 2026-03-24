use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform::{Arch, Platform, Target};
use crate::version::VersionSpec;

use super::{
    ArchiveFormat, Provider, ProviderError, collect_stable_versions, fetch_json,
    resolve_from_candidates,
};

use super::SimpleGitHubRelease as GitHubRelease;

/// Deno tool provider.
///
/// Resolves versions from GitHub releases (`denoland/deno`) and constructs
/// download URLs for the official Deno binary distributions.
///
/// Deno distributes a single binary per platform, packaged as a zip archive.
pub struct DenoProvider;

impl DenoProvider {
    const RELEASES_URL: &str = "https://api.github.com/repos/denoland/deno/releases?per_page=100";

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = fetch_json(Self::RELEASES_URL, "deno")?;
        Self::parse_releases(&body)
    }

    /// Parse GitHub releases JSON into a list of stable Deno versions.
    fn parse_releases(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        let releases: Vec<GitHubRelease> =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "deno".to_string(),
                reason: format!("failed to parse releases: {e}"),
            })?;

        let versions =
            collect_stable_versions(releases.iter().map(|r| Self::parse_tag(&r.tag_name)));

        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "deno".to_string(),
                reason: "no stable versions found in releases".to_string(),
            });
        }

        Ok(versions)
    }

    /// Parse a Deno release tag like "v1.40.5" into a semver Version.
    fn parse_tag(tag: &str) -> Option<semver::Version> {
        let ver_str = tag.strip_prefix('v')?;
        semver::Version::parse(ver_str).ok()
    }

    /// Map platform/arch to Deno's download naming convention.
    fn deno_target(target: &Target) -> Result<&'static str, ProviderError> {
        match (target.platform, target.arch) {
            (Platform::MacOS, Arch::X86_64) => Ok("x86_64-apple-darwin"),
            (Platform::MacOS, Arch::Aarch64) => Ok("aarch64-apple-darwin"),
            (Platform::Linux, Arch::X86_64) => Ok("x86_64-unknown-linux-gnu"),
            (Platform::Linux, Arch::Aarch64) => Ok("aarch64-unknown-linux-gnu"),
            (Platform::Windows, Arch::X86_64) => Ok("x86_64-pc-windows-msvc"),
            (Platform::Windows, Arch::Aarch64) => Err(ProviderError::UnsupportedTarget {
                tool: "deno".to_string(),
                target: target.to_string(),
            }),
        }
    }
}

impl Provider for DenoProvider {
    fn name(&self) -> &str {
        "deno"
    }

    fn resolve_version(
        &self,
        spec: &VersionSpec,
        _target: &Target,
    ) -> Result<semver::Version, ProviderError> {
        let candidates = Self::fetch_versions()?;
        resolve_from_candidates(&candidates, spec, "deno")
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let deno_target = Self::deno_target(target)?;
        Ok(format!(
            "https://github.com/denoland/deno/releases/download/v{version}/deno-{deno_target}.zip"
        ))
    }

    fn archive_format(&self, _target: &Target) -> ArchiveFormat {
        // Deno always distributes as zip on all platforms
        ArchiveFormat::Zip
    }

    fn bin_paths(&self, _version: &semver::Version, _target: &Target) -> Vec<PathBuf> {
        // Deno zip extracts to a flat directory with just the `deno` binary
        vec![PathBuf::from(".")]
    }

    fn env_vars(&self, _install_dir: &Path) -> HashMap<String, String> {
        HashMap::new()
    }

    fn temp_env_dirs(&self) -> Vec<&'static str> {
        vec!["DENO_DIR"]
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(DenoProvider.name(), "deno");
    }

    // --- Download URL ---

    #[test]
    fn test_download_url_macos_arm64() {
        let url = DenoProvider
            .download_url(&v("1.40.5"), &macos_arm64())
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/denoland/deno/releases/download/v1.40.5/deno-aarch64-apple-darwin.zip"
        );
    }

    #[test]
    fn test_download_url_macos_x64() {
        let target = Target::new(Platform::MacOS, Arch::X86_64);
        let url = DenoProvider.download_url(&v("1.40.5"), &target).unwrap();
        assert_eq!(
            url,
            "https://github.com/denoland/deno/releases/download/v1.40.5/deno-x86_64-apple-darwin.zip"
        );
    }

    #[test]
    fn test_download_url_linux_x64() {
        let url = DenoProvider
            .download_url(&v("1.40.5"), &linux_x64())
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/denoland/deno/releases/download/v1.40.5/deno-x86_64-unknown-linux-gnu.zip"
        );
    }

    #[test]
    fn test_download_url_linux_arm64() {
        let url = DenoProvider
            .download_url(&v("1.40.5"), &linux_arm64())
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/denoland/deno/releases/download/v1.40.5/deno-aarch64-unknown-linux-gnu.zip"
        );
    }

    #[test]
    fn test_download_url_windows_x64() {
        let url = DenoProvider
            .download_url(&v("1.40.5"), &windows_x64())
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/denoland/deno/releases/download/v1.40.5/deno-x86_64-pc-windows-msvc.zip"
        );
    }

    #[test]
    fn test_download_url_windows_arm64_unsupported() {
        let target = Target::new(Platform::Windows, Arch::Aarch64);
        let result = DenoProvider.download_url(&v("1.40.5"), &target);
        assert!(result.is_err());
    }

    // --- Archive format ---

    #[test]
    fn test_archive_format_always_zip() {
        assert_eq!(
            DenoProvider.archive_format(&macos_arm64()),
            ArchiveFormat::Zip
        );
        assert_eq!(
            DenoProvider.archive_format(&linux_x64()),
            ArchiveFormat::Zip
        );
        assert_eq!(
            DenoProvider.archive_format(&windows_x64()),
            ArchiveFormat::Zip
        );
    }

    // --- Bin paths ---

    #[test]
    fn test_bin_paths() {
        let paths = DenoProvider.bin_paths(&v("1.40.5"), &macos_arm64());
        assert_eq!(paths, vec![PathBuf::from(".")]);
    }

    // --- Env vars ---

    #[test]
    fn test_env_vars_empty() {
        let vars = DenoProvider.env_vars(Path::new("/cache/deno/1.40.5"));
        assert!(vars.is_empty()); // DENO_DIR managed by TempDirs
    }

    // --- parse_tag ---

    #[test]
    fn test_parse_tag_valid() {
        assert_eq!(DenoProvider::parse_tag("v1.40.5"), Some(v("1.40.5")));
        assert_eq!(DenoProvider::parse_tag("v2.0.0"), Some(v("2.0.0")));
    }

    #[test]
    fn test_parse_tag_no_prefix() {
        assert!(DenoProvider::parse_tag("1.40.5").is_none());
    }

    #[test]
    fn test_parse_tag_invalid() {
        assert!(DenoProvider::parse_tag("vnotaversion").is_none());
    }

    // --- parse_releases ---

    #[test]
    fn test_parse_releases_basic() {
        let json = r#"[
            {"tag_name": "v1.40.5"},
            {"tag_name": "v1.40.4"},
            {"tag_name": "v1.39.0"}
        ]"#;
        let versions = DenoProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 3);
        assert!(versions.contains(&v("1.40.5")));
        assert!(versions.contains(&v("1.39.0")));
    }

    #[test]
    fn test_parse_releases_filters_prerelease() {
        let json = r#"[
            {"tag_name": "v2.0.0-rc.1"},
            {"tag_name": "v1.40.5"}
        ]"#;
        let versions = DenoProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], v("1.40.5"));
    }

    #[test]
    fn test_parse_releases_all_unparseable_returns_error() {
        let json = r#"[{"tag_name": "notaversion"}, {"tag_name": "also-bad"}]"#;
        assert!(DenoProvider::parse_releases(json).is_err());
    }

    #[test]
    fn test_parse_releases_deduplicates() {
        let json = r#"[
            {"tag_name": "v1.40.5"},
            {"tag_name": "v1.40.5"}
        ]"#;
        let versions = DenoProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 1);
    }

    #[test]
    fn test_parse_releases_empty_returns_error() {
        assert!(DenoProvider::parse_releases("[]").is_err());
    }

    #[test]
    fn test_parse_releases_invalid_json() {
        assert!(DenoProvider::parse_releases("not json").is_err());
    }

    // --- deno_target ---

    #[test]
    fn test_deno_target_all_supported() {
        assert_eq!(
            DenoProvider::deno_target(&macos_arm64()).unwrap(),
            "aarch64-apple-darwin"
        );
        assert_eq!(
            DenoProvider::deno_target(&linux_x64()).unwrap(),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            DenoProvider::deno_target(&linux_arm64()).unwrap(),
            "aarch64-unknown-linux-gnu"
        );
        assert_eq!(
            DenoProvider::deno_target(&windows_x64()).unwrap(),
            "x86_64-pc-windows-msvc"
        );
    }

    #[test]
    fn test_deno_target_windows_arm64_unsupported() {
        let target = Target::new(Platform::Windows, Arch::Aarch64);
        assert!(DenoProvider::deno_target(&target).is_err());
    }

    // --- get_provider ---

    #[test]
    fn test_get_provider_deno() {
        assert_eq!(super::super::get_provider("deno").unwrap().name(), "deno");
    }
}

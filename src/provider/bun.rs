use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform::{Arch, Platform, Target};
use crate::version::VersionSpec;

use super::{
    ArchiveFormat, Provider, ProviderError, fetch_json, parse_github_releases,
    resolve_from_candidates,
};

/// Bun tool provider.
///
/// Resolves versions from GitHub releases (`oven-sh/bun`) and constructs
/// download URLs for the official Bun binary distributions.
///
/// Bun distributes platform-specific zip archives containing the `bun` binary
/// and `bunx` symlink.
pub struct BunProvider;

impl BunProvider {
    const RELEASES_URL: &str = "https://api.github.com/repos/oven-sh/bun/releases?per_page=100";

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = fetch_json(Self::RELEASES_URL, "bun")?;
        Self::parse_releases(&body)
    }

    /// Parse GitHub releases JSON into a list of stable Bun versions.
    fn parse_releases(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        parse_github_releases(json, "bun", Self::parse_tag)
    }

    /// Parse a Bun release tag like "bun-v1.0.25" into a semver Version.
    fn parse_tag(tag: &str) -> Option<semver::Version> {
        let ver_str = tag.strip_prefix("bun-v")?;
        semver::Version::parse(ver_str).ok()
    }

    /// Map platform/arch to Bun's download naming convention.
    fn bun_target(target: &Target) -> Result<&'static str, ProviderError> {
        match (target.platform, target.arch) {
            (Platform::MacOS, Arch::X86_64) => Ok("darwin-x64"),
            (Platform::MacOS, Arch::Aarch64) => Ok("darwin-aarch64"),
            (Platform::Linux, Arch::X86_64) => Ok("linux-x64"),
            (Platform::Linux, Arch::Aarch64) => Ok("linux-aarch64"),
            (Platform::Windows, Arch::X86_64) => Ok("windows-x64"),
            (Platform::Windows, Arch::Aarch64) => Err(ProviderError::UnsupportedTarget {
                tool: "bun".to_string(),
                target: target.to_string(),
            }),
        }
    }
}

impl Provider for BunProvider {
    fn name(&self) -> &str {
        "bun"
    }

    fn resolve_version(
        &self,
        spec: &VersionSpec,
        _target: &Target,
    ) -> Result<semver::Version, ProviderError> {
        let candidates = Self::fetch_versions()?;
        resolve_from_candidates(&candidates, spec, "bun")
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let bun_target = Self::bun_target(target)?;
        Ok(format!(
            "https://github.com/oven-sh/bun/releases/download/bun-v{version}/bun-{bun_target}.zip"
        ))
    }

    fn archive_format(&self, _target: &Target) -> ArchiveFormat {
        // Bun always distributes as zip on all platforms
        ArchiveFormat::Zip
    }

    fn bin_paths(&self, _version: &semver::Version, target: &Target) -> Vec<PathBuf> {
        // Bun zip extracts to a directory like bun-darwin-aarch64/ containing bun and bunx.
        // If the target is unsupported, download_url will fail before bin_paths is called,
        // but we handle it gracefully here by returning the base "bun" directory.
        match Self::bun_target(target) {
            Ok(t) => vec![PathBuf::from(format!("bun-{t}"))],
            Err(_) => vec![PathBuf::from("bun")],
        }
    }

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        HashMap::from([(
            "BUN_INSTALL".to_string(),
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
        assert_eq!(BunProvider.name(), "bun");
    }

    // --- Download URL ---

    #[test]
    fn test_download_url_macos_arm64() {
        let url = BunProvider
            .download_url(&v("1.0.25"), &macos_arm64())
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/oven-sh/bun/releases/download/bun-v1.0.25/bun-darwin-aarch64.zip"
        );
    }

    #[test]
    fn test_download_url_macos_x64() {
        let target = Target::new(Platform::MacOS, Arch::X86_64);
        let url = BunProvider.download_url(&v("1.0.25"), &target).unwrap();
        assert_eq!(
            url,
            "https://github.com/oven-sh/bun/releases/download/bun-v1.0.25/bun-darwin-x64.zip"
        );
    }

    #[test]
    fn test_download_url_linux_x64() {
        let url = BunProvider
            .download_url(&v("1.0.25"), &linux_x64())
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/oven-sh/bun/releases/download/bun-v1.0.25/bun-linux-x64.zip"
        );
    }

    #[test]
    fn test_download_url_linux_arm64() {
        let url = BunProvider
            .download_url(&v("1.0.25"), &linux_arm64())
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/oven-sh/bun/releases/download/bun-v1.0.25/bun-linux-aarch64.zip"
        );
    }

    #[test]
    fn test_download_url_windows_x64() {
        let url = BunProvider
            .download_url(&v("1.0.25"), &windows_x64())
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/oven-sh/bun/releases/download/bun-v1.0.25/bun-windows-x64.zip"
        );
    }

    #[test]
    fn test_download_url_windows_arm64_unsupported() {
        let target = Target::new(Platform::Windows, Arch::Aarch64);
        assert!(BunProvider.download_url(&v("1.0.25"), &target).is_err());
    }

    // --- Archive format ---

    #[test]
    fn test_archive_format_always_zip() {
        assert_eq!(
            BunProvider.archive_format(&macos_arm64()),
            ArchiveFormat::Zip
        );
        assert_eq!(BunProvider.archive_format(&linux_x64()), ArchiveFormat::Zip);
        assert_eq!(
            BunProvider.archive_format(&windows_x64()),
            ArchiveFormat::Zip
        );
    }

    // --- Bin paths ---

    #[test]
    fn test_bin_paths_macos_arm64() {
        let paths = BunProvider.bin_paths(&v("1.0.25"), &macos_arm64());
        assert_eq!(paths, vec![PathBuf::from("bun-darwin-aarch64")]);
    }

    #[test]
    fn test_bin_paths_windows_x64() {
        let paths = BunProvider.bin_paths(&v("1.0.25"), &windows_x64());
        assert_eq!(paths, vec![PathBuf::from("bun-windows-x64")]);
    }

    #[test]
    fn test_bin_paths_linux_x64() {
        let paths = BunProvider.bin_paths(&v("1.0.25"), &linux_x64());
        assert_eq!(paths, vec![PathBuf::from("bun-linux-x64")]);
    }

    // --- Env vars ---

    #[test]
    fn test_env_vars() {
        let vars = BunProvider.env_vars(Path::new("/cache/bun/1.0.25"));
        assert_eq!(vars.get("BUN_INSTALL").unwrap(), "/cache/bun/1.0.25");
        assert_eq!(vars.len(), 1);
    }

    // --- parse_tag ---

    #[test]
    fn test_parse_tag_valid() {
        assert_eq!(BunProvider::parse_tag("bun-v1.0.25"), Some(v("1.0.25")));
        assert_eq!(BunProvider::parse_tag("bun-v1.1.0"), Some(v("1.1.0")));
    }

    #[test]
    fn test_parse_tag_no_prefix() {
        assert!(BunProvider::parse_tag("v1.0.25").is_none());
        assert!(BunProvider::parse_tag("1.0.25").is_none());
    }

    #[test]
    fn test_parse_tag_invalid() {
        assert!(BunProvider::parse_tag("bun-vnotaversion").is_none());
    }

    // --- parse_releases ---

    #[test]
    fn test_parse_releases_basic() {
        let json = r#"[
            {"tag_name": "bun-v1.0.25"},
            {"tag_name": "bun-v1.0.24"},
            {"tag_name": "bun-v1.1.0"}
        ]"#;
        let versions = BunProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 3);
        assert!(versions.contains(&v("1.0.25")));
        assert!(versions.contains(&v("1.1.0")));
    }

    #[test]
    fn test_parse_releases_filters_prerelease() {
        let json = r#"[
            {"tag_name": "bun-v1.1.0-canary.1"},
            {"tag_name": "bun-v1.0.25"}
        ]"#;
        let versions = BunProvider::parse_releases(json).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], v("1.0.25"));
    }

    #[test]
    fn test_parse_releases_deduplicates() {
        let json = r#"[
            {"tag_name": "bun-v1.0.25"},
            {"tag_name": "bun-v1.0.25"}
        ]"#;
        assert_eq!(BunProvider::parse_releases(json).unwrap().len(), 1);
    }

    #[test]
    fn test_parse_releases_all_unparseable() {
        let json = r#"[{"tag_name": "not-a-bun-tag"}]"#;
        assert!(BunProvider::parse_releases(json).is_err());
    }

    #[test]
    fn test_parse_releases_empty_returns_error() {
        assert!(BunProvider::parse_releases("[]").is_err());
    }

    #[test]
    fn test_parse_releases_invalid_json() {
        assert!(BunProvider::parse_releases("not json").is_err());
    }

    // --- bun_target ---

    #[test]
    fn test_bun_target_all_supported() {
        assert_eq!(
            BunProvider::bun_target(&macos_arm64()).unwrap(),
            "darwin-aarch64"
        );
        let macos_x64 = Target::new(Platform::MacOS, Arch::X86_64);
        assert_eq!(BunProvider::bun_target(&macos_x64).unwrap(), "darwin-x64");
        assert_eq!(BunProvider::bun_target(&linux_x64()).unwrap(), "linux-x64");
        assert_eq!(
            BunProvider::bun_target(&linux_arm64()).unwrap(),
            "linux-aarch64"
        );
        assert_eq!(
            BunProvider::bun_target(&windows_x64()).unwrap(),
            "windows-x64"
        );
    }

    #[test]
    fn test_bun_target_windows_arm64_unsupported() {
        let target = Target::new(Platform::Windows, Arch::Aarch64);
        assert!(BunProvider::bun_target(&target).is_err());
    }

    // --- get_provider ---

    #[test]
    fn test_get_provider_bun() {
        assert_eq!(super::super::get_provider("bun").unwrap().name(), "bun");
    }

    #[test]
    fn test_get_provider_bunx_alias() {
        assert_eq!(super::super::get_provider("bunx").unwrap().name(), "bun");
    }
}

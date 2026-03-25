use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform::Target;
use crate::version::VersionSpec;

use super::{ArchiveFormat, Provider, ProviderError, fetch_json, resolve_from_candidates};

/// Rust tool provider using official standalone installers.
///
/// Resolves versions from the official Rust release channel manifest at
/// `static.rust-lang.org`. This avoids GitHub API rate limits and provides
/// faster, more reliable version resolution.
///
/// Unlike other providers, Rust requires a post-install step (`install.sh`)
/// to place binaries in the correct directory structure.
pub struct RustProvider;

/// Minimum Rust minor version to include in the candidate list.
/// Rust 1.20.0 (Aug 2017) is a reasonable lower bound — older versions
/// lack many modern features and are unlikely to be requested.
const MIN_MINOR_VERSION: u64 = 20;

impl RustProvider {
    const CHANNEL_URL: &str = "https://static.rust-lang.org/dist/channel-rust-stable.toml";

    /// Fetch the latest stable Rust version from the channel manifest.
    fn fetch_stable_version() -> Result<semver::Version, ProviderError> {
        let body = fetch_json(Self::CHANNEL_URL, "rust")?;
        Self::parse_channel_version(&body)
    }

    /// Parse the stable version from the channel TOML manifest.
    ///
    /// Extracts the `pkg.rust.version` field which contains e.g. "1.94.0 (4a4ef493e 2026-03-02)".
    /// Uses the `toml` crate (already a project dependency) to deserialize only the fields we need,
    /// which is both more robust and more correct than line-scanning — the previous approach
    /// returned the first `version = "..."` line, which could match the wrong package.
    fn parse_channel_version(toml_body: &str) -> Result<semver::Version, ProviderError> {
        #[derive(serde::Deserialize)]
        struct ChannelManifest {
            pkg: PkgSection,
        }
        #[derive(serde::Deserialize)]
        struct PkgSection {
            rust: PkgEntry,
        }
        #[derive(serde::Deserialize)]
        struct PkgEntry {
            version: String,
        }

        let manifest: ChannelManifest =
            toml::from_str(toml_body).map_err(|e| ProviderError::ResolutionFailed {
                tool: "rust".to_string(),
                reason: format!("failed to parse channel manifest: {e}"),
            })?;

        // Version field contains e.g. "1.94.0 (4a4ef493e 2026-03-02)" — take the first token
        let ver_str = manifest
            .pkg
            .rust
            .version
            .split_whitespace()
            .next()
            .ok_or_else(|| ProviderError::ResolutionFailed {
                tool: "rust".to_string(),
                reason: "empty version string in channel manifest".to_string(),
            })?;

        semver::Version::parse(ver_str).map_err(|e| ProviderError::ResolutionFailed {
            tool: "rust".to_string(),
            reason: format!("invalid version `{ver_str}` in channel manifest: {e}"),
        })
    }

    /// Generate all plausible Rust versions from 1.MIN to current stable.
    ///
    /// Rust follows a predictable 6-week release cadence where each release
    /// increments the minor version. Every `1.x.0` from 1.0.0 to current exists.
    /// Patch releases (1.x.1, 1.x.2) are rare — we include .0 for all and add
    /// common patch versions that are known to exist.
    fn generate_candidates(latest: &semver::Version) -> Vec<semver::Version> {
        let range_count = if latest.minor >= MIN_MINOR_VERSION {
            (latest.minor - MIN_MINOR_VERSION + 1) as usize
        } else {
            0
        };
        let count = range_count + usize::from(latest.patch > 0);
        let mut candidates = Vec::with_capacity(count);
        for minor in MIN_MINOR_VERSION..=latest.minor {
            candidates.push(semver::Version::new(1, minor, 0));
        }
        // Include the exact latest if it has a patch > 0 (e.g. 1.77.2).
        // The contains() check is unnecessary — all generated versions have patch=0,
        // so a latest with patch > 0 is guaranteed to be absent.
        if latest.patch > 0 {
            candidates.push(latest.clone());
        }
        candidates
    }

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let latest = Self::fetch_stable_version()?;
        Ok(Self::generate_candidates(&latest))
    }
}

impl Provider for RustProvider {
    fn name(&self) -> &str {
        "rust"
    }

    fn resolve_version(
        &self,
        spec: &VersionSpec,
        _target: &Target,
    ) -> Result<semver::Version, ProviderError> {
        let candidates = Self::fetch_versions()?;
        resolve_from_candidates(&candidates, spec, "rust")
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        if target.platform == crate::platform::Platform::Windows {
            return Err(ProviderError::UnsupportedTarget {
                tool: "rust".to_string(),
                target: target.to_string(),
            });
        }
        let triple = target.triple();
        Ok(format!(
            "https://static.rust-lang.org/dist/rust-{version}-{triple}.tar.gz"
        ))
    }

    fn archive_format(&self, _target: &Target) -> ArchiveFormat {
        ArchiveFormat::TarGz
    }

    fn bin_paths(&self, _version: &semver::Version, _target: &Target) -> Vec<PathBuf> {
        vec![PathBuf::from("bin")]
    }

    fn env_vars(&self, _install_dir: &Path) -> HashMap<String, String> {
        // No special env vars needed — cargo/rustc only need PATH,
        // which is handled via bin_paths(). Setting RUSTUP_HOME would
        // collide with the user's existing rustup installation.
        HashMap::new()
    }

    fn temp_env_dirs(&self) -> Vec<&'static str> {
        vec!["CARGO_HOME"]
    }

    fn list_versions(&self, _target: &Target) -> Result<Vec<semver::Version>, ProviderError> {
        let mut versions = Self::fetch_versions()?;
        versions.sort_by(|a, b| b.cmp(a));
        Ok(versions)
    }

    fn post_install_command(
        &self,
        version: &semver::Version,
        target: &Target,
        install_dir: &std::path::Path,
    ) -> Option<String> {
        let triple = target.triple();
        let prefix = install_dir.display();
        Some(format!(
            "rust-{version}-{triple}/install.sh '--prefix={prefix}' --without=rust-docs --disable-ldconfig"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(RustProvider.name(), "rust");
    }

    #[test]
    fn test_archive_format() {
        assert_eq!(
            RustProvider.archive_format(&macos_arm64()),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            RustProvider.archive_format(&linux_x64()),
            ArchiveFormat::TarGz
        );
    }

    #[test]
    fn test_bin_paths() {
        let paths = RustProvider.bin_paths(&v("1.77.0"), &macos_arm64());
        assert_eq!(paths, vec![PathBuf::from("bin")]);
    }

    #[test]
    fn test_env_vars_empty() {
        let vars = RustProvider.env_vars(Path::new("/cache/rust/1.77.0"));
        assert!(vars.is_empty());
    }

    #[test]
    fn test_temp_env_dirs() {
        assert_eq!(RustProvider.temp_env_dirs(), vec!["CARGO_HOME"]);
    }

    #[test]
    fn test_download_url_macos_arm64() {
        let url = RustProvider
            .download_url(&v("1.77.0"), &macos_arm64())
            .unwrap();
        assert_eq!(
            url,
            "https://static.rust-lang.org/dist/rust-1.77.0-aarch64-apple-darwin.tar.gz"
        );
    }

    #[test]
    fn test_download_url_linux_x64() {
        let url = RustProvider
            .download_url(&v("1.77.0"), &linux_x64())
            .unwrap();
        assert_eq!(
            url,
            "https://static.rust-lang.org/dist/rust-1.77.0-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn test_post_install_command() {
        let cmd = RustProvider
            .post_install_command(&v("1.77.0"), &linux_x64(), Path::new("/cache/rust/1.77.0"))
            .unwrap();
        assert!(cmd.contains("install.sh"));
        assert!(cmd.contains("'--prefix=/cache/rust/1.77.0'")); // quoted for shell safety
        assert!(cmd.contains("rust-1.77.0-x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_download_url_windows_unsupported() {
        assert!(
            RustProvider
                .download_url(&v("1.77.0"), &windows_x64())
                .is_err()
        );
    }

    #[test]
    fn test_get_provider_cargo_alias() {
        assert_eq!(super::super::get_provider("cargo").unwrap().name(), "rust");
    }

    #[test]
    fn test_get_provider_rust() {
        assert_eq!(super::super::get_provider("rust").unwrap().name(), "rust");
    }

    #[test]
    fn test_get_provider_rustc_alias() {
        assert_eq!(super::super::get_provider("rustc").unwrap().name(), "rust");
    }

    // --- Channel manifest parsing ---

    #[test]
    fn test_parse_channel_version_valid() {
        let toml = r#"
manifest-version = "2"
date = "2026-03-05"

[pkg.rust]
version = "1.94.0 (4a4ef493e 2026-03-02)"
"#;
        let version = RustProvider::parse_channel_version(toml).unwrap();
        assert_eq!(version, v("1.94.0"));
    }

    #[test]
    fn test_parse_channel_version_no_version_returns_error() {
        let toml = r#"
manifest-version = "2"
date = "2026-03-05"
"#;
        assert!(RustProvider::parse_channel_version(toml).is_err());
    }

    #[test]
    fn test_parse_channel_version_invalid_version_string() {
        let toml = r#"
[pkg.rust]
version = "not-a-version"
"#;
        assert!(RustProvider::parse_channel_version(toml).is_err());
    }

    #[test]
    fn test_parse_channel_version_multi_package_manifest() {
        // The real manifest has version lines for cargo, clippy, rustfmt, etc.
        // We must extract only pkg.rust.version, not the first version = line.
        let toml = r#"
manifest-version = "2"
date = "2026-03-05"

[pkg.cargo]
version = "0.95.0 (abc1234 2026-03-01)"

[pkg.rust]
version = "1.94.0 (4a4ef493e 2026-03-02)"

[pkg.clippy]
version = "0.1.94 (def5678 2026-03-02)"
"#;
        let version = RustProvider::parse_channel_version(toml).unwrap();
        assert_eq!(version, v("1.94.0"));
    }

    #[test]
    fn test_parse_channel_version_no_pkg_rust_returns_error() {
        // Manifest that has other packages but no [pkg.rust] section
        let toml = r#"
manifest-version = "2"

[pkg.cargo]
version = "0.95.0 (abc1234 2026-03-01)"
"#;
        assert!(RustProvider::parse_channel_version(toml).is_err());
    }

    // --- Candidate generation ---

    #[test]
    fn test_generate_candidates_includes_range() {
        let latest = v("1.80.0");
        let candidates = RustProvider::generate_candidates(&latest);
        // Should include 1.20.0 through 1.80.0
        assert_eq!(candidates.len(), 61); // 80 - 20 + 1
        assert!(candidates.contains(&v("1.20.0")));
        assert!(candidates.contains(&v("1.80.0")));
        assert!(!candidates.contains(&v("1.19.0")));
    }

    #[test]
    fn test_generate_candidates_with_patch_version() {
        let latest = v("1.77.2");
        let candidates = RustProvider::generate_candidates(&latest);
        assert!(candidates.contains(&v("1.77.0")));
        assert!(candidates.contains(&v("1.77.2")));
    }

    #[test]
    fn test_generate_candidates_no_duplicate_for_patch_zero() {
        let latest = v("1.80.0");
        let candidates = RustProvider::generate_candidates(&latest);
        let target = v("1.80.0");
        let count = candidates.iter().filter(|c| **c == target).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_generate_candidates_below_min_minor_returns_empty() {
        // If latest.minor < MIN_MINOR_VERSION, the range is empty
        let latest = v("1.19.0");
        let candidates = RustProvider::generate_candidates(&latest);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_generate_candidates_exact_min_minor() {
        let latest = v("1.20.0");
        let candidates = RustProvider::generate_candidates(&latest);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], v("1.20.0"));
    }
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform::{Platform, Target};
use crate::version::VersionSpec;

use super::{ArchiveFormat, Provider, ProviderError, fetch_json, resolve_from_candidates};

/// Ruby tool provider using ruby/ruby-builder prebuilt binaries.
///
/// Resolves versions from GitHub releases and constructs download URLs
/// for prebuilt Ruby binaries distributed as tar.gz archives.
pub struct RubyProvider;

impl RubyProvider {
    const RELEASES_URL: &str =
        "https://api.github.com/repos/ruby/ruby-builder/releases?per_page=30";

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = fetch_json(Self::RELEASES_URL, "ruby")?;
        Self::parse_releases(&body)
    }

    /// Parse GitHub releases JSON into a list of stable Ruby versions.
    ///
    /// ruby-builder tags look like "toolcache" or version-like tags.
    /// We extract versions from release asset filenames instead.
    fn parse_releases(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Release {
            assets: Vec<Asset>,
        }

        #[derive(Deserialize)]
        struct Asset {
            name: String,
        }

        let releases: Vec<Release> =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "ruby".to_string(),
                reason: format!("failed to parse releases: {e}"),
            })?;

        let versions = super::collect_stable_versions(
            releases
                .iter()
                .flat_map(|r| r.assets.iter())
                .map(|a| Self::extract_version(&a.name)),
        );

        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "ruby".to_string(),
                reason: "no stable Ruby versions found in releases".to_string(),
            });
        }

        Ok(versions)
    }

    /// Extract a Ruby version from an asset filename like "ruby-3.3.0-ubuntu-22.04.tar.gz".
    fn extract_version(name: &str) -> Option<semver::Version> {
        let stem = name.strip_prefix("ruby-")?;
        let ver_str = stem.split('-').next()?;
        semver::Version::parse(ver_str).ok()
    }

    /// Map target to ruby-builder's platform naming.
    fn ruby_platform(target: &Target) -> Result<&'static str, ProviderError> {
        match (target.platform, target.arch) {
            (Platform::MacOS, crate::platform::Arch::Aarch64) => Ok("macos-arm64"),
            (Platform::MacOS, crate::platform::Arch::X86_64) => Ok("macos-latest"),
            (Platform::Linux, crate::platform::Arch::X86_64) => Ok("ubuntu-22.04"),
            (Platform::Linux, crate::platform::Arch::Aarch64) => Ok("ubuntu-22.04-arm"),
            _ => Err(ProviderError::UnsupportedTarget {
                tool: "ruby".to_string(),
                target: target.to_string(),
            }),
        }
    }
}

impl Provider for RubyProvider {
    fn name(&self) -> &str {
        "ruby"
    }

    fn resolve_version(
        &self,
        spec: &VersionSpec,
        _target: &Target,
    ) -> Result<semver::Version, ProviderError> {
        let candidates = Self::fetch_versions()?;
        resolve_from_candidates(&candidates, spec, "ruby")
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let platform = Self::ruby_platform(target)?;
        Ok(format!(
            "https://github.com/ruby/ruby-builder/releases/download/toolcache/ruby-{version}-{platform}.tar.gz"
        ))
    }

    fn archive_format(&self, _target: &Target) -> ArchiveFormat {
        ArchiveFormat::TarGz
    }

    fn bin_paths(&self, version: &semver::Version, _target: &Target) -> Vec<PathBuf> {
        vec![PathBuf::from(format!("ruby-{version}")).join("bin")]
    }

    fn env_vars(&self, _install_dir: &Path) -> HashMap<String, String> {
        HashMap::new()
    }

    fn temp_env_dirs(&self) -> Vec<&'static str> {
        vec!["GEM_HOME", "GEM_PATH"]
    }

    fn list_versions(&self, _target: &Target) -> Result<Vec<semver::Version>, ProviderError> {
        let mut versions = Self::fetch_versions()?;
        versions.sort_by(|a, b| b.cmp(a));
        Ok(versions)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(RubyProvider.name(), "ruby");
    }

    #[test]
    fn test_archive_format() {
        assert_eq!(
            RubyProvider.archive_format(&macos_arm64()),
            ArchiveFormat::TarGz
        );
    }

    #[test]
    fn test_bin_paths() {
        let paths = RubyProvider.bin_paths(&v("3.3.0"), &macos_arm64());
        assert_eq!(paths, vec![PathBuf::from("ruby-3.3.0/bin")]);
    }

    #[test]
    fn test_temp_env_dirs() {
        let dirs = RubyProvider.temp_env_dirs();
        assert!(dirs.contains(&"GEM_HOME"));
        assert!(dirs.contains(&"GEM_PATH"));
    }

    #[test]
    fn test_download_url_macos_arm64() {
        let url = RubyProvider
            .download_url(&v("3.3.0"), &macos_arm64())
            .unwrap();
        assert!(url.contains("ruby-3.3.0-macos-arm64.tar.gz"));
    }

    #[test]
    fn test_download_url_linux_x64() {
        let url = RubyProvider
            .download_url(&v("3.3.0"), &linux_x64())
            .unwrap();
        assert!(url.contains("ruby-3.3.0-ubuntu-22.04.tar.gz"));
    }

    #[test]
    fn test_download_url_windows_unsupported() {
        assert!(
            RubyProvider
                .download_url(&v("3.3.0"), &windows_x64())
                .is_err()
        );
    }

    #[test]
    fn test_ruby_platform() {
        assert_eq!(
            RubyProvider::ruby_platform(&macos_arm64()).unwrap(),
            "macos-arm64"
        );
        assert_eq!(
            RubyProvider::ruby_platform(&linux_x64()).unwrap(),
            "ubuntu-22.04"
        );
    }

    #[test]
    fn test_extract_version() {
        assert_eq!(
            RubyProvider::extract_version("ruby-3.3.0-ubuntu-22.04.tar.gz"),
            Some(v("3.3.0"))
        );
        assert!(RubyProvider::extract_version("something-else.tar.gz").is_none());
        // Non-semver version returns None (not Some(None))
        assert!(RubyProvider::extract_version("ruby-head-ubuntu-22.04.tar.gz").is_none());
    }

    #[test]
    fn test_ruby_platform_macos_x64() {
        let target = Target::new(Platform::MacOS, crate::platform::Arch::X86_64);
        assert_eq!(
            RubyProvider::ruby_platform(&target).unwrap(),
            "macos-latest"
        );
    }

    #[test]
    fn test_ruby_platform_linux_arm64() {
        assert_eq!(
            RubyProvider::ruby_platform(&linux_arm64()).unwrap(),
            "ubuntu-22.04-arm"
        );
    }

    #[test]
    fn test_get_provider_ruby() {
        assert_eq!(super::super::get_provider("ruby").unwrap().name(), "ruby");
    }

    #[test]
    fn test_get_provider_rb_alias() {
        assert_eq!(super::super::get_provider("rb").unwrap().name(), "ruby");
    }
}

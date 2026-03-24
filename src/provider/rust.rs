use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform::Target;
use crate::version::VersionSpec;

use super::{
    ArchiveFormat, Provider, ProviderError, fetch_json, parse_github_releases,
    resolve_from_candidates,
};

/// Rust tool provider using official standalone installers.
///
/// Resolves versions from GitHub releases (`rust-lang/rust`) and constructs
/// download URLs for standalone Rust toolchain installers from `static.rust-lang.org`.
///
/// Unlike other providers, Rust requires a post-install step (`install.sh`)
/// to place binaries in the correct directory structure.
pub struct RustProvider;

impl RustProvider {
    const RELEASES_URL: &str = "https://api.github.com/repos/rust-lang/rust/releases?per_page=30";

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = fetch_json(Self::RELEASES_URL, "rust")?;
        Self::parse_releases(&body)
    }

    fn parse_releases(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        parse_github_releases(json, "rust", Self::parse_tag)
    }

    /// Parse a Rust release tag like "1.77.0" into a semver Version.
    fn parse_tag(tag: &str) -> Option<semver::Version> {
        semver::Version::parse(tag).ok()
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

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        HashMap::from([(
            "RUSTUP_HOME".to_string(),
            install_dir.to_string_lossy().to_string(),
        )])
    }

    fn temp_env_dirs(&self) -> Vec<&'static str> {
        vec!["CARGO_HOME"]
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
            "rust-{version}-{triple}/install.sh --prefix={prefix} --without=rust-docs --disable-ldconfig"
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
    fn test_env_vars() {
        let vars = RustProvider.env_vars(Path::new("/cache/rust/1.77.0"));
        assert_eq!(vars.get("RUSTUP_HOME").unwrap(), "/cache/rust/1.77.0");
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
        assert!(cmd.contains("--prefix=/cache/rust/1.77.0"));
        assert!(cmd.contains("rust-1.77.0-x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_parse_tag() {
        assert_eq!(RustProvider::parse_tag("1.77.0"), Some(v("1.77.0")));
        assert!(RustProvider::parse_tag("nightly").is_none());
    }

    #[test]
    fn test_get_provider_rust() {
        assert_eq!(super::super::get_provider("rust").unwrap().name(), "rust");
    }

    #[test]
    fn test_get_provider_rustc_alias() {
        assert_eq!(super::super::get_provider("rustc").unwrap().name(), "rust");
    }
}

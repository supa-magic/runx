use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::platform::{Arch, Platform, Target};
use crate::version::VersionSpec;

use super::{
    ArchiveFormat, Provider, ProviderError, collect_stable_versions, fetch_json,
    resolve_from_candidates,
};

/// Adoptium API version entry.
#[derive(Debug, Deserialize)]
struct AdoptiumVersion {
    major: u64,
    minor: u64,
    security: u64,
    semver: String,
}

/// Adoptium API available releases response.
#[derive(Debug, Deserialize)]
struct AdoptiumAvailableReleases {
    available_releases: Vec<u64>,
}

/// Java tool provider using Eclipse Adoptium (Temurin) builds.
///
/// Resolves versions from the Adoptium API and constructs download URLs
/// for the official Temurin JDK binary distributions.
///
/// Uses the `/v3/binary/latest/{major}/ga/` endpoint for downloads, which
/// handles build metadata internally and always returns the latest GA build.
pub struct JavaProvider;

impl JavaProvider {
    const AVAILABLE_URL: &str = "https://api.adoptium.net/v3/info/available_releases";

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = fetch_json(Self::AVAILABLE_URL, "java")?;
        Self::parse_available(&body)
    }

    /// Parse Adoptium available releases into semver versions.
    ///
    /// Fetches the latest GA release for each available major version.
    /// Errors for individual majors are collected and surfaced if no versions
    /// can be resolved at all.
    fn parse_available(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        let info: AdoptiumAvailableReleases =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "java".to_string(),
                reason: format!("failed to parse available releases: {e}"),
            })?;

        let mut versions = Vec::new();
        let mut last_error = None;

        for major in &info.available_releases {
            let url = format!(
                "https://api.adoptium.net/v3/info/release_versions?architecture=x64&heap_size=normal&image_type=jdk&os=linux&page=0&page_size=1&project=jdk&release_type=ga&sort_method=DEFAULT&sort_order=DESC&vendor=eclipse&version=%5B{major}%2C{}%29",
                major + 1
            );
            match fetch_json(&url, "java").and_then(|b| Self::parse_version_response(&b)) {
                Ok(parsed) => versions.extend(parsed),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        if versions.is_empty() {
            return Err(
                last_error.unwrap_or_else(|| ProviderError::ResolutionFailed {
                    tool: "java".to_string(),
                    reason: "no Java versions found".to_string(),
                }),
            );
        }

        versions.sort_by(|a, b| b.cmp(a));
        versions.dedup();
        Ok(versions)
    }

    fn parse_version_response(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        #[derive(Deserialize)]
        struct VersionsResponse {
            versions: Vec<AdoptiumVersion>,
        }

        let resp: VersionsResponse =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "java".to_string(),
                reason: format!("failed to parse version response: {e}"),
            })?;

        Ok(collect_stable_versions(resp.versions.into_iter().map(
            |v| {
                // Adoptium semver field includes build metadata, strip it
                let clean = v
                    .semver
                    .split_once('+')
                    .map_or(v.semver.as_str(), |(pre, _)| pre);
                semver::Version::parse(clean)
                    .ok()
                    .or_else(|| Some(semver::Version::new(v.major, v.minor, v.security)))
            },
        )))
    }

    /// Map target to Adoptium API naming.
    fn adoptium_os(platform: Platform) -> &'static str {
        match platform {
            Platform::MacOS => "mac",
            Platform::Linux => "linux",
            Platform::Windows => "windows",
        }
    }

    fn adoptium_arch(arch: Arch) -> &'static str {
        match arch {
            Arch::X86_64 => "x64",
            Arch::Aarch64 => "aarch64",
        }
    }

    /// Return the JDK root directory relative to install_dir.
    ///
    /// After post_install renames the extracted `jdk-{version}+{build}` to `jdk`,
    /// the JDK root is simply `jdk/` (or `jdk/Contents/Home/` on macOS).
    fn jdk_home(platform: Platform) -> PathBuf {
        match platform {
            Platform::MacOS => PathBuf::from("jdk").join("Contents").join("Home"),
            _ => PathBuf::from("jdk"),
        }
    }
}

impl Provider for JavaProvider {
    fn name(&self) -> &str {
        "java"
    }

    fn resolve_version(
        &self,
        spec: &VersionSpec,
        _target: &Target,
    ) -> Result<semver::Version, ProviderError> {
        let candidates = Self::fetch_versions()?;
        resolve_from_candidates(&candidates, spec, "java")
    }

    /// Download URL uses the Adoptium `/latest/{major}/ga/` endpoint.
    ///
    /// This endpoint resolves to the latest GA build for a given major version,
    /// handling build metadata internally. No need to carry the `+build` suffix.
    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let os = Self::adoptium_os(target.platform);
        let arch = Self::adoptium_arch(target.arch);
        Ok(format!(
            "https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse?project=jdk",
            major = version.major
        ))
    }

    fn archive_format(&self, target: &Target) -> ArchiveFormat {
        target.platform.default_archive_format()
    }

    fn bin_paths(&self, _version: &semver::Version, target: &Target) -> Vec<PathBuf> {
        vec![Self::jdk_home(target.platform).join("bin")]
    }

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        // JAVA_HOME must point to the JDK root (the directory containing bin/, lib/).
        // On macOS this is jdk/Contents/Home, on Linux/Windows it's just jdk/.
        // The post_install_command renames the extracted directory to just "jdk".
        let platform = crate::platform::Platform::detect().unwrap_or(Platform::Linux);
        let jdk_root = install_dir.join(Self::jdk_home(platform));
        HashMap::from([(
            "JAVA_HOME".to_string(),
            jdk_root.to_string_lossy().to_string(),
        )])
    }

    /// Rename the extracted `jdk-{version}+{build}` directory to just `jdk`
    /// so that bin_paths and JAVA_HOME work regardless of the build metadata.
    fn post_install_command(
        &self,
        _version: &semver::Version,
        _target: &Target,
        _install_dir: &std::path::Path,
    ) -> Option<String> {
        // Find the jdk-* directory and rename it to jdk
        Some("for d in jdk-*; do [ -d \"$d\" ] && mv \"$d\" jdk && break; done".to_string())
    }

    /// Override to return all versions in a single pass instead of the
    /// default O(major * minor) loop.
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
        assert_eq!(JavaProvider.name(), "java");
    }

    #[test]
    fn test_archive_format_unix() {
        assert_eq!(
            JavaProvider.archive_format(&macos_arm64()),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            JavaProvider.archive_format(&linux_x64()),
            ArchiveFormat::TarGz
        );
    }

    #[test]
    fn test_archive_format_windows() {
        assert_eq!(
            JavaProvider.archive_format(&windows_x64()),
            ArchiveFormat::Zip
        );
    }

    #[test]
    fn test_bin_paths_macos() {
        let paths = JavaProvider.bin_paths(&v("21.0.2"), &macos_arm64());
        assert_eq!(paths, vec![PathBuf::from("jdk/Contents/Home/bin")]);
    }

    #[test]
    fn test_bin_paths_linux() {
        let paths = JavaProvider.bin_paths(&v("21.0.2"), &linux_x64());
        assert_eq!(paths, vec![PathBuf::from("jdk/bin")]);
    }

    #[test]
    fn test_bin_paths_windows() {
        let paths = JavaProvider.bin_paths(&v("21.0.2"), &windows_x64());
        assert_eq!(paths, vec![PathBuf::from("jdk/bin")]);
    }

    #[test]
    fn test_env_vars() {
        let vars = JavaProvider.env_vars(Path::new("/cache/java/21.0.2"));
        let java_home = vars.get("JAVA_HOME").unwrap();
        // On macOS: /cache/java/21.0.2/jdk/Contents/Home
        // On Linux: /cache/java/21.0.2/jdk
        assert!(java_home.starts_with("/cache/java/21.0.2/jdk"));
    }

    #[test]
    fn test_jdk_home_macos() {
        let home = JavaProvider::jdk_home(Platform::MacOS);
        assert_eq!(home, PathBuf::from("jdk/Contents/Home"));
    }

    #[test]
    fn test_jdk_home_linux() {
        let home = JavaProvider::jdk_home(Platform::Linux);
        assert_eq!(home, PathBuf::from("jdk"));
    }

    #[test]
    fn test_post_install_renames_jdk_dir() {
        let cmd = JavaProvider
            .post_install_command(&v("21.0.2"), &linux_x64(), Path::new("/cache"))
            .unwrap();
        assert!(cmd.contains("mv"));
        assert!(cmd.contains("jdk"));
    }

    #[test]
    fn test_adoptium_os() {
        assert_eq!(JavaProvider::adoptium_os(Platform::MacOS), "mac");
        assert_eq!(JavaProvider::adoptium_os(Platform::Linux), "linux");
        assert_eq!(JavaProvider::adoptium_os(Platform::Windows), "windows");
    }

    #[test]
    fn test_adoptium_arch() {
        assert_eq!(JavaProvider::adoptium_arch(Arch::X86_64), "x64");
        assert_eq!(JavaProvider::adoptium_arch(Arch::Aarch64), "aarch64");
    }

    #[test]
    fn test_download_url_uses_latest_endpoint() {
        let url = JavaProvider
            .download_url(&v("21.0.2"), &linux_x64())
            .unwrap();
        // Uses /latest/21/ga/ endpoint instead of /version/jdk-21.0.2/
        assert!(url.contains("/latest/21/ga/"));
        assert!(url.contains("linux"));
        assert!(url.contains("x64"));
    }

    #[test]
    fn test_download_url_macos_arm64() {
        let url = JavaProvider
            .download_url(&v("21.0.2"), &macos_arm64())
            .unwrap();
        assert!(url.contains("/latest/21/ga/"));
        assert!(url.contains("mac"));
        assert!(url.contains("aarch64"));
    }

    #[test]
    fn test_download_url_windows() {
        let url = JavaProvider
            .download_url(&v("21.0.2"), &windows_x64())
            .unwrap();
        assert!(url.contains("windows"));
    }

    #[test]
    fn test_parse_version_response() {
        let json = r#"{"versions": [{"major": 21, "minor": 0, "security": 2, "semver": "21.0.2+13.0.LTS"}]}"#;
        let versions = JavaProvider::parse_version_response(json).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], v("21.0.2"));
    }

    #[test]
    fn test_parse_version_response_fallback_to_major_minor_security() {
        // When semver field can't be parsed, fall back to major.minor.security
        let json =
            r#"{"versions": [{"major": 17, "minor": 0, "security": 9, "semver": "unparseable"}]}"#;
        let versions = JavaProvider::parse_version_response(json).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], v("17.0.9"));
    }

    #[test]
    fn test_parse_version_response_invalid_json() {
        assert!(JavaProvider::parse_version_response("not json").is_err());
    }

    #[test]
    fn test_parse_available_invalid_json() {
        assert!(JavaProvider::parse_available("not json").is_err());
    }

    #[test]
    fn test_parse_available_empty_releases() {
        let json = r#"{"available_releases": []}"#;
        assert!(JavaProvider::parse_available(json).is_err());
    }

    #[test]
    fn test_get_provider_java() {
        assert_eq!(super::super::get_provider("java").unwrap().name(), "java");
    }

    #[test]
    fn test_get_provider_jdk_alias() {
        assert_eq!(super::super::get_provider("jdk").unwrap().name(), "java");
    }
}

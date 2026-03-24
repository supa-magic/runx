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
pub struct JavaProvider;

impl JavaProvider {
    const AVAILABLE_URL: &str = "https://api.adoptium.net/v3/info/available_releases";

    fn fetch_versions() -> Result<Vec<semver::Version>, ProviderError> {
        let body = fetch_json(Self::AVAILABLE_URL, "java")?;
        Self::parse_available(&body)
    }

    /// Parse Adoptium available releases into semver versions.
    ///
    /// The Adoptium API returns major version numbers. We resolve each to
    /// the latest GA release to get the full version.
    fn parse_available(json: &str) -> Result<Vec<semver::Version>, ProviderError> {
        let info: AdoptiumAvailableReleases =
            serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
                tool: "java".to_string(),
                reason: format!("failed to parse available releases: {e}"),
            })?;

        let mut versions = Vec::new();
        for major in &info.available_releases {
            // Fetch the latest GA release for each major version
            let url = format!(
                "https://api.adoptium.net/v3/info/release_versions?architecture=x64&heap_size=normal&image_type=jdk&os=linux&page=0&page_size=1&project=jdk&release_type=ga&sort_method=DEFAULT&sort_order=DESC&vendor=eclipse&version=%5B{major}%2C{}%29",
                major + 1
            );
            if let Ok(body) = fetch_json(&url, "java")
                && let Ok(parsed) = Self::parse_version_response(&body)
            {
                versions.extend(parsed);
            }
        }

        if versions.is_empty() {
            return Err(ProviderError::ResolutionFailed {
                tool: "java".to_string(),
                reason: "no Java versions found".to_string(),
            });
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
                let clean = v.semver.split('+').next().unwrap_or(&v.semver);
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

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        let os = Self::adoptium_os(target.platform);
        let arch = Self::adoptium_arch(target.arch);
        Ok(format!(
            "https://api.adoptium.net/v3/binary/version/jdk-{version}/\
             {os}/{arch}/jdk/hotspot/normal/eclipse?project=jdk"
        ))
    }

    fn archive_format(&self, target: &Target) -> ArchiveFormat {
        target.platform.default_archive_format()
    }

    fn bin_paths(&self, version: &semver::Version, target: &Target) -> Vec<PathBuf> {
        let dir_name = match target.platform {
            Platform::MacOS => format!("jdk-{version}/Contents/Home/bin"),
            _ => format!("jdk-{version}/bin"),
        };
        vec![PathBuf::from(dir_name)]
    }

    fn env_vars(&self, install_dir: &Path) -> HashMap<String, String> {
        // JAVA_HOME is set to the jdk directory; we'll use a glob-like approach
        // since the exact directory name varies. The caller sets JAVA_HOME
        // based on the first entry in bin_paths' parent.
        HashMap::from([(
            "JAVA_HOME".to_string(),
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
        assert_eq!(paths, vec![PathBuf::from("jdk-21.0.2/Contents/Home/bin")]);
    }

    #[test]
    fn test_bin_paths_linux() {
        let paths = JavaProvider.bin_paths(&v("21.0.2"), &linux_x64());
        assert_eq!(paths, vec![PathBuf::from("jdk-21.0.2/bin")]);
    }

    #[test]
    fn test_env_vars() {
        let vars = JavaProvider.env_vars(Path::new("/cache/java/21.0.2"));
        assert_eq!(vars.get("JAVA_HOME").unwrap(), "/cache/java/21.0.2");
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
    fn test_download_url_contains_version() {
        let url = JavaProvider
            .download_url(&v("21.0.2"), &linux_x64())
            .unwrap();
        assert!(url.contains("jdk-21.0.2"));
        assert!(url.contains("linux"));
        assert!(url.contains("x64"));
    }

    #[test]
    fn test_parse_version_response() {
        let json = r#"{"versions": [{"major": 21, "minor": 0, "security": 2, "semver": "21.0.2+13.0.LTS"}]}"#;
        let versions = JavaProvider::parse_version_response(json).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], v("21.0.2"));
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

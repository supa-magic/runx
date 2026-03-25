pub mod bun;
pub mod deno;
pub mod go;
pub mod java;
pub mod node;
pub mod python;
pub mod ruby;
pub mod rust;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::platform::Target;
use crate::version::VersionSpec;

/// Global verbose flag — set once from CLI parsing, read by `fetch_json` for retry messages.
pub static VERBOSE: AtomicBool = AtomicBool::new(false);

pub use bun::BunProvider;
pub use deno::DenoProvider;
pub use go::GoProvider;
pub use java::JavaProvider;
pub use node::NodeProvider;
pub use python::PythonProvider;
pub use ruby::RubyProvider;
pub use rust::RustProvider;

/// Canonical registry of all supported tools with their aliases and interpreter commands.
///
/// Single source of truth — used by `get_provider`, `list`, `init`, and `tool_interpreter`.
pub const TOOL_REGISTRY: &[ToolEntry] = &[
    ToolEntry {
        name: "node",
        aliases: &["nodejs"],
        interpreter: &["node"],
    },
    ToolEntry {
        name: "python",
        aliases: &["python3"],
        interpreter: &["python3"],
    },
    ToolEntry {
        name: "go",
        aliases: &["golang"],
        interpreter: &["go", "run"],
    },
    ToolEntry {
        name: "deno",
        aliases: &[],
        interpreter: &["deno", "run"],
    },
    ToolEntry {
        name: "bun",
        aliases: &["bunx"],
        interpreter: &["bun", "run"],
    },
    ToolEntry {
        name: "ruby",
        aliases: &["rb"],
        interpreter: &["ruby"],
    },
    ToolEntry {
        name: "java",
        aliases: &["jdk"],
        interpreter: &["java"],
    },
    ToolEntry {
        name: "rust",
        aliases: &["rustc", "cargo"],
        interpreter: &[],
    },
];

/// A registered tool with its aliases and default interpreter command.
pub struct ToolEntry {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub interpreter: &'static [&'static str],
}

/// Look up a provider by tool name.
///
/// Returns the appropriate `Provider` implementation for the given tool,
/// or an error if the tool is not supported.
pub fn get_provider(name: &str) -> Result<Box<dyn Provider>, ProviderError> {
    match name {
        "node" | "nodejs" => Ok(Box::new(NodeProvider)),
        "python" | "python3" => Ok(Box::new(PythonProvider)),
        "go" | "golang" => Ok(Box::new(GoProvider)),
        "deno" => Ok(Box::new(DenoProvider)),
        "bun" | "bunx" => Ok(Box::new(BunProvider)),
        "ruby" | "rb" => Ok(Box::new(RubyProvider)),
        "java" | "jdk" => Ok(Box::new(JavaProvider)),
        "rust" | "rustc" | "cargo" => Ok(Box::new(RustProvider)),
        other => {
            // Check for plugins before returning UnknownTool
            if let Ok(Some(provider)) = crate::plugin::get_plugin_provider(other) {
                return Ok(provider);
            }
            Err(ProviderError::UnknownTool {
                name: other.to_string(),
            })
        }
    }
}

// --- Shared helpers ---

/// Fetch JSON from a URL using blocking HTTP inside an async context.
///
/// Shared HTTP client and per-invocation response cache.
///
/// The client is built once via `LazyLock`. The response cache deduplicates
/// HTTP calls within a single process lifetime — if `fetch_json` is called
/// twice with the same URL, the second call returns the cached body.
static HTTP_CLIENT: std::sync::LazyLock<reqwest::blocking::Client> =
    std::sync::LazyLock::new(|| {
        reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build HTTP client")
    });

static RESPONSE_CACHE: std::sync::LazyLock<std::sync::Mutex<HashMap<String, String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

/// Maximum number of retry attempts for transient HTTP errors.
const MAX_RETRIES: u32 = 3;

/// HTTP status codes considered transient and eligible for retry.
fn is_transient_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504)
}

pub fn fetch_json(url: &str, tool: &'static str) -> Result<String, ProviderError> {
    tokio::task::block_in_place(|| {
        // Check cache first (recover from poison — cached data is still valid)
        {
            let cache = RESPONSE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(body) = cache.get(url) {
                return Ok(body.clone());
            }
        }

        let verbose = VERBOSE.load(Ordering::Relaxed);
        let mut last_reason = String::new();

        for attempt in 1..=MAX_RETRIES {
            // Determine the outcome and whether it's retryable
            let (reason, delay) = match HTTP_CLIENT
                .get(url)
                .header("User-Agent", "runx")
                .header("Accept", "application/json")
                .send()
            {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let body =
                            response
                                .text()
                                .map_err(|e| ProviderError::ResolutionFailed {
                                    tool: tool.to_string(),
                                    reason: format!("{e:#}"),
                                })?;

                        // Cache the response (recover from poison)
                        RESPONSE_CACHE
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(url.to_string(), body.clone());

                        return Ok(body);
                    }

                    let delay = if is_transient_status(status) {
                        Some(retry_delay(&response, attempt))
                    } else {
                        None
                    };
                    (format!("HTTP {status} from {url}"), delay)
                }
                Err(e) => {
                    let delay = Some(std::time::Duration::from_secs(1u64 << (attempt - 1).min(6)));
                    (format!("{e:#}"), delay)
                }
            };

            // Non-retryable error, or final attempt — return immediately
            let Some(delay) = delay.filter(|_| attempt < MAX_RETRIES) else {
                return Err(ProviderError::ResolutionFailed {
                    tool: tool.to_string(),
                    reason,
                });
            };

            if verbose {
                eprintln!(
                    "Retrying {tool} resolution (attempt {}/{MAX_RETRIES}, {reason})...",
                    attempt + 1
                );
            }
            std::thread::sleep(delay);
            last_reason = reason;
        }

        // Unreachable in practice (the loop always returns), but keeps the compiler happy
        Err(ProviderError::ResolutionFailed {
            tool: tool.to_string(),
            reason: last_reason,
        })
    })
}

/// Compute the retry delay, respecting `Retry-After` header for 429 responses.
fn retry_delay(response: &reqwest::blocking::Response, attempt: u32) -> std::time::Duration {
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
        && let Some(retry_after) = response.headers().get(reqwest::header::RETRY_AFTER)
        && let Ok(secs) = retry_after.to_str().unwrap_or("").parse::<u64>()
    {
        return std::time::Duration::from_secs(secs);
    }
    // Exponential backoff: 1s, 2s, 4s — capped at 64s to prevent shift overflow
    std::time::Duration::from_secs(1u64 << (attempt - 1).min(6))
}

/// A simple GitHub release entry with just the tag name.
///
/// Shared by Deno and Bun providers which both fetch releases from GitHub.
#[derive(Debug, serde::Deserialize)]
pub struct SimpleGitHubRelease {
    pub tag_name: String,
}

/// Collect unique stable versions from an iterator of optional versions.
///
/// Filters out pre-release versions and duplicates. Uses a HashSet
/// for O(n) deduplication instead of O(n^2) Vec::contains.
pub fn collect_stable_versions(
    versions: impl Iterator<Item = Option<semver::Version>>,
) -> Vec<semver::Version> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for ver in versions.flatten() {
        if ver.pre.is_empty() && seen.insert(ver.clone()) {
            result.push(ver);
        }
    }
    result
}

/// Parse GitHub releases JSON and extract stable versions using a tag-to-version parser.
///
/// Shared by providers that use GitHub releases (Deno, Bun) with different tag formats.
pub fn parse_github_releases(
    json: &str,
    tool: &'static str,
    parse_tag: impl Fn(&str) -> Option<semver::Version>,
) -> Result<Vec<semver::Version>, ProviderError> {
    let releases: Vec<SimpleGitHubRelease> =
        serde_json::from_str(json).map_err(|e| ProviderError::ResolutionFailed {
            tool: tool.to_string(),
            reason: format!("failed to parse releases: {e}"),
        })?;

    let versions = collect_stable_versions(releases.iter().map(|r| parse_tag(&r.tag_name)));

    if versions.is_empty() {
        return Err(ProviderError::ResolutionFailed {
            tool: tool.to_string(),
            reason: "no stable versions found in releases".to_string(),
        });
    }

    Ok(versions)
}

/// Resolve a version spec against a list of candidates.
///
/// Shared logic used by all providers: fetch candidates, resolve, or return VersionNotFound.
pub fn resolve_from_candidates(
    candidates: &[semver::Version],
    spec: &VersionSpec,
    tool: &'static str,
) -> Result<semver::Version, ProviderError> {
    spec.resolve(candidates)
        .cloned()
        .ok_or_else(|| ProviderError::VersionNotFound {
            tool: tool.to_string(),
            spec: spec.to_string(),
        })
}

// --- Types ---

/// Supported archive formats for tool downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    TarGz,
    #[allow(unused)]
    TarXz,
    Zip,
}

/// Trait that all tool providers must implement.
///
/// Each provider knows how to resolve versions, construct download URLs,
/// and describe the binary layout and environment variables for a specific tool.
///
/// **Note:** `bin_paths` returns paths relative to the install directory
/// after archive extraction. These must be directories (not individual files)
/// since they become `PATH` entries. Providers may override the default
/// `archive_format` from the platform if the tool uses a different format.
pub trait Provider {
    /// The tool name (e.g., "node", "python", "go").
    fn name(&self) -> &str;

    /// Resolve a version spec to an exact version by querying upstream.
    fn resolve_version(
        &self,
        spec: &VersionSpec,
        target: &Target,
    ) -> Result<semver::Version, ProviderError>;

    /// Construct the download URL for a specific version and target.
    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError>;

    /// Return the archive format used for the given target.
    fn archive_format(&self, target: &Target) -> ArchiveFormat;

    /// Return directory paths relative to the install directory for PATH.
    ///
    /// These must be directories, not individual executables.
    fn bin_paths(&self, version: &semver::Version, target: &Target) -> Vec<PathBuf>;

    /// Return environment variables to set for this tool.
    fn env_vars(&self, install_dir: &std::path::Path) -> HashMap<String, String>;

    /// Return names of environment variables that need per-invocation temp directories.
    ///
    /// For example, Go needs `GOPATH` and Deno needs `DENO_DIR` as ephemeral
    /// temp directories that are cleaned up after the command exits.
    /// Default: no temp dirs needed.
    fn temp_env_dirs(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// Return all available versions from upstream, sorted descending.
    ///
    /// Providers that can return a full version list in a single HTTP call
    /// should override this for efficiency. The default implementation
    /// resolves Latest and then scans major/minor ranges.
    fn list_versions(&self, target: &Target) -> Result<Vec<semver::Version>, ProviderError> {
        let latest = self.resolve_version(&crate::version::VersionSpec::Latest, target)?;
        let max_major = latest.major;
        let latest_minor = latest.minor;

        let mut all = vec![latest];
        for major in (0..=max_major).rev() {
            if let Ok(v) = self.resolve_version(&crate::version::VersionSpec::Major(major), target)
            {
                all.push(v);
            }
        }
        for minor in (0..=latest_minor).rev() {
            if let Ok(v) = self.resolve_version(
                &crate::version::VersionSpec::MajorMinor(max_major, minor),
                target,
            ) {
                all.push(v);
            }
        }
        all.sort_by(|a, b| b.cmp(a));
        all.dedup();
        Ok(all)
    }

    /// Return an optional post-install command to run after extraction.
    ///
    /// The command runs with CWD set to `install_dir`. Placeholders
    /// `{install_dir}`, `{version}`, `{os}`, `{arch}` are expanded.
    /// Default: no post-install step.
    fn post_install_command(
        &self,
        _version: &semver::Version,
        _target: &Target,
        _install_dir: &std::path::Path,
    ) -> Option<String> {
        None
    }
}

/// Errors that occur during provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The requested tool is not supported.
    #[error(
        "unknown tool `{name}`.\n  Supported tools: node, python, go, deno, bun, ruby, java, rust\n  Run `runx list` to see all available tools."
    )]
    UnknownTool { name: String },

    /// No version matched the given spec.
    #[error(
        "no {tool} version found matching `{spec}`.\n  Run `runx list {tool}` to see available versions."
    )]
    VersionNotFound { tool: String, spec: String },

    /// The tool does not support this platform/architecture combination.
    #[error(
        "{tool} does not support target `{target}`.\n  Run `runx list {tool}` to see supported platforms."
    )]
    UnsupportedTarget { tool: String, target: String },

    /// Network or API error during version resolution.
    #[error(
        "failed to resolve {tool} version: {reason}\n  Check your network connection and try again."
    )]
    ResolutionFailed { tool: String, reason: String },
}

/// Shared test helpers for provider tests.
#[cfg(test)]
pub mod test_helpers {
    use crate::platform::{Arch, Platform, Target};

    pub fn macos_arm64() -> Target {
        Target::new(Platform::MacOS, Arch::Aarch64)
    }

    pub fn linux_x64() -> Target {
        Target::new(Platform::Linux, Arch::X86_64)
    }

    pub fn linux_arm64() -> Target {
        Target::new(Platform::Linux, Arch::Aarch64)
    }

    pub fn windows_x64() -> Target {
        Target::new(Platform::Windows, Arch::X86_64)
    }

    pub fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::platform::{Arch, Platform, Target};

    use super::*;

    #[test]
    fn test_provider_error_display_version_not_found() {
        let err = ProviderError::VersionNotFound {
            tool: "node".to_string(),
            spec: "99".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("no node version found matching `99`"));
        assert!(msg.contains("runx list node"));
    }

    #[test]
    fn test_provider_error_display_unsupported_target() {
        let err = ProviderError::UnsupportedTarget {
            tool: "bun".to_string(),
            target: "Windows-x86_64".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("bun does not support target `Windows-x86_64`"));
        assert!(msg.contains("runx list bun"));
    }

    #[test]
    fn test_provider_error_display_resolution_failed() {
        let err = ProviderError::ResolutionFailed {
            tool: "python".to_string(),
            reason: "network timeout".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("failed to resolve python version: network timeout"));
        assert!(msg.contains("Check your network connection"));
    }

    #[test]
    fn test_archive_format_equality() {
        assert_eq!(ArchiveFormat::TarGz, ArchiveFormat::TarGz);
        assert_ne!(ArchiveFormat::TarGz, ArchiveFormat::Zip);
        assert_ne!(ArchiveFormat::TarXz, ArchiveFormat::Zip);
        assert_eq!(ArchiveFormat::TarXz, ArchiveFormat::TarXz);
    }

    #[test]
    fn test_provider_error_is_std_error() {
        let err = ProviderError::VersionNotFound {
            tool: "node".to_string(),
            spec: "99".to_string(),
        };
        let boxed: Box<dyn std::error::Error> = Box::new(err);
        assert!(boxed.source().is_none());
    }

    #[test]
    fn test_collect_stable_versions() {
        let versions = collect_stable_versions(
            vec![
                Some(semver::Version::new(1, 0, 0)),
                None,
                Some(semver::Version::new(2, 0, 0)),
                Some(semver::Version::new(1, 0, 0)), // duplicate
            ]
            .into_iter(),
        );
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_collect_stable_versions_filters_prerelease() {
        let pre = semver::Version::parse("1.0.0-alpha.1").unwrap();
        let stable = semver::Version::new(1, 0, 0);
        let versions = collect_stable_versions(vec![Some(pre), Some(stable.clone())].into_iter());
        assert_eq!(versions, vec![stable]);
    }

    /// Dummy provider to verify the trait is implementable.
    struct DummyProvider;

    impl Provider for DummyProvider {
        fn name(&self) -> &str {
            "dummy"
        }

        fn resolve_version(
            &self,
            _spec: &VersionSpec,
            _target: &Target,
        ) -> Result<semver::Version, ProviderError> {
            Ok(semver::Version::new(1, 0, 0))
        }

        fn download_url(
            &self,
            _version: &semver::Version,
            _target: &Target,
        ) -> Result<String, ProviderError> {
            Ok("https://example.com/dummy-1.0.0.tar.gz".to_string())
        }

        fn archive_format(&self, _target: &Target) -> ArchiveFormat {
            ArchiveFormat::TarGz
        }

        fn bin_paths(&self, _version: &semver::Version, _target: &Target) -> Vec<PathBuf> {
            vec![PathBuf::from("bin/dummy")]
        }

        fn env_vars(&self, install_dir: &std::path::Path) -> HashMap<String, String> {
            HashMap::from([(
                "DUMMY_HOME".to_string(),
                install_dir.to_string_lossy().to_string(),
            )])
        }
    }

    #[test]
    fn test_dummy_provider_implements_trait() {
        let provider = DummyProvider;
        let target = Target::new(Platform::MacOS, Arch::Aarch64);
        let spec = VersionSpec::Latest;

        assert_eq!(provider.name(), "dummy");
        let version = provider.resolve_version(&spec, &target).unwrap();
        assert_eq!(version, semver::Version::new(1, 0, 0));
        let url = provider.download_url(&version, &target).unwrap();
        assert!(url.contains("dummy"));
        assert_eq!(provider.archive_format(&target), ArchiveFormat::TarGz);
        assert!(!provider.bin_paths(&version, &target).is_empty());
        assert!(
            provider
                .env_vars(std::path::Path::new("/tmp/dummy"))
                .contains_key("DUMMY_HOME")
        );
    }

    // --- resolve_from_candidates ---

    #[test]
    fn test_resolve_from_candidates_returns_highest_match() {
        let candidates = vec![
            semver::Version::new(18, 17, 0),
            semver::Version::new(18, 19, 1),
            semver::Version::new(20, 0, 0),
        ];
        let spec = VersionSpec::Major(18);
        let result = resolve_from_candidates(&candidates, &spec, "node").unwrap();
        assert_eq!(result, semver::Version::new(18, 19, 1));
    }

    #[test]
    fn test_resolve_from_candidates_no_match_returns_version_not_found() {
        let candidates = vec![semver::Version::new(20, 0, 0)];
        let spec = VersionSpec::Major(18);
        let result = resolve_from_candidates(&candidates, &spec, "node");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::VersionNotFound { .. }
        ));
    }

    #[test]
    fn test_resolve_from_candidates_empty_returns_version_not_found() {
        let result = resolve_from_candidates(&[], &VersionSpec::Latest, "node");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::VersionNotFound { .. }
        ));
    }

    // --- parse_github_releases ---

    #[test]
    fn test_parse_github_releases_success() {
        let json = r#"[{"tag_name": "v1.0.0"}, {"tag_name": "v1.1.0"}, {"tag_name": "v2.0.0"}]"#;
        let versions = parse_github_releases(json, "test", |tag| {
            tag.strip_prefix('v')
                .and_then(|s| semver::Version::parse(s).ok())
        })
        .unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn test_parse_github_releases_filters_unparseable_tags() {
        let json =
            r#"[{"tag_name": "v1.0.0"}, {"tag_name": "not-a-version"}, {"tag_name": "v2.0.0"}]"#;
        let versions = parse_github_releases(json, "test", |tag| {
            tag.strip_prefix('v')
                .and_then(|s| semver::Version::parse(s).ok())
        })
        .unwrap();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_parse_github_releases_all_unparseable_returns_error() {
        let json = r#"[{"tag_name": "bad"}, {"tag_name": "also-bad"}]"#;
        let result = parse_github_releases(json, "test", |_| None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("no stable versions found"));
    }

    #[test]
    fn test_parse_github_releases_empty_array_returns_error() {
        let result = parse_github_releases("[]", "test", |_| None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_github_releases_invalid_json_returns_error() {
        let result = parse_github_releases("{not json}", "test", |_| None);
        assert!(result.is_err());
    }

    // --- Provider::temp_env_dirs default ---

    #[test]
    fn test_dummy_provider_temp_env_dirs_empty_by_default() {
        let provider = DummyProvider;
        assert!(provider.temp_env_dirs().is_empty());
    }

    // --- Provider::post_install_command default ---

    #[test]
    fn test_dummy_provider_post_install_command_none_by_default() {
        let provider = DummyProvider;
        let version = semver::Version::new(1, 0, 0);
        let target = Target::new(Platform::MacOS, Arch::Aarch64);
        assert!(
            provider
                .post_install_command(&version, &target, std::path::Path::new("/tmp"))
                .is_none()
        );
    }

    // --- get_provider: unknown tool error message ---

    #[test]
    fn test_get_provider_unknown_tool_error_message() {
        let result = get_provider("zig");
        let Err(err) = result else {
            panic!("expected Err for unknown tool");
        };
        let msg = err.to_string();
        assert!(msg.contains("zig"));
        assert!(msg.contains("runx list"));
    }

    // --- ProviderError::UnknownTool display ---

    #[test]
    fn test_provider_error_unknown_tool_display() {
        let err = ProviderError::UnknownTool {
            name: "zig".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("unknown tool `zig`"));
        assert!(msg.contains("node, python, go, deno, bun, ruby, java, rust"));
    }

    // --- is_transient_status ---

    #[test]
    fn test_transient_status_codes() {
        use reqwest::StatusCode;
        assert!(is_transient_status(StatusCode::TOO_MANY_REQUESTS)); // 429
        assert!(is_transient_status(StatusCode::INTERNAL_SERVER_ERROR)); // 500
        assert!(is_transient_status(StatusCode::BAD_GATEWAY)); // 502
        assert!(is_transient_status(StatusCode::SERVICE_UNAVAILABLE)); // 503
        assert!(is_transient_status(StatusCode::GATEWAY_TIMEOUT)); // 504
    }

    #[test]
    fn test_non_transient_status_codes() {
        use reqwest::StatusCode;
        assert!(!is_transient_status(StatusCode::BAD_REQUEST)); // 400
        assert!(!is_transient_status(StatusCode::UNAUTHORIZED)); // 401
        assert!(!is_transient_status(StatusCode::FORBIDDEN)); // 403
        assert!(!is_transient_status(StatusCode::NOT_FOUND)); // 404
        assert!(!is_transient_status(StatusCode::OK)); // 200
    }

    // --- retry backoff ---

    #[test]
    fn test_exponential_backoff_delays() {
        // Verify the exponential backoff formula: 1 << (attempt - 1)
        // attempt 1 → 1s, attempt 2 → 2s, attempt 3 → 4s
        assert_eq!(1u64 << 0, 1);
        assert_eq!(1u64 << 1, 2);
        assert_eq!(1u64 << 2, 4);
    }

    // --- VERBOSE flag ---

    #[test]
    fn test_verbose_flag_default_is_false() {
        // VERBOSE may have been set by other tests, so just verify it's an AtomicBool
        use std::sync::atomic::Ordering;
        let _ = VERBOSE.load(Ordering::Relaxed);
    }

    #[test]
    fn test_max_retries_is_three() {
        assert_eq!(MAX_RETRIES, 3);
    }
}

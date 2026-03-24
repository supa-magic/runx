use crate::cache::Cache;
use crate::cli::ToolSpec;
use crate::error::RunxError;
use crate::platform::Target;
use crate::provider::{self, Provider, ProviderError};
use crate::version::VersionSpec;

/// All supported tool names and their aliases.
const SUPPORTED_TOOLS: &[(&str, &[&str])] = &[
    ("node", &["nodejs"]),
    ("python", &["python3"]),
    ("go", &["golang"]),
    ("deno", &[]),
    ("bun", &["bunx"]),
];

/// Execute the `runx list` subcommand.
pub async fn run(cached: bool, tool: Option<ToolSpec>) -> Result<(), RunxError> {
    if cached {
        list_cached(tool.as_ref())?;
    } else if let Some(ref spec) = tool {
        list_upstream(spec).await?;
    } else {
        list_providers()?;
    }
    Ok(())
}

/// Show all supported tool providers with cache status.
fn list_providers() -> Result<(), RunxError> {
    let cache = Cache::new()?;
    let cached_tools = cache.list_cached()?;

    println!("Supported tools:");
    println!();
    println!("  {:<12} {:<16} Cached", "Tool", "Aliases");
    println!("  {:<12} {:<16} ──────", "────", "───────");

    for (name, aliases) in SUPPORTED_TOOLS {
        let alias_str = if aliases.is_empty() {
            "—".to_string()
        } else {
            aliases.join(", ")
        };

        let cached_info = cached_tools
            .iter()
            .find(|t| t.name == *name)
            .map(|t| {
                let count = t.versions.len();
                let size = format_size(t.size_bytes);
                format!(
                    "{count} version{} ({size})",
                    if count == 1 { "" } else { "s" }
                )
            })
            .unwrap_or_else(|| "—".to_string());

        println!("  {:<12} {:<16} {}", name, alias_str, cached_info);
    }

    println!();
    println!("Cache directory: {}", cache.root().display());
    Ok(())
}

/// Show cached tool versions with disk sizes.
fn list_cached(filter: Option<&ToolSpec>) -> Result<(), RunxError> {
    let cache = Cache::new()?;
    let cached_tools = cache.list_cached()?;

    let tools: Vec<_> = if let Some(spec) = filter {
        cached_tools
            .into_iter()
            .filter(|t| t.name == spec.name)
            .collect()
    } else {
        cached_tools
    };

    if tools.is_empty() {
        if let Some(spec) = filter {
            println!("No cached versions of {}.", spec.name);
        } else {
            println!("No cached tools. Run a command with --with to download tools.");
        }
        return Ok(());
    }

    let tool_count = tools.len();
    let mut total_size = 0u64;

    for mut tool in tools {
        let size = format_size(tool.size_bytes);
        total_size += tool.size_bytes;

        println!("{} ({size})", tool.name);

        tool.versions.sort_by(|a, b| b.cmp(a));
        for version in &tool.versions {
            println!("  {version}");
        }
    }

    println!();
    println!(
        "Total: {} tool{}, {}",
        tool_count,
        if tool_count == 1 { "" } else { "s" },
        format_size(total_size)
    );
    println!("Cache directory: {}", cache.root().display());
    Ok(())
}

/// Query upstream for available versions of a specific tool.
async fn list_upstream(spec: &ToolSpec) -> Result<(), RunxError> {
    let provider = provider::get_provider(&spec.name)?;
    let target = Target::detect().map_err(RunxError::UnsupportedPlatform)?;

    println!("Fetching available {} versions...", provider.name());
    println!();

    let versions = fetch_available_versions(provider.as_ref(), &target)?;

    if versions.is_empty() {
        println!("No versions found for {}.", provider.name());
        return Ok(());
    }

    // Show latest 20 versions grouped by major
    let display_versions: Vec<_> = versions.iter().take(20).collect();

    println!(
        "Available {} versions (latest {}):",
        provider.name(),
        display_versions.len()
    );
    println!();

    let cache = Cache::new()?;
    for version in &display_versions {
        let cached = cache.is_cached(provider.name(), version, &target);
        let marker = if cached { " (cached)" } else { "" };
        println!("  {version}{marker}");
    }

    if versions.len() > 20 {
        println!("  ... and {} more", versions.len() - 20);
    }

    println!();
    println!(
        "Use: runx --with {}@<version> -- <command>",
        provider.name()
    );
    Ok(())
}

/// Fetch all available versions from a provider's upstream.
fn fetch_available_versions(
    provider: &dyn Provider,
    target: &Target,
) -> Result<Vec<semver::Version>, ProviderError> {
    // Resolve Latest to get the highest version, then scan major/minor versions
    // to discover what's available upstream.
    let latest = provider.resolve_version(&VersionSpec::Latest, target)?;
    let max_major = latest.major;
    let latest_minor = latest.minor;

    let mut all_versions = vec![latest];

    // Resolve the latest patch for each major version
    for major in (0..=max_major).rev() {
        if let Ok(version) = provider.resolve_version(&VersionSpec::Major(major), target) {
            all_versions.push(version);
        }
    }

    // Also resolve minor versions for the latest major
    for minor in (0..=latest_minor).rev() {
        if let Ok(version) =
            provider.resolve_version(&VersionSpec::MajorMinor(max_major, minor), target)
        {
            all_versions.push(version);
        }
    }

    // Sort descending and deduplicate
    all_versions.sort_by(|a, b| b.cmp(a));
    all_versions.dedup();

    Ok(all_versions)
}

/// Format a byte count as a human-readable size string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_supported_tools_complete() {
        // Verify all tools in SUPPORTED_TOOLS are recognized by get_provider
        for (name, aliases) in SUPPORTED_TOOLS {
            assert!(
                provider::get_provider(name).is_ok(),
                "provider not found for {name}"
            );
            for alias in *aliases {
                assert!(
                    provider::get_provider(alias).is_ok(),
                    "provider not found for alias {alias}"
                );
            }
        }
    }

    #[test]
    fn test_list_providers_runs() {
        // Just verify it doesn't panic with a valid cache
        let result = list_providers();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_cached_empty() {
        // With a temp cache, should report no cached tools
        let _cache = Cache::new().unwrap();
        // This just verifies the function doesn't panic
        let _ = list_cached(None);
    }

    #[test]
    fn test_list_cached_with_filter_no_match() {
        let spec = ToolSpec {
            name: "nonexistent".to_string(),
            version: None,
        };
        // Should report no cached versions without error
        let result = list_cached(Some(&spec));
        assert!(result.is_ok());
    }

    #[test]
    fn test_format_size_boundaries() {
        // Just below KB threshold
        assert_eq!(format_size(1023), "1023 B");
        // Just below MB threshold
        assert_eq!(format_size(1024 * 1024 - 1), "1024.0 KB");
        // Just below GB threshold
        assert_eq!(format_size(1024 * 1024 * 1024 - 1), "1024.0 MB");
    }

    #[tokio::test]
    async fn test_list_dispatch_providers() {
        // cached=false, tool=None should call list_providers
        let result = run(false, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_dispatch_cached() {
        // cached=true, tool=None should call list_cached
        let result = run(true, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_dispatch_cached_with_filter() {
        let spec = ToolSpec {
            name: "node".to_string(),
            version: None,
        };
        let result = run(true, Some(spec)).await;
        assert!(result.is_ok());
    }
}

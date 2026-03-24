use crate::cache::Cache;
use crate::cli::ToolSpec;
use crate::download::download_and_install;
use crate::error::RunxError;
use crate::platform::Target;
use crate::provider::{self, ProviderError};
use crate::version::VersionSpec;

/// Execute the `runx update` subcommand.
pub async fn run(tool: Option<ToolSpec>, dry_run: bool) -> Result<(), RunxError> {
    let cache = Cache::new()?;
    let target = Target::detect().map_err(RunxError::UnsupportedPlatform)?;
    let cached_tools = cache.list_cached()?;

    if cached_tools.is_empty() {
        println!("No cached tools to update.");
        return Ok(());
    }

    let tool_filter = tool.as_ref().map(|t| t.name.as_str());
    let mut updated = 0;
    let mut checked = 0;
    let mut updates_available = 0;

    for cached in &cached_tools {
        if let Some(filter) = tool_filter
            && cached.name != filter
        {
            continue;
        }

        // Check if this is a known provider
        let provider = match provider::get_provider(&cached.name) {
            Ok(p) => p,
            Err(_) => continue, // Skip unknown tools in cache
        };

        for version_str in &cached.versions {
            let current: semver::Version = match version_str.parse() {
                Ok(v) => v,
                Err(_) => continue, // Skip unparseable versions
            };

            // Resolve the same major.minor to find the latest patch
            let spec = VersionSpec::MajorMinor(current.major, current.minor);
            let latest = match provider.resolve_version(&spec, &target) {
                Ok(v) => v,
                Err(ProviderError::ResolutionFailed { .. }) => continue,
                Err(e) => return Err(e.into()),
            };

            checked += 1;

            if latest > current {
                if dry_run {
                    println!("  {} {current} → {latest} (update available)", cached.name);
                    updates_available += 1;
                } else {
                    eprint!("Updating {} {current} → {latest}...", cached.name);

                    let install_dir = cache.install_path(provider.name(), &latest, &target);
                    if !cache.is_cached(provider.name(), &latest, &target) {
                        let url = provider.download_url(&latest, &target)?;
                        let format = provider.archive_format(&target);
                        download_and_install(&url, &install_dir, format, None, true).await?;
                    }
                    eprintln!(" done");

                    println!("  {} {current} → {latest}", cached.name);
                    updated += 1;
                }
            }
        }
    }

    if dry_run {
        if checked > 0 && updates_available == 0 {
            println!("All cached tools are up to date.");
        }
    } else if updated == 0 {
        println!("All cached tools are up to date.");
    } else {
        println!(
            "\nUpdated {updated} tool{}. Old versions kept in cache.",
            if updated == 1 { "" } else { "s" }
        );
    }

    if let Some(ref spec) = tool
        && tool_filter.is_some()
        && !cached_tools.iter().any(|t| t.name == spec.name)
    {
        println!("No cached versions of {}.", spec.name);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_update_empty_cache() {
        // Should handle empty cache gracefully
        // (uses real ~/.runx/cache which may or may not be empty)
        // Just verify it doesn't panic
        let _ = run(
            Some(ToolSpec {
                name: "nonexistent-tool-xyz".to_string(),
                version: None,
            }),
            true,
        )
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_update_dry_run_no_tool_filter() {
        // dry_run=true, tool=None — should complete without error
        let result = run(None, true).await;
        assert!(
            result.is_ok(),
            "update dry-run with no filter should succeed: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_update_unknown_tool_filter_does_not_error() {
        // Filtering for a tool not in the cache should print a message and return Ok
        let result = run(
            Some(ToolSpec {
                name: "definitely-not-cached-abc".to_string(),
                version: None,
            }),
            false,
        )
        .await;
        assert!(
            result.is_ok(),
            "update with missing tool should not fail: {result:?}"
        );
    }
}

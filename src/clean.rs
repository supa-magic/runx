use std::io::{self, Write};

use crate::cache::Cache;
use crate::cli::{HumanDuration, ToolSpec};
use crate::error::RunxError;
use crate::list::format_size;

/// Execute the `runx clean` subcommand.
pub fn run(
    tool: Option<ToolSpec>,
    older_than: Option<HumanDuration>,
    yes: bool,
    dry_run: bool,
) -> Result<(), RunxError> {
    let cache = Cache::new()?;

    if let Some(ref duration) = older_than {
        clean_older_than(&cache, tool.as_ref(), duration, yes, dry_run)
    } else if let Some(ref spec) = tool {
        clean_tool(&cache, spec, yes, dry_run)
    } else {
        clean_all(&cache, yes, dry_run)
    }
}

/// Remove all cached tools.
fn clean_all(cache: &Cache, yes: bool, dry_run: bool) -> Result<(), RunxError> {
    let tools = cache.list_cached()?;
    if tools.is_empty() {
        println!("Nothing to clean. Cache is empty.");
        return Ok(());
    }

    let total_size: u64 = tools.iter().map(|t| t.size_bytes).sum();
    let total_versions: usize = tools.iter().map(|t| t.versions.len()).sum();

    println!(
        "Will remove {} tool{}, {} version{} ({})",
        tools.len(),
        if tools.len() == 1 { "" } else { "s" },
        total_versions,
        if total_versions == 1 { "" } else { "s" },
        format_size(total_size)
    );

    if dry_run {
        for tool in &tools {
            for version in &tool.versions {
                println!("  Would remove {}@{}", tool.name, version);
            }
        }
        return Ok(());
    }

    if !yes && !confirm("Remove all cached tools?")? {
        println!("Aborted.");
        return Ok(());
    }

    let freed = cache.clean_all()?;
    println!("Removed all cached tools. Freed {}.", format_size(freed));
    Ok(())
}

/// Remove all cached versions of a specific tool.
fn clean_tool(cache: &Cache, spec: &ToolSpec, yes: bool, dry_run: bool) -> Result<(), RunxError> {
    if spec.version.is_some() {
        eprintln!(
            "Warning: version specifier ignored. `clean --tool {}` removes all cached versions of {}.",
            spec, spec.name
        );
    }

    let tools = cache.list_cached()?;
    let tool = tools.iter().find(|t| t.name == spec.name);

    let Some(tool) = tool else {
        println!("No cached versions of {}.", spec.name);
        return Ok(());
    };

    println!(
        "Will remove {} ({} version{}, {})",
        tool.name,
        tool.versions.len(),
        if tool.versions.len() == 1 { "" } else { "s" },
        format_size(tool.size_bytes)
    );

    if dry_run {
        for version in &tool.versions {
            println!("  Would remove {}@{}", tool.name, version);
        }
        return Ok(());
    }

    if !yes && !confirm(&format!("Remove all cached versions of {}?", spec.name))? {
        println!("Aborted.");
        return Ok(());
    }

    let freed = cache.clean_tool(&spec.name)?;
    println!(
        "Removed all cached versions of {}. Freed {}.",
        spec.name,
        format_size(freed)
    );
    Ok(())
}

/// Remove cached versions older than a duration.
fn clean_older_than(
    cache: &Cache,
    tool: Option<&ToolSpec>,
    duration: &HumanDuration,
    yes: bool,
    dry_run: bool,
) -> Result<(), RunxError> {
    let tool_filter = tool.map(|t| t.name.as_str());
    let candidates = cache.find_older_than(duration.days, tool_filter)?;

    if candidates.entries.is_empty() {
        println!("Nothing to clean older than {}.", duration);
        return Ok(());
    }

    let count = candidates.entries.len();
    let verb = if dry_run {
        "Would remove"
    } else {
        "Will remove"
    };
    println!(
        "{verb} {count} item{} older than {} ({}):",
        if count == 1 { "" } else { "s" },
        duration,
        format_size(candidates.total_bytes)
    );
    for entry in &candidates.entries {
        println!("  {}", entry.label);
    }

    if dry_run {
        return Ok(());
    }

    if !yes && !confirm("Proceed?")? {
        println!("Aborted.");
        return Ok(());
    }

    let freed = cache.remove_candidates(&candidates)?;
    println!(
        "Removed {count} item{}. Freed {}.",
        if count == 1 { "" } else { "s" },
        format_size(freed)
    );
    Ok(())
}

/// Prompt the user for confirmation. Returns true if they answer y/Y.
fn confirm(prompt: &str) -> Result<bool, RunxError> {
    print!("{prompt} [y/N] ");
    io::stdout().flush().map_err(RunxError::Io)?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(RunxError::Io)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Cache;
    use crate::platform::{Arch, Platform, Target};

    fn test_target() -> Target {
        Target {
            platform: Platform::MacOS,
            arch: Arch::Aarch64,
        }
    }

    #[test]
    fn test_clean_all_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        // list_cached returns empty when no tools cached
        let tools = cache.list_cached().unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_clean_tool_not_cached() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let freed = cache.clean_tool("nonexistent").unwrap();
        assert_eq!(freed, 0);
    }

    #[test]
    fn test_clean_tool_with_data() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let target = test_target();
        let version = semver::Version::new(18, 0, 0);

        cache
            .prepare_install_dir("node", &version, &target)
            .unwrap();
        let install = cache.install_path("node", &version, &target);
        std::fs::write(install.join("bin"), "binary data here").unwrap();

        let freed = cache.clean_tool("node").unwrap();
        assert!(freed > 0);
        assert!(!cache.is_cached("node", &version, &target));
    }

    #[test]
    fn test_clean_all_with_data() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let target = test_target();

        cache
            .prepare_install_dir("node", &semver::Version::new(18, 0, 0), &target)
            .unwrap();
        cache
            .prepare_install_dir("python", &semver::Version::new(3, 11, 0), &target)
            .unwrap();

        let tools = cache.list_cached().unwrap();
        assert_eq!(tools.len(), 2);

        let freed = cache.clean_all().unwrap();
        // Empty dirs have 0 file bytes, but the operation should succeed
        assert!(!cache.root().exists());
        assert_eq!(freed, 0);
    }

    #[test]
    fn test_find_older_than_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let result = cache.find_older_than(30, None).unwrap();
        assert!(result.entries.is_empty());
        assert_eq!(result.total_bytes, 0);
    }

    #[test]
    fn test_find_older_than_recent() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let target = test_target();

        // Create a recent cache entry
        cache
            .prepare_install_dir("node", &semver::Version::new(20, 0, 0), &target)
            .unwrap();

        // Nothing should be older than 30 days since we just created it
        let result = cache.find_older_than(30, None).unwrap();
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_find_older_than_with_tool_filter() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let target = test_target();

        cache
            .prepare_install_dir("node", &semver::Version::new(18, 0, 0), &target)
            .unwrap();
        cache
            .prepare_install_dir("python", &semver::Version::new(3, 11, 0), &target)
            .unwrap();

        // Filter to only python — nothing recent to remove
        let result = cache.find_older_than(30, Some("python")).unwrap();
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_clean_candidates_default() {
        let result = crate::cache::CleanCandidates::default();
        assert_eq!(result.total_bytes, 0);
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_format_size_in_clean() {
        // Verify format_size is accessible and works
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }
}

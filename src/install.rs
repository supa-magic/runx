use std::path::{Path, PathBuf};

use crate::cache::Cache;
use crate::cli::ToolSpec;
use crate::download::download_and_install;
use crate::error::RunxError;
use crate::platform::Target;
use crate::provider;

/// Return the global bin directory at `~/.runx/bin/`.
fn bin_dir() -> Result<PathBuf, RunxError> {
    let home = dirs::home_dir().ok_or(RunxError::NoHomeDir)?;
    Ok(home.join(".runx").join("bin"))
}

/// Execute `runx install`.
pub async fn install(tool: Option<ToolSpec>, list: bool) -> Result<(), RunxError> {
    if list {
        return list_installed();
    }

    let specs = match tool {
        Some(spec) => vec![spec],
        None => {
            // Install from .runxrc
            let cwd = std::env::current_dir().map_err(RunxError::NoCwd)?;
            let cfg = crate::config::load_config(&cwd)?;
            if cfg.tools.is_empty() {
                eprintln!("No tool specified and no .runxrc found.\n  Usage: runx install node@22");
                return Ok(());
            }
            cfg.tools
        }
    };

    let target = Target::detect().map_err(RunxError::UnsupportedPlatform)?;
    let cache = Cache::new()?;
    let bin = bin_dir()?;
    std::fs::create_dir_all(&bin).map_err(RunxError::Io)?;

    for spec in &specs {
        install_tool(spec, &target, &cache, &bin).await?;
    }

    // Check if ~/.runx/bin is in PATH
    if let Ok(path) = std::env::var("PATH")
        && !std::env::split_paths(&path).any(|p| p == bin)
    {
        eprintln!();
        eprintln!("Add ~/.runx/bin to your PATH:");
        eprintln!("  export PATH=\"$HOME/.runx/bin:$PATH\"");
    }

    Ok(())
}

/// Install a single tool: resolve, download if needed, symlink.
async fn install_tool(
    spec: &ToolSpec,
    target: &Target,
    cache: &Cache,
    bin_dir: &Path,
) -> Result<(), RunxError> {
    let provider = provider::get_provider(&spec.name)?;

    let version_spec = spec.version_spec()?;

    eprintln!("Resolving {}@{}...", spec.name, version_spec);
    let version = provider.resolve_version(&version_spec, target)?;
    eprintln!("Resolved {} → {}", spec.name, version);

    let install_dir = cache.install_path(provider.name(), &version, target);

    // Download if not cached
    if !cache.is_cached(provider.name(), &version, target) {
        let url = provider.download_url(&version, target)?;
        let format = provider.archive_format(target);
        eprintln!("Downloading {}...", spec.name);
        download_and_install(&url, &install_dir, format, None, false).await?;
        eprintln!("Installed {} to cache", spec.name);
    }

    // Create symlinks
    let bin_paths = provider.bin_paths(&version, target);
    let mut linked = Vec::new();

    for rel_bin_path in &bin_paths {
        let abs_bin_dir = install_dir.join(rel_bin_path);
        if !abs_bin_dir.is_dir() {
            continue;
        }

        let entries = std::fs::read_dir(&abs_bin_dir).map_err(RunxError::Io)?;
        for entry in entries {
            let entry = entry.map_err(RunxError::Io)?;
            let ft = entry.file_type().map_err(RunxError::Io)?;
            if !ft.is_file() && !ft.is_symlink() {
                continue;
            }

            let file_name = entry.file_name();
            let link_path = bin_dir.join(&file_name);

            // Remove existing symlink/file
            if link_path.exists() || link_path.symlink_metadata().is_ok() {
                let _ = std::fs::remove_file(&link_path);
            }

            #[cfg(unix)]
            std::os::unix::fs::symlink(entry.path(), &link_path).map_err(RunxError::Io)?;

            #[cfg(windows)]
            {
                // On Windows, create a .cmd shim
                let shim_path = link_path.with_extension("cmd");
                let shim_content = format!("@echo off\r\n\"{}\" %*\r\n", entry.path().display());
                std::fs::write(&shim_path, shim_content).map_err(RunxError::Io)?;
            }

            linked.push(file_name.to_string_lossy().to_string());
        }
    }

    if linked.is_empty() {
        eprintln!("Warning: no binaries found for {}@{}", spec.name, version);
    } else {
        println!(
            "Installed {}@{} → ~/.runx/bin/{{{}}}",
            spec.name,
            version,
            linked.join(", ")
        );
    }

    Ok(())
}

/// List all globally installed tools.
fn list_installed() -> Result<(), RunxError> {
    let bin = bin_dir()?;
    if !bin.exists() {
        println!("No globally installed tools.");
        println!("  Use `runx install node@22` to install a tool.");
        return Ok(());
    }

    let entries = std::fs::read_dir(&bin).map_err(RunxError::Io)?;
    let mut tools: Vec<(String, String)> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(RunxError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Read symlink target to show which version
        let target = std::fs::read_link(entry.path())
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "?".to_string());

        tools.push((name, target));
    }

    if tools.is_empty() {
        println!("No globally installed tools.");
        return Ok(());
    }

    tools.sort_by(|a, b| a.0.cmp(&b.0));

    println!("Globally installed tools (~/.runx/bin/):");
    println!();
    for (name, target) in &tools {
        println!("  {name} → {target}");
    }
    println!();
    println!("Directory: {}", bin.display());
    Ok(())
}

/// Uninstall a globally installed tool.
pub fn uninstall(spec: &ToolSpec) -> Result<(), RunxError> {
    let bin = bin_dir()?;
    if !bin.exists() {
        println!("No globally installed tools.");
        return Ok(());
    }

    // Build the expected cache prefix: ~/.runx/cache/<tool>/
    let cache = crate::cache::Cache::new()?;
    let cache_tool_dir = cache.root().join(&spec.name);

    let entries = std::fs::read_dir(&bin).map_err(RunxError::Io)?;
    let mut removed = Vec::new();

    for entry in entries {
        let entry = entry.map_err(RunxError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();

        if let Ok(target) = std::fs::read_link(entry.path()) {
            // Check if the symlink points into this tool's cache directory
            if target.starts_with(&cache_tool_dir) {
                std::fs::remove_file(entry.path()).map_err(RunxError::Io)?;
                removed.push(name);
            }
        }
    }

    if removed.is_empty() {
        println!("No installed binaries found for {}.", spec.name);
    } else {
        println!("Uninstalled {}: {}", spec.name, removed.join(", "));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bin_dir() {
        let dir = bin_dir().unwrap();
        assert!(dir.ends_with(".runx/bin"));
    }

    #[test]
    fn test_list_installed_empty() {
        // Should not panic when bin dir doesn't exist
        let result = list_installed();
        assert!(result.is_ok());
    }

    #[test]
    fn test_uninstall_nonexistent() {
        let spec = ToolSpec {
            name: "nonexistent".to_string(),
            version: None,
        };
        let result = uninstall(&spec);
        assert!(result.is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_creation_and_removal() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("bin");
        std::fs::create_dir(&bin_path).unwrap();

        // Create a fake binary
        let tool_dir = dir.path().join("tool");
        std::fs::create_dir(&tool_dir).unwrap();
        let fake_binary = tool_dir.join("mytool");
        std::fs::write(&fake_binary, "#!/bin/sh\necho hi").unwrap();

        // Create symlink
        let link = bin_path.join("mytool");
        std::os::unix::fs::symlink(&fake_binary, &link).unwrap();
        assert!(link.exists());

        // Read symlink target
        let target = std::fs::read_link(&link).unwrap();
        assert_eq!(target, fake_binary);

        // Remove symlink
        std::fs::remove_file(&link).unwrap();
        assert!(!link.exists());
        // Original still exists
        assert!(fake_binary.exists());
    }
}

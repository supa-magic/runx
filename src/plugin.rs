use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::RunxError;
use crate::platform::Target;
use crate::provider::{ArchiveFormat, Provider, ProviderError};
use crate::version::VersionSpec;

/// Plugin directory at `~/.runx/plugins/`.
fn plugins_dir() -> Result<PathBuf, RunxError> {
    let home = dirs::home_dir().ok_or(RunxError::NoHomeDir)?;
    Ok(home.join(".runx").join("plugins"))
}

/// A declarative plugin manifest (TOML).
///
/// Example `~/.runx/plugins/zig.toml`:
/// ```toml
/// name = "zig"
/// aliases = ["ziglang"]
/// versions_url = "https://ziglang.org/download/index.json"
/// download_url = "https://ziglang.org/builds/zig-{os}-{arch}-{version}.tar.xz"
/// bin_path = "zig-{os}-{arch}-{version}"
/// interpreter = ["zig", "run"]
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    /// Tool name (e.g., "zig").
    pub name: String,
    /// Aliases (e.g., ["ziglang"]).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Description of the tool.
    #[serde(default)]
    pub description: String,
    /// Download URL template with `{version}`, `{os}`, `{arch}` placeholders.
    pub download_url: String,
    /// Archive format: "tar.gz", "tar.xz", or "zip".
    #[serde(default = "default_archive_format")]
    pub archive_format: String,
    /// Bin path relative to install dir, with `{version}`, `{os}`, `{arch}` placeholders.
    #[serde(default = "default_bin_path")]
    pub bin_path: String,
    /// Interpreter command for shebang (e.g., ["zig", "run"]).
    #[serde(default)]
    #[allow(dead_code)] // Reserved for future shebang integration with plugins
    pub interpreter: Vec<String>,
}

fn default_archive_format() -> String {
    "tar.gz".to_string()
}

fn default_bin_path() -> String {
    "bin".to_string()
}

impl PluginManifest {
    /// Expand placeholders in a template string.
    fn expand(&self, template: &str, version: &semver::Version, target: &Target) -> String {
        template
            .replace("{version}", &version.to_string())
            .replace("{os}", &target.platform.to_string().to_lowercase())
            .replace("{arch}", &target.arch.to_string())
    }
}

/// A provider backed by a plugin manifest.
pub struct PluginProvider {
    manifest: PluginManifest,
}

impl Provider for PluginProvider {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn resolve_version(
        &self,
        spec: &VersionSpec,
        _target: &Target,
    ) -> Result<semver::Version, ProviderError> {
        // Plugin providers use exact versions only — the user specifies the version directly
        match spec {
            VersionSpec::Exact(v) => Ok(v.clone()),
            other => Err(ProviderError::ResolutionFailed {
                tool: self.manifest.name.clone(),
                reason: format!(
                    "plugin `{}` requires an exact version (e.g., {}@1.0.0). Got: {other}",
                    self.manifest.name, self.manifest.name
                ),
            }),
        }
    }

    fn download_url(
        &self,
        version: &semver::Version,
        target: &Target,
    ) -> Result<String, ProviderError> {
        Ok(self
            .manifest
            .expand(&self.manifest.download_url, version, target))
    }

    fn archive_format(&self, _target: &Target) -> ArchiveFormat {
        match self.manifest.archive_format.as_str() {
            "tar.xz" => ArchiveFormat::TarXz,
            "zip" => ArchiveFormat::Zip,
            _ => ArchiveFormat::TarGz,
        }
    }

    fn bin_paths(&self, version: &semver::Version, target: &Target) -> Vec<PathBuf> {
        vec![PathBuf::from(self.manifest.expand(
            &self.manifest.bin_path,
            version,
            target,
        ))]
    }

    fn env_vars(&self, _install_dir: &Path) -> HashMap<String, String> {
        HashMap::new()
    }
}

/// Load all plugin manifests from `~/.runx/plugins/`.
pub fn load_plugins() -> Result<Vec<PluginManifest>, RunxError> {
    let dir = plugins_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(RunxError::Io)?;

    for entry in entries {
        let entry = entry.map_err(RunxError::Io)?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            let content = std::fs::read_to_string(&path).map_err(RunxError::Io)?;
            match toml::from_str::<PluginManifest>(&content) {
                Ok(manifest) => plugins.push(manifest),
                Err(e) => {
                    eprintln!("Warning: failed to parse plugin {}: {e}", path.display());
                }
            }
        }
    }

    Ok(plugins)
}

/// Try to get a provider from installed plugins.
pub fn get_plugin_provider(name: &str) -> Result<Option<Box<dyn Provider>>, RunxError> {
    let plugins = load_plugins()?;
    for manifest in plugins {
        if manifest.name == name || manifest.aliases.iter().any(|a| a == name) {
            return Ok(Some(Box::new(PluginProvider { manifest })));
        }
    }
    Ok(None)
}

/// Execute `runx plugin` subcommands.
pub fn run_plugin_command(action: &str, arg: Option<&str>) -> Result<(), RunxError> {
    match action {
        "list" => list_plugins(),
        "add" => {
            let url = arg.ok_or(RunxError::Plugin(
                "usage: runx plugin add <url-or-path>".to_string(),
            ))?;
            add_plugin(url)
        }
        "remove" => {
            let name = arg.ok_or(RunxError::Plugin(
                "usage: runx plugin remove <name>".to_string(),
            ))?;
            remove_plugin(name)
        }
        _ => {
            eprintln!("Unknown plugin command: {action}");
            eprintln!("  runx plugin list              Show installed plugins");
            eprintln!("  runx plugin add <path>        Install a plugin from a .toml file");
            eprintln!("  runx plugin remove <name>     Remove a plugin");
            Ok(())
        }
    }
}

/// List installed plugins.
fn list_plugins() -> Result<(), RunxError> {
    let plugins = load_plugins()?;
    if plugins.is_empty() {
        println!("No plugins installed.");
        println!("  Create a .toml manifest and add it with `runx plugin add <path>`");
        return Ok(());
    }

    println!("Installed plugins (~/.runx/plugins/):");
    println!();
    for plugin in &plugins {
        let aliases = if plugin.aliases.is_empty() {
            String::new()
        } else {
            format!(" (aliases: {})", plugin.aliases.join(", "))
        };
        println!("  {}{}", plugin.name, aliases);
        if !plugin.description.is_empty() {
            println!("    {}", plugin.description);
        }
    }
    Ok(())
}

/// Add a plugin from a local .toml file path.
fn add_plugin(source: &str) -> Result<(), RunxError> {
    let source_path = Path::new(source);
    if !source_path.is_file() {
        eprintln!("File not found: {source}");
        return Ok(());
    }

    // Validate the manifest
    let content = std::fs::read_to_string(source_path).map_err(RunxError::Io)?;
    let manifest: PluginManifest = toml::from_str(&content)
        .map_err(|e| RunxError::Plugin(format!("invalid plugin manifest: {e}")))?;

    let dir = plugins_dir()?;
    std::fs::create_dir_all(&dir).map_err(RunxError::Io)?;

    let dest = dir.join(format!("{}.toml", manifest.name));
    std::fs::copy(source_path, &dest).map_err(RunxError::Io)?;

    println!("Installed plugin: {}", manifest.name);
    println!(
        "  Use: runx --with {}@<version> -- <command>",
        manifest.name
    );
    Ok(())
}

/// Remove a plugin by name.
fn remove_plugin(name: &str) -> Result<(), RunxError> {
    let dir = plugins_dir()?;
    let path = dir.join(format!("{name}.toml"));

    if !path.exists() {
        println!("Plugin `{name}` not found.");
        return Ok(());
    }

    std::fs::remove_file(&path).map_err(RunxError::Io)?;
    println!("Removed plugin: {name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manifest_deserialize() {
        let toml_str = r#"
name = "zig"
aliases = ["ziglang"]
description = "Zig programming language"
download_url = "https://ziglang.org/builds/zig-{os}-{arch}-{version}.tar.xz"
archive_format = "tar.xz"
bin_path = "zig-{os}-{arch}-{version}"
interpreter = ["zig", "run"]
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.name, "zig");
        assert_eq!(manifest.aliases, vec!["ziglang"]);
        assert_eq!(manifest.archive_format, "tar.xz");
        assert_eq!(manifest.interpreter, vec!["zig", "run"]);
    }

    #[test]
    fn test_plugin_manifest_minimal() {
        let toml_str = r#"
name = "mytool"
download_url = "https://example.com/{version}/mytool.tar.gz"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.name, "mytool");
        assert!(manifest.aliases.is_empty());
        assert_eq!(manifest.archive_format, "tar.gz");
        assert_eq!(manifest.bin_path, "bin");
    }

    #[test]
    fn test_plugin_expand_template() {
        let manifest = PluginManifest {
            name: "zig".to_string(),
            aliases: vec![],
            description: String::new(),
            download_url: "https://example.com/zig-{os}-{arch}-{version}.tar.xz".to_string(),
            archive_format: "tar.xz".to_string(),
            bin_path: "zig-{os}-{arch}-{version}".to_string(),
            interpreter: vec![],
        };

        let version = semver::Version::new(0, 11, 0);
        let target = Target::new(
            crate::platform::Platform::Linux,
            crate::platform::Arch::X86_64,
        );

        let url = manifest.expand(&manifest.download_url, &version, &target);
        assert!(url.contains("0.11.0"));
        assert!(url.contains("linux"));
    }

    #[test]
    fn test_load_plugins_empty() {
        // Should not panic when plugins dir doesn't exist
        let result = load_plugins();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_plugin_provider_not_found() {
        let result = get_plugin_provider("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_plugin_provider_archive_formats() {
        let make_manifest = |fmt: &str| PluginManifest {
            name: "test".to_string(),
            aliases: vec![],
            description: String::new(),
            download_url: String::new(),
            archive_format: fmt.to_string(),
            bin_path: "bin".to_string(),
            interpreter: vec![],
        };

        let target = Target::new(
            crate::platform::Platform::Linux,
            crate::platform::Arch::X86_64,
        );

        let p = PluginProvider {
            manifest: make_manifest("tar.gz"),
        };
        assert_eq!(p.archive_format(&target), ArchiveFormat::TarGz);

        let p = PluginProvider {
            manifest: make_manifest("tar.xz"),
        };
        assert_eq!(p.archive_format(&target), ArchiveFormat::TarXz);

        let p = PluginProvider {
            manifest: make_manifest("zip"),
        };
        assert_eq!(p.archive_format(&target), ArchiveFormat::Zip);
    }

    #[test]
    fn test_remove_plugin_not_found() {
        let result = remove_plugin("nonexistent");
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_plugins_no_dir() {
        let result = list_plugins();
        assert!(result.is_ok());
    }
}

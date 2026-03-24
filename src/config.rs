use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cli::ToolSpec;

/// The `.runxrc` config file name.
pub const CONFIG_FILE_NAME: &str = ".runxrc";

/// Walk up from `start_dir` looking for a file with `name`.
pub fn find_ancestor_file(start_dir: &Path, name: &str) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Parsed `.runxrc` configuration.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Path to the config file that was loaded (None if no config found).
    pub source: Option<PathBuf>,
    /// Tool specs from the config file.
    pub tools: Vec<ToolSpec>,
    /// Whether to inherit the user's full environment.
    pub inherit_env: Option<bool>,
}

/// Raw TOML structure matching the `.runxrc` file format.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    /// Tools in `name@version` format.
    tools: Option<Vec<String>>,
    /// Whether to inherit the user's full environment.
    inherit_env: Option<bool>,
}

/// Errors that occur during config file operations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config `{}`: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config `{}`: {reason}", path.display())]
    Parse { path: PathBuf, reason: String },

    #[error("invalid tool spec in config `{}`: {reason}", path.display())]
    InvalidToolSpec { path: PathBuf, reason: String },
}

/// Load the `.runxrc` config file by walking up from `start_dir`.
///
/// Returns `Config::default()` if no config file is found.
pub fn load_config(start_dir: &Path) -> Result<Config, ConfigError> {
    match find_config(start_dir) {
        Some(path) => parse_config_file(&path),
        None => Ok(Config::default()),
    }
}

/// Walk up from `start_dir` looking for `.runxrc`.
fn find_config(start_dir: &Path) -> Option<PathBuf> {
    find_ancestor_file(start_dir, CONFIG_FILE_NAME)
}

/// Parse a `.runxrc` TOML file into a `Config`.
fn parse_config_file(path: &Path) -> Result<Config, ConfigError> {
    let contents = std::fs::read_to_string(path).map_err(|e| ConfigError::Read {
        path: path.to_path_buf(),
        source: e,
    })?;

    let raw: RawConfig = toml::from_str(&contents).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;

    let tools = match raw.tools {
        Some(specs) => specs
            .into_iter()
            .map(|s| {
                s.parse::<ToolSpec>()
                    .map_err(|e| ConfigError::InvalidToolSpec {
                        path: path.to_path_buf(),
                        reason: e,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };

    Ok(Config {
        source: Some(path.to_path_buf()),
        tools,
        inherit_env: raw.inherit_env,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_config_in_current_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "tools = [\"node@18\"]").unwrap();

        let found = find_config(dir.path());
        assert_eq!(found, Some(config_path));
    }

    #[test]
    fn test_find_config_walks_up() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "tools = [\"node@18\"]").unwrap();

        let child = dir.path().join("subdir");
        std::fs::create_dir(&child).unwrap();

        let found = find_config(&child);
        assert_eq!(found, Some(config_path));
    }

    #[test]
    fn test_find_config_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&child).unwrap();

        // No .runxrc anywhere in the temp dir tree
        let found = find_config(&child);
        assert!(found.is_none());
    }

    #[test]
    fn test_parse_full_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(
            &config_path,
            r#"
tools = ["node@18", "python@3.11"]
inherit_env = true
"#,
        )
        .unwrap();

        let config = parse_config_file(&config_path).unwrap();
        assert_eq!(config.source, Some(config_path));
        assert_eq!(config.tools.len(), 2);
        assert_eq!(config.tools[0].name, "node");
        assert_eq!(config.tools[0].version.as_deref(), Some("18"));
        assert_eq!(config.tools[1].name, "python");
        assert_eq!(config.tools[1].version.as_deref(), Some("3.11"));
        assert_eq!(config.inherit_env, Some(true));
    }

    #[test]
    fn test_parse_tools_only() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "tools = [\"go@1.21\"]\n").unwrap();

        let config = parse_config_file(&config_path).unwrap();
        assert_eq!(config.tools.len(), 1);
        assert_eq!(config.tools[0].name, "go");
        assert_eq!(config.inherit_env, None);
    }

    #[test]
    fn test_parse_empty_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "").unwrap();

        let config = parse_config_file(&config_path).unwrap();
        assert!(config.tools.is_empty());
        assert_eq!(config.inherit_env, None);
    }

    #[test]
    fn test_parse_inherit_env_only() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "inherit_env = false\n").unwrap();

        let config = parse_config_file(&config_path).unwrap();
        assert!(config.tools.is_empty());
        assert_eq!(config.inherit_env, Some(false));
    }

    #[test]
    fn test_parse_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "tools = [not valid toml").unwrap();

        let err = parse_config_file(&config_path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains("failed to parse config"));
    }

    #[test]
    fn test_parse_invalid_tool_spec() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "tools = [\"@18\"]\n").unwrap();

        let err = parse_config_file(&config_path).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidToolSpec { .. }));
    }

    #[test]
    fn test_parse_tool_without_version() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "tools = [\"node\"]\n").unwrap();

        let config = parse_config_file(&config_path).unwrap();
        assert_eq!(config.tools[0].name, "node");
        assert_eq!(config.tools[0].version, None);
    }

    #[test]
    fn test_load_config_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_config(dir.path()).unwrap();
        assert!(config.source.is_none());
        assert!(config.tools.is_empty());
    }

    #[test]
    fn test_load_config_with_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE_NAME), "tools = [\"node@20\"]\n").unwrap();

        let config = load_config(dir.path()).unwrap();
        assert!(config.source.is_some());
        assert_eq!(config.tools.len(), 1);
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::Parse {
            path: PathBuf::from("/tmp/.runxrc"),
            reason: "unexpected token".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to parse config `/tmp/.runxrc`: unexpected token"
        );

        let err = ConfigError::InvalidToolSpec {
            path: PathBuf::from("/tmp/.runxrc"),
            reason: "missing tool name".to_string(),
        };
        assert!(err.to_string().contains("invalid tool spec"));
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.source.is_none());
        assert!(config.tools.is_empty());
        assert_eq!(config.inherit_env, None);
    }

    #[test]
    fn test_parse_unknown_key_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "tols = [\"node@18\"]\n").unwrap();

        let err = parse_config_file(&config_path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains("failed to parse config"));
    }

    #[test]
    fn test_find_config_skips_directory() {
        let dir = tempfile::tempdir().unwrap();
        // Create .runxrc as a directory, not a file
        std::fs::create_dir(dir.path().join(CONFIG_FILE_NAME)).unwrap();

        let found = find_config(dir.path());
        assert!(found.is_none());
    }
}

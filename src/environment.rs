use std::collections::HashMap;
use std::path::PathBuf;

use crate::platform::Platform;

/// Safe baseline environment variables that are always inherited from the user's shell.
///
/// These are non-tool-specific variables needed for basic process operation.
const BASELINE_VARS: &[&str] = &["HOME", "USER", "LOGNAME", "TERM", "LANG", "SHELL", "TMPDIR"];

/// Prefixes for environment variables that are inherited when they match.
const BASELINE_PREFIXES: &[&str] = &["LC_", "XDG_"];

/// Errors that occur during environment construction.
#[derive(Debug, thiserror::Error)]
pub enum EnvironmentError {
    /// Failed to create a temporary directory.
    #[error("failed to create temp directory for `{var}`: {source}")]
    TempDir { var: String, source: std::io::Error },
}

/// Manages per-invocation temporary directories with RAII cleanup.
///
/// Some tools require writable directories (GOPATH, DENO_DIR) that should
/// not persist after the runx invocation. This guard creates temp directories
/// and cleans them up when dropped.
#[derive(Debug, Default)]
pub struct TempDirs {
    dirs: Vec<(String, tempfile::TempDir)>,
}

impl TempDirs {
    /// Create a new empty TempDirs guard.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a temporary directory and associate it with an env var name.
    ///
    /// The directory is cleaned up when this guard is dropped.
    pub fn create(&mut self, env_var: &str) -> Result<PathBuf, EnvironmentError> {
        let dir = tempfile::tempdir().map_err(|e| EnvironmentError::TempDir {
            var: env_var.to_string(),
            source: e,
        })?;
        let path = dir.path().to_path_buf();
        self.dirs.push((env_var.to_string(), dir));
        Ok(path)
    }

    /// Return the env var mappings for all managed temp directories.
    pub fn env_vars(&self) -> HashMap<String, String> {
        self.dirs
            .iter()
            .map(|(var, dir)| (var.clone(), dir.path().to_string_lossy().to_string()))
            .collect()
    }
}

/// A fully constructed environment for a child process.
///
/// Contains the PATH and all environment variables needed to run a command
/// in an isolated context with specific tool versions.
#[derive(Debug, Clone)]
pub struct Environment {
    /// The complete set of environment variables for the child process.
    vars: HashMap<String, String>,
}

impl Environment {
    /// Build an isolated environment from tool bin paths and env vars.
    ///
    /// In isolated mode (default):
    /// - PATH = tool bin dirs + minimal system paths
    /// - Only safe baseline vars inherited from user's shell
    /// - Tool-specific env vars set from providers
    ///
    /// In inherit mode (`--inherit-env`):
    /// - PATH = tool bin dirs + user's full PATH
    /// - All user env vars inherited (including `LD_PRELOAD`, `NODE_OPTIONS`, etc.)
    /// - Tool-specific env vars override user's values
    ///
    /// **Security note:** Inherit mode passes the entire user environment through
    /// without filtering. This is an explicit opt-in escape hatch for users who
    /// need their full shell context. Isolated mode (default) is the safe choice.
    pub fn build(
        platform: Platform,
        tool_bin_dirs: &[PathBuf],
        tool_env_vars: &HashMap<String, String>,
        temp_env_vars: &HashMap<String, String>,
        inherit_env: bool,
    ) -> Self {
        let mut vars = if inherit_env {
            Self::inherited_vars(platform, tool_bin_dirs)
        } else {
            Self::isolated_vars(platform, tool_bin_dirs)
        };

        // Apply tool-specific env vars (override any inherited values)
        for (key, value) in tool_env_vars {
            vars.insert(key.clone(), value.clone());
        }

        // Apply temp dir env vars
        for (key, value) in temp_env_vars {
            vars.insert(key.clone(), value.clone());
        }

        Self { vars }
    }

    /// Return the environment variables as a reference for child process spawning.
    pub fn vars(&self) -> &HashMap<String, String> {
        &self.vars
    }

    /// Return the environment variables as owned key-value pairs.
    #[allow(unused)] // Available for consumers that need ownership
    pub fn into_vars(self) -> HashMap<String, String> {
        self.vars
    }

    /// Build isolated environment: only baseline vars + constructed PATH.
    fn isolated_vars(platform: Platform, tool_bin_dirs: &[PathBuf]) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        let sep = platform.path_separator();

        // Construct clean PATH: tool bin dirs + minimal system paths
        let path = Self::build_path(tool_bin_dirs, platform.system_path(), sep);
        vars.insert("PATH".to_string(), path);

        // Inherit safe baseline variables from the current process
        for var_name in BASELINE_VARS {
            if let Ok(value) = std::env::var(var_name) {
                vars.insert(var_name.to_string(), value);
            }
        }

        // Inherit variables matching baseline prefixes (LC_*, XDG_*)
        for (key, value) in std::env::vars() {
            if BASELINE_PREFIXES
                .iter()
                .any(|prefix| key.starts_with(prefix))
            {
                vars.insert(key, value);
            }
        }

        vars
    }

    /// Build inherited environment: full user env + tool paths prepended to PATH.
    fn inherited_vars(platform: Platform, tool_bin_dirs: &[PathBuf]) -> HashMap<String, String> {
        let mut vars: HashMap<String, String> = std::env::vars().collect();
        let sep = platform.path_separator();

        // Prepend tool bin dirs to the user's existing PATH
        let existing_path = vars.get("PATH").cloned().unwrap_or_default();
        let path = Self::build_path(tool_bin_dirs, &existing_path, sep);
        vars.insert("PATH".to_string(), path);

        vars
    }

    /// Construct a PATH string from tool bin dirs and a base path.
    fn build_path(tool_bin_dirs: &[PathBuf], base_path: &str, sep: char) -> String {
        let mut parts: Vec<String> = tool_bin_dirs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        if !base_path.is_empty() {
            parts.push(base_path.to_string());
        }
        parts.join(&sep.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool_bins() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/cache/node/18.19.1/bin"),
            PathBuf::from("/cache/python/3.11.0/bin"),
        ]
    }

    fn make_tool_env_vars() -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert("NODE_HOME".to_string(), "/cache/node/18.19.1".to_string());
        vars.insert("PYTHONHOME".to_string(), "/cache/python/3.11.0".to_string());
        vars
    }

    // --- TempDirs ---

    #[test]
    fn test_temp_dirs_create_and_cleanup() {
        let path;
        {
            let mut temps = TempDirs::new();
            path = temps.create("GOPATH").unwrap();
            assert!(path.exists());
            assert_eq!(temps.env_vars().len(), 1);
            assert!(temps.env_vars().contains_key("GOPATH"));
        }
        // After drop, temp dir should be cleaned up
        assert!(!path.exists());
    }

    #[test]
    fn test_temp_dirs_multiple() {
        let mut temps = TempDirs::new();
        let gopath = temps.create("GOPATH").unwrap();
        let deno_dir = temps.create("DENO_DIR").unwrap();

        assert!(gopath.exists());
        assert!(deno_dir.exists());
        assert_ne!(gopath, deno_dir);

        let env = temps.env_vars();
        assert_eq!(env.len(), 2);
        assert!(env.contains_key("GOPATH"));
        assert!(env.contains_key("DENO_DIR"));
    }

    #[test]
    fn test_temp_dirs_empty() {
        let temps = TempDirs::new();
        assert!(temps.env_vars().is_empty());
    }

    // --- Environment: isolated mode ---

    #[test]
    fn test_isolated_path_contains_tool_bins_and_system() {
        let env = Environment::build(
            Platform::MacOS,
            &make_tool_bins(),
            &HashMap::new(),
            &HashMap::new(),
            false,
        );
        let path = env.vars().get("PATH").unwrap();

        assert!(path.starts_with("/cache/node/18.19.1/bin:"));
        assert!(path.contains("/cache/python/3.11.0/bin"));
        assert!(path.ends_with("/usr/bin:/bin"));
    }

    #[test]
    fn test_isolated_path_does_not_contain_user_path() {
        // Set a recognizable user PATH
        let env = Environment::build(
            Platform::MacOS,
            &make_tool_bins(),
            &HashMap::new(),
            &HashMap::new(),
            false,
        );
        let path = env.vars().get("PATH").unwrap();

        // Should NOT contain typical user PATH entries
        assert!(!path.contains("/usr/local/bin"));
        assert!(!path.contains(".nvm"));
        assert!(!path.contains(".cargo"));
    }

    #[test]
    fn test_isolated_inherits_baseline_vars() {
        // HOME should always exist on Unix/macOS
        let env = Environment::build(
            Platform::MacOS,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            false,
        );

        // HOME should be inherited
        if std::env::var("HOME").is_ok() {
            assert!(env.vars().contains_key("HOME"));
        }
    }

    #[test]
    fn test_isolated_does_not_inherit_tool_vars() {
        let env = Environment::build(
            Platform::MacOS,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            false,
        );

        // Tool-management vars should NOT be in isolated env
        // (they might be set in the user's shell, but we don't inherit them)
        assert!(!env.vars().contains_key("NVM_DIR"));
        assert!(!env.vars().contains_key("PYENV_ROOT"));
    }

    #[test]
    fn test_isolated_tool_env_vars_applied() {
        let env = Environment::build(
            Platform::MacOS,
            &make_tool_bins(),
            &make_tool_env_vars(),
            &HashMap::new(),
            false,
        );

        assert_eq!(env.vars().get("NODE_HOME").unwrap(), "/cache/node/18.19.1");
        assert_eq!(
            env.vars().get("PYTHONHOME").unwrap(),
            "/cache/python/3.11.0"
        );
    }

    #[test]
    fn test_isolated_temp_env_vars_applied() {
        let mut temp_vars = HashMap::new();
        temp_vars.insert("GOPATH".to_string(), "/tmp/gopath-123".to_string());

        let env = Environment::build(Platform::MacOS, &[], &HashMap::new(), &temp_vars, false);

        assert_eq!(env.vars().get("GOPATH").unwrap(), "/tmp/gopath-123");
    }

    // --- Environment: inherit mode ---

    #[test]
    fn test_inherit_prepends_tool_bins_to_path() {
        let env = Environment::build(
            Platform::MacOS,
            &make_tool_bins(),
            &HashMap::new(),
            &HashMap::new(),
            true,
        );
        let path = env.vars().get("PATH").unwrap();

        // Tool bins should be at the start
        assert!(path.starts_with("/cache/node/18.19.1/bin:"));
        assert!(path.contains("/cache/python/3.11.0/bin"));
    }

    #[test]
    fn test_inherit_includes_user_env_vars() {
        let env = Environment::build(Platform::MacOS, &[], &HashMap::new(), &HashMap::new(), true);

        // In inherit mode, all user vars should be present
        if std::env::var("HOME").is_ok() {
            assert!(env.vars().contains_key("HOME"));
        }
    }

    #[test]
    fn test_inherit_tool_vars_override_user_vars() {
        // Even in inherit mode, tool-specific vars should override
        let mut tool_vars = HashMap::new();
        tool_vars.insert("NODE_HOME".to_string(), "/cache/node/18".to_string());

        let env = Environment::build(Platform::MacOS, &[], &tool_vars, &HashMap::new(), true);

        assert_eq!(env.vars().get("NODE_HOME").unwrap(), "/cache/node/18");
    }

    // --- Platform-specific PATH ---

    #[test]
    fn test_windows_path_uses_semicolon_separator() {
        let env = Environment::build(
            Platform::Windows,
            &make_tool_bins(),
            &HashMap::new(),
            &HashMap::new(),
            false,
        );
        let path = env.vars().get("PATH").unwrap();

        assert!(path.contains(';'));
        assert!(path.ends_with(r"C:\Windows\System32;C:\Windows"));
    }

    #[test]
    fn test_linux_path_uses_colon_separator() {
        let env = Environment::build(
            Platform::Linux,
            &make_tool_bins(),
            &HashMap::new(),
            &HashMap::new(),
            false,
        );
        let path = env.vars().get("PATH").unwrap();

        assert!(path.contains(':'));
        assert!(path.ends_with("/usr/bin:/bin"));
    }

    // --- build_path ---

    #[test]
    fn test_build_path_empty_tool_bins() {
        let path = Environment::build_path(&[], "/usr/bin:/bin", ':');
        assert_eq!(path, "/usr/bin:/bin");
    }

    #[test]
    fn test_build_path_empty_base() {
        let bins = vec![PathBuf::from("/cache/node/bin")];
        let path = Environment::build_path(&bins, "", ':');
        assert_eq!(path, "/cache/node/bin");
    }

    #[test]
    fn test_build_path_both_empty() {
        let path = Environment::build_path(&[], "", ':');
        assert_eq!(path, "");
    }

    // --- Baseline prefix inheritance (LC_*, XDG_*) ---

    #[test]
    fn test_isolated_inherits_lc_and_xdg_prefixes() {
        // SAFETY for env mutation: use unique var names to avoid test pollution
        unsafe {
            std::env::set_var("LC_RUNX_TEST", "en_US.UTF-8");
            std::env::set_var("XDG_RUNX_TEST", "/run/user/1000");
            std::env::set_var("RUNX_CUSTOM_SHOULD_NOT_INHERIT", "nope");
        }

        let env = Environment::build(
            Platform::MacOS,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            false,
        );

        assert_eq!(env.vars().get("LC_RUNX_TEST").unwrap(), "en_US.UTF-8");
        assert_eq!(env.vars().get("XDG_RUNX_TEST").unwrap(), "/run/user/1000");
        assert!(!env.vars().contains_key("RUNX_CUSTOM_SHOULD_NOT_INHERIT"));

        // Clean up
        unsafe {
            std::env::remove_var("LC_RUNX_TEST");
            std::env::remove_var("XDG_RUNX_TEST");
            std::env::remove_var("RUNX_CUSTOM_SHOULD_NOT_INHERIT");
        }
    }

    // --- Non-baseline exclusion with real env var ---

    #[test]
    fn test_isolated_excludes_non_baseline_vars() {
        unsafe {
            std::env::set_var("RUNX_TEST_EXCLUDE_ME", "should_not_appear");
        }

        let env = Environment::build(
            Platform::MacOS,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            false,
        );

        assert!(!env.vars().contains_key("RUNX_TEST_EXCLUDE_ME"));

        unsafe {
            std::env::remove_var("RUNX_TEST_EXCLUDE_ME");
        }
    }

    // --- Temp vars override tool vars ---

    #[test]
    fn test_temp_vars_override_tool_vars() {
        let mut tool_vars = HashMap::new();
        tool_vars.insert("GOPATH".to_string(), "/tool/gopath".to_string());

        let mut temp_vars = HashMap::new();
        temp_vars.insert("GOPATH".to_string(), "/tmp/gopath-ephemeral".to_string());

        let env = Environment::build(Platform::MacOS, &[], &tool_vars, &temp_vars, false);

        // Temp vars are applied after tool vars, so temp wins
        assert_eq!(env.vars().get("GOPATH").unwrap(), "/tmp/gopath-ephemeral");
    }

    // --- Inherit mode + temp vars ---

    #[test]
    fn test_inherit_temp_env_vars_applied() {
        let mut temp_vars = HashMap::new();
        temp_vars.insert("DENO_DIR".to_string(), "/tmp/deno-123".to_string());

        let env = Environment::build(Platform::MacOS, &[], &HashMap::new(), &temp_vars, true);

        assert_eq!(env.vars().get("DENO_DIR").unwrap(), "/tmp/deno-123");
    }

    // --- build_path with multiple bins and Windows separator ---

    #[test]
    fn test_build_path_multiple_bins() {
        let bins = vec![
            PathBuf::from("/cache/node/bin"),
            PathBuf::from("/cache/python/bin"),
        ];
        let path = Environment::build_path(&bins, "/usr/bin:/bin", ':');
        assert_eq!(path, "/cache/node/bin:/cache/python/bin:/usr/bin:/bin");
    }

    #[test]
    fn test_build_path_windows_separator() {
        let bins = vec![PathBuf::from(r"C:\cache\node\bin")];
        let path = Environment::build_path(&bins, r"C:\Windows\System32", ';');
        assert_eq!(path, r"C:\cache\node\bin;C:\Windows\System32");
    }

    // --- into_vars ---

    #[test]
    fn test_into_vars_consumes() {
        let env = Environment::build(
            Platform::MacOS,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            false,
        );
        let vars = env.into_vars();
        assert!(vars.contains_key("PATH"));
    }
}

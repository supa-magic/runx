use crate::cache::CacheError;
use crate::config::ConfigError;
use crate::download::DownloadError;
use crate::environment::EnvironmentError;
use crate::executor::ExecutorError;
use crate::provider::ProviderError;

/// Errors that can occur during runx execution.
#[derive(Debug, thiserror::Error)]
pub enum RunxError {
    /// No command specified after `--` separator.
    #[error(
        "no command specified. Use -- to separate the command.\n\
         Example: runx --with node@18 -- node -v"
    )]
    NoCommand,

    /// No tools specified via `--with` flag.
    #[error(
        "no tools specified. Use --with to add tools.\n\
         Example: runx --with node@18 -- node -v"
    )]
    NoTools,

    /// Platform detection failed.
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),

    /// A provider operation failed.
    #[error(transparent)]
    Provider(#[from] ProviderError),

    /// A cache operation failed.
    #[error(transparent)]
    Cache(#[from] CacheError),

    /// A download operation failed.
    #[error(transparent)]
    Download(#[from] DownloadError),

    /// An environment construction error.
    #[error(transparent)]
    Environment(#[from] EnvironmentError),

    /// A command execution error.
    #[error(transparent)]
    Executor(#[from] ExecutorError),

    /// A config file error.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// Failed to determine the current working directory.
    #[error("cannot determine current directory: {0}")]
    NoCwd(std::io::Error),

    /// An I/O error during interactive prompting.
    #[error("I/O error: {0}")]
    Io(std::io::Error),

    /// Cannot determine the home directory.
    #[error("cannot determine home directory")]
    NoHomeDir,

    /// A plugin operation error.
    #[error("plugin error: {0}")]
    Plugin(String),

    /// Child process exited with a non-zero exit code.
    #[error("process exited with code {0}")]
    ProcessExited(i32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_command_display() {
        let err = RunxError::NoCommand;
        let msg = err.to_string();
        assert!(msg.contains("no command specified"));
        assert!(msg.contains("Example:"));
    }

    #[test]
    fn test_no_tools_display() {
        let err = RunxError::NoTools;
        let msg = err.to_string();
        assert!(msg.contains("no tools specified"));
        assert!(msg.contains("--with"));
    }

    #[test]
    fn test_unsupported_platform_display() {
        let err = RunxError::UnsupportedPlatform("RISC-V".to_string());
        assert!(err.to_string().contains("RISC-V"));
    }

    #[test]
    fn test_no_cwd_display() {
        let err = RunxError::NoCwd(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "access denied",
        ));
        let msg = err.to_string();
        assert!(msg.contains("cannot determine current directory"));
        assert!(msg.contains("access denied"));
    }

    #[test]
    fn test_io_display() {
        let err = RunxError::Io(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "broken pipe",
        ));
        assert!(err.to_string().contains("broken pipe"));
    }

    #[test]
    fn test_provider_error_converts() {
        let err: RunxError = ProviderError::UnknownTool {
            name: "ruby".to_string(),
        }
        .into();
        assert!(matches!(err, RunxError::Provider(_)));
        assert!(err.to_string().contains("ruby"));
    }

    #[test]
    fn test_cache_error_converts() {
        let err: RunxError = CacheError::NoHomeDir.into();
        assert!(matches!(err, RunxError::Cache(_)));
    }

    #[test]
    fn test_no_home_dir_display() {
        let err = RunxError::NoHomeDir;
        assert_eq!(err.to_string(), "cannot determine home directory");
    }

    #[test]
    fn test_plugin_error_display() {
        let err = RunxError::Plugin("missing manifest".to_string());
        assert!(err.to_string().contains("missing manifest"));
        assert!(err.to_string().contains("plugin error"));
    }

    #[test]
    fn test_process_exited_display() {
        let err = RunxError::ProcessExited(42);
        assert_eq!(err.to_string(), "process exited with code 42");
    }

    #[test]
    fn test_process_exited_zero_display() {
        // Code 0 means success — but the type still formats correctly
        let err = RunxError::ProcessExited(0);
        assert_eq!(err.to_string(), "process exited with code 0");
    }

    #[test]
    fn test_environment_error_converts() {
        let err: RunxError = crate::environment::EnvironmentError::TempDir {
            var: "GOPATH".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        }
        .into();
        assert!(matches!(err, RunxError::Environment(_)));
        assert!(err.to_string().contains("GOPATH"));
    }

    #[test]
    fn test_executor_error_converts() {
        let err: RunxError = crate::executor::ExecutorError::Spawn {
            program: "sh".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        }
        .into();
        assert!(matches!(err, RunxError::Executor(_)));
        assert!(err.to_string().contains("sh"));
    }

    #[test]
    fn test_download_error_converts() {
        let err: RunxError = crate::download::DownloadError::ChecksumMismatch {
            expected: "aaa".to_string(),
            actual: "bbb".to_string(),
        }
        .into();
        assert!(matches!(err, RunxError::Download(_)));
        assert!(err.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn test_config_error_converts() {
        let err: RunxError = crate::config::ConfigError::Parse {
            path: std::path::PathBuf::from("/tmp/.runxrc"),
            reason: "bad toml".to_string(),
        }
        .into();
        assert!(matches!(err, RunxError::Config(_)));
    }
}

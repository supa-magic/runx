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
    #[allow(unused)]
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),

    /// A provider operation failed.
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

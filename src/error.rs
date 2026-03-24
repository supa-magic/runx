use std::fmt;

use crate::provider::ProviderError;

/// Errors that can occur during runx execution.
#[derive(Debug)]
pub enum RunxError {
    /// No command specified after `--` separator.
    NoCommand,
    /// No tools specified via `--with` flag.
    NoTools,
    /// Platform detection failed (used once Target::detect is wired into run).
    #[allow(unused)]
    UnsupportedPlatform(String),
    /// A provider operation failed.
    Provider(ProviderError),
}

impl fmt::Display for RunxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCommand => write!(
                f,
                "no command specified. Use -- to separate the command.\n\
                 Example: runx --with node@18 -- node -v"
            ),
            Self::NoTools => write!(
                f,
                "no tools specified. Use --with to add tools.\n\
                 Example: runx --with node@18 -- node -v"
            ),
            Self::UnsupportedPlatform(msg) => write!(f, "unsupported platform: {msg}"),
            Self::Provider(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for RunxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Provider(err) => Some(err),
            _ => None,
        }
    }
}

impl From<ProviderError> for RunxError {
    fn from(err: ProviderError) -> Self {
        Self::Provider(err)
    }
}

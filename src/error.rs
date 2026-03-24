use std::fmt;

/// Errors that can occur during runx execution.
#[derive(Debug)]
pub enum RunxError {
    /// No command specified after `--` separator.
    NoCommand,
    /// No tools specified via `--with` flag.
    NoTools,
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
        }
    }
}

impl std::error::Error for RunxError {}

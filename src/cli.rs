use std::fmt;
use std::str::FromStr;

use clap::{Parser, Subcommand};

/// A tool specifier in the format `name` or `name@version`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    pub version: Option<String>,
}

impl FromStr for ToolSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err("tool spec cannot be empty".to_string());
        }

        if let Some((name, version)) = s.split_once('@') {
            if name.is_empty() {
                return Err(format!("missing tool name in `{s}`"));
            }
            if version.is_empty() {
                return Err(format!("missing version after `@` in `{s}`"));
            }
            Ok(Self {
                name: name.to_string(),
                version: Some(version.to_string()),
            })
        } else {
            Ok(Self {
                name: s.to_string(),
                version: None,
            })
        }
    }
}

impl fmt::Display for ToolSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            Some(v) => write!(f, "{}@{}", self.name, v),
            None => write!(f, "{}", self.name),
        }
    }
}

/// A human-readable duration like `30d`, `7d`, `24h`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanDuration {
    pub days: u64,
}

impl FromStr for HumanDuration {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err("duration cannot be empty".to_string());
        }

        if let Some(num) = s.strip_suffix('d') {
            let days = num
                .parse::<u64>()
                .map_err(|_| format!("invalid number in duration `{s}`"))?;
            Ok(Self { days })
        } else if let Some(num) = s.strip_suffix('h') {
            let hours = num
                .parse::<u64>()
                .map_err(|_| format!("invalid number in duration `{s}`"))?;
            Ok(Self {
                days: hours.div_ceil(24),
            })
        } else {
            Err(format!(
                "invalid duration `{s}`. Use a suffix: d (days) or h (hours). Example: 30d"
            ))
        }
    }
}

impl fmt::Display for HumanDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}d", self.days)
    }
}

/// Ephemeral environment runner — run any command with specific tool versions.
#[derive(Parser, Debug)]
#[command(name = "runx", version, about, long_about = None)]
#[command(
    after_help = "Examples:\n  runx --with node@18 -- node -v\n  runx --with node@20 --with python@3.11 -- node process.js\n  runx list --cached\n  runx clean --older-than 30d"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Tool to include in the environment (repeatable, e.g. --with node@18)
    #[arg(long = "with", value_name = "TOOL@VERSION")]
    pub tools: Vec<ToolSpec>,

    /// Show download progress and debug output
    #[arg(short, long, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Show what would be downloaded/executed without doing it
    #[arg(long)]
    pub dry_run: bool,

    /// Inherit the user's full environment instead of isolated env
    #[arg(long)]
    pub inherit_env: bool,

    /// Suppress progress output
    #[arg(short, long)]
    pub quiet: bool,

    /// Command to execute (after --)
    #[arg(last = true)]
    pub cmd: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Remove cached tool binaries to reclaim disk space
    Clean {
        /// Remove only caches for this tool
        #[arg(long, value_name = "NAME")]
        tool: Option<ToolSpec>,

        /// Remove caches older than this duration (e.g. 30d, 7d)
        #[arg(long, value_name = "DURATION")]
        older_than: Option<HumanDuration>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// List available tools and cached versions
    List {
        /// Show only cached tool versions with sizes
        #[arg(long)]
        cached: bool,

        /// Specific tool to query (e.g. node)
        tool: Option<ToolSpec>,
    },

    /// Scaffold a .runxrc config file in the current directory
    Init,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ToolSpec ---

    #[test]
    fn test_tool_spec_with_version() {
        let spec: ToolSpec = "node@18".parse().unwrap();
        assert_eq!(spec.name, "node");
        assert_eq!(spec.version.as_deref(), Some("18"));
        assert_eq!(spec.to_string(), "node@18");
    }

    #[test]
    fn test_tool_spec_with_semver() {
        let spec: ToolSpec = "python@3.11.2".parse().unwrap();
        assert_eq!(spec.name, "python");
        assert_eq!(spec.version.as_deref(), Some("3.11.2"));
    }

    #[test]
    fn test_tool_spec_without_version() {
        let spec: ToolSpec = "node".parse().unwrap();
        assert_eq!(spec.name, "node");
        assert_eq!(spec.version, None);
        assert_eq!(spec.to_string(), "node");
    }

    #[test]
    fn test_tool_spec_empty_rejected() {
        assert!("".parse::<ToolSpec>().is_err());
    }

    #[test]
    fn test_tool_spec_missing_name_rejected() {
        assert!("@18".parse::<ToolSpec>().is_err());
    }

    #[test]
    fn test_tool_spec_missing_version_rejected() {
        assert!("node@".parse::<ToolSpec>().is_err());
    }

    // --- HumanDuration ---

    #[test]
    fn test_duration_days() {
        let d: HumanDuration = "30d".parse().unwrap();
        assert_eq!(d.days, 30);
        assert_eq!(d.to_string(), "30d");
    }

    #[test]
    fn test_duration_hours_rounds_up() {
        let d: HumanDuration = "25h".parse().unwrap();
        assert_eq!(d.days, 2); // 25h = ceil(25/24) = 2 days
    }

    #[test]
    fn test_duration_empty_rejected() {
        assert!("".parse::<HumanDuration>().is_err());
    }

    #[test]
    fn test_duration_invalid_suffix_rejected() {
        assert!("30x".parse::<HumanDuration>().is_err());
    }

    #[test]
    fn test_duration_invalid_number_rejected() {
        assert!("abcd".parse::<HumanDuration>().is_err());
    }
}

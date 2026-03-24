use clap::{Parser, Subcommand};

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
    pub tools: Vec<String>,

    /// Show download progress and debug output
    #[arg(short, long)]
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
        tool: Option<String>,

        /// Remove caches older than this duration (e.g. 30d, 7d)
        #[arg(long, value_name = "DURATION")]
        older_than: Option<String>,
    },

    /// List available tools and cached versions
    List {
        /// Show only cached tool versions with sizes
        #[arg(long)]
        cached: bool,

        /// Specific tool to query (e.g. node)
        tool: Option<String>,
    },

    /// Scaffold a .runxrc config file in the current directory
    Init,
}

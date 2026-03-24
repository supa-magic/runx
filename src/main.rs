#[allow(unused)]
mod cache;
mod cli;
#[allow(unused)]
mod download;
#[allow(unused)]
mod environment;
mod error;
#[allow(unused)]
mod executor;
#[allow(unused)]
mod platform;
#[allow(unused)]
mod provider;
mod run;
#[allow(unused)]
mod version;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run::run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::{Cli, Command};

    // --- CLI parsing ---

    #[test]
    fn test_parse_with_single_tool_and_command() {
        let cli = Cli::try_parse_from(["runx", "--with", "node@18", "--", "node", "-v"]).unwrap();
        assert_eq!(cli.tools.len(), 1);
        assert_eq!(cli.tools[0].name, "node");
        assert_eq!(cli.tools[0].version.as_deref(), Some("18"));
        assert_eq!(cli.cmd, vec!["node", "-v"]);
        assert!(!cli.verbose);
        assert!(!cli.dry_run);
    }

    #[test]
    fn test_parse_with_multiple_tools() {
        let cli = Cli::try_parse_from([
            "runx",
            "--with",
            "node@20",
            "--with",
            "python@3.11",
            "--",
            "node",
            "process.js",
        ])
        .unwrap();
        assert_eq!(cli.tools.len(), 2);
        assert_eq!(cli.tools[0].name, "node");
        assert_eq!(cli.tools[1].name, "python");
        assert_eq!(cli.cmd, vec!["node", "process.js"]);
    }

    #[test]
    fn test_parse_tool_without_version() {
        let cli = Cli::try_parse_from(["runx", "--with", "node", "--", "node", "-v"]).unwrap();
        assert_eq!(cli.tools[0].name, "node");
        assert_eq!(cli.tools[0].version, None);
    }

    #[test]
    fn test_parse_all_flags() {
        let cli = Cli::try_parse_from([
            "runx",
            "--with",
            "node@18",
            "--verbose",
            "--dry-run",
            "--inherit-env",
            "--",
            "node",
            "-v",
        ])
        .unwrap();
        assert!(cli.verbose);
        assert!(cli.dry_run);
        assert!(cli.inherit_env);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_parse_short_flags() {
        let cli = Cli::try_parse_from(["runx", "--with", "node@18", "-v", "--", "node"]).unwrap();
        assert!(cli.verbose);

        let cli = Cli::try_parse_from(["runx", "--with", "node@18", "-q", "--", "node"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn test_verbose_and_quiet_conflict() {
        let result = Cli::try_parse_from(["runx", "--verbose", "--quiet", "--", "node", "-v"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_tool_spec_rejected() {
        let result = Cli::try_parse_from(["runx", "--with", "@18", "--", "node"]);
        assert!(result.is_err());

        let result = Cli::try_parse_from(["runx", "--with", "node@", "--", "node"]);
        assert!(result.is_err());

        let result = Cli::try_parse_from(["runx", "--with", "", "--", "node"]);
        assert!(result.is_err());
    }

    // --- Subcommands ---

    #[test]
    fn test_parse_clean_with_args() {
        let cli = Cli::try_parse_from(["runx", "clean", "--tool", "node", "--older-than", "30d"])
            .unwrap();
        let Some(Command::Clean { tool, older_than }) = cli.command else {
            panic!("expected Clean subcommand");
        };
        assert_eq!(tool.unwrap().name, "node");
        assert_eq!(older_than.unwrap().days, 30);
    }

    #[test]
    fn test_parse_clean_no_args() {
        let cli = Cli::try_parse_from(["runx", "clean"]).unwrap();
        let Some(Command::Clean { tool, older_than }) = cli.command else {
            panic!("expected Clean subcommand");
        };
        assert!(tool.is_none());
        assert!(older_than.is_none());
    }

    #[test]
    fn test_parse_clean_invalid_duration_rejected() {
        let result = Cli::try_parse_from(["runx", "clean", "--older-than", "30x"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_list_with_cached_and_tool() {
        let cli = Cli::try_parse_from(["runx", "list", "--cached", "node"]).unwrap();
        let Some(Command::List { cached, tool }) = cli.command else {
            panic!("expected List subcommand");
        };
        assert!(cached);
        assert_eq!(tool.unwrap().name, "node");
    }

    #[test]
    fn test_parse_list_no_args() {
        let cli = Cli::try_parse_from(["runx", "list"]).unwrap();
        let Some(Command::List { cached, tool }) = cli.command else {
            panic!("expected List subcommand");
        };
        assert!(!cached);
        assert!(tool.is_none());
    }

    #[test]
    fn test_parse_init_subcommand() {
        let cli = Cli::try_parse_from(["runx", "init"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Init)));
    }

    #[test]
    fn test_parse_unknown_flag_rejected() {
        let result = Cli::try_parse_from(["runx", "--unknown"]);
        assert!(result.is_err());
    }
}

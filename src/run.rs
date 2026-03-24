use crate::cli::{Cli, Command};
use crate::error::RunxError;

/// Dispatch CLI arguments to the appropriate subcommand handler.
pub fn run(cli: Cli) -> Result<(), RunxError> {
    match cli.command {
        Some(Command::Clean { tool, older_than }) => {
            println!("clean: tool={tool:?}, older_than={older_than:?}");
        }
        Some(Command::List { cached, tool }) => {
            println!("list: cached={cached}, tool={tool:?}");
        }
        Some(Command::Init) => {
            println!("init: scaffolding .runxrc");
        }
        None => {
            if cli.cmd.is_empty() {
                return Err(RunxError::NoCommand);
            }

            if cli.tools.is_empty() {
                return Err(RunxError::NoTools);
            }

            println!("run: tools={:?}, cmd={:?}", cli.tools, cli.cmd);
            println!(
                "  verbose={}, dry_run={}, inherit_env={}, quiet={}",
                cli.verbose, cli.dry_run, cli.inherit_env, cli.quiet
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;
    use crate::error::RunxError;

    use super::run;

    #[test]
    fn test_run_with_tools_and_command() {
        let cli = Cli::try_parse_from(["runx", "--with", "node@18", "--", "node", "-v"]).unwrap();
        assert!(run(cli).is_ok());
    }

    #[test]
    fn test_run_no_command_returns_error() {
        let cli = Cli::try_parse_from(["runx", "--with", "node@18"]).unwrap();
        let err = run(cli).unwrap_err();
        assert!(matches!(err, RunxError::NoCommand));
    }

    #[test]
    fn test_run_no_tools_returns_error() {
        let cli = Cli::try_parse_from(["runx", "--", "node", "-v"]).unwrap();
        let err = run(cli).unwrap_err();
        assert!(matches!(err, RunxError::NoTools));
    }

    #[test]
    fn test_run_clean_subcommand() {
        let cli = Cli::try_parse_from(["runx", "clean"]).unwrap();
        assert!(run(cli).is_ok());
    }

    #[test]
    fn test_run_clean_with_args() {
        let cli = Cli::try_parse_from(["runx", "clean", "--tool", "node", "--older-than", "30d"])
            .unwrap();
        assert!(run(cli).is_ok());
    }

    #[test]
    fn test_run_list_subcommand() {
        let cli = Cli::try_parse_from(["runx", "list"]).unwrap();
        assert!(run(cli).is_ok());
    }

    #[test]
    fn test_run_list_cached_with_tool() {
        let cli = Cli::try_parse_from(["runx", "list", "--cached", "node"]).unwrap();
        assert!(run(cli).is_ok());
    }

    #[test]
    fn test_run_init_subcommand() {
        let cli = Cli::try_parse_from(["runx", "init"]).unwrap();
        assert!(run(cli).is_ok());
    }
}

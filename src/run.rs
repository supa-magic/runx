use std::collections::HashMap;
use std::path::PathBuf;

use crate::cache::Cache;
use crate::cli::{Cli, Command};
use crate::download::download_and_install;
use crate::environment::{Environment, TempDirs};
use crate::error::RunxError;
use crate::executor;
use crate::platform::Target;
use crate::provider;
use crate::version::VersionSpec;

/// Dispatch CLI arguments to the appropriate subcommand handler.
pub async fn run(cli: Cli) -> Result<(), RunxError> {
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

            run_command(&cli).await?;
        }
    }

    Ok(())
}

/// Execute the main run workflow: resolve tools, download, build env, execute.
async fn run_command(cli: &Cli) -> Result<(), RunxError> {
    let target = Target::detect().map_err(RunxError::UnsupportedPlatform)?;
    let cache = Cache::new()?;

    let mut all_bin_dirs: Vec<PathBuf> = Vec::new();
    let mut all_tool_env_vars: HashMap<String, String> = HashMap::new();
    let mut temp_dirs = TempDirs::new();

    // Resolve, download, and collect bin paths for each tool
    for tool_spec in &cli.tools {
        let provider = provider::get_provider(&tool_spec.name)?;

        // Parse version spec from the CLI tool spec
        let version_spec = match &tool_spec.version {
            Some(v) => v.parse::<VersionSpec>().map_err(|e| {
                crate::provider::ProviderError::ResolutionFailed {
                    tool: tool_spec.name.clone(),
                    reason: e,
                }
            })?,
            None => VersionSpec::Latest,
        };

        if !cli.quiet {
            eprintln!("Resolving {}@{}...", tool_spec.name, version_spec);
        }

        // Resolve version
        let version = provider.resolve_version(&version_spec, &target)?;

        if !cli.quiet {
            eprintln!("Resolved {} → {}", tool_spec.name, version);
        }

        // Check cache
        if !cache.is_cached(provider.name(), &version, &target) {
            let url = provider.download_url(&version, &target)?;
            let format = provider.archive_format(&target);
            let install_dir = cache.install_path(provider.name(), &version, &target);

            if cli.dry_run {
                eprintln!("Would download: {url}");
                eprintln!("Would install to: {}", install_dir.display());
                continue;
            }

            if !cli.quiet {
                eprintln!("Downloading {}@{}...", tool_spec.name, version);
            }

            download_and_install(&url, &install_dir, format, None, cli.quiet).await?;

            if !cli.quiet {
                eprintln!("Installed {}@{}", tool_spec.name, version);
            }
        } else if !cli.quiet {
            eprintln!("Using cached {}@{}", tool_spec.name, version);
        }

        // Collect bin paths (relative to cache install dir)
        let install_dir = cache.install_path(provider.name(), &version, &target);
        let bin_paths = provider.bin_paths(&version, &target);
        for bin_path in &bin_paths {
            all_bin_dirs.push(install_dir.join(bin_path));
        }

        // Collect env vars
        let env_vars = provider.env_vars(&install_dir);
        all_tool_env_vars.extend(env_vars);
    }

    if cli.dry_run {
        eprintln!("Would execute: {:?}", cli.cmd);
        return Ok(());
    }

    // Build isolated environment
    let temp_env_vars = temp_dirs.env_vars();
    let environment = Environment::build(
        target.platform,
        &all_bin_dirs,
        &all_tool_env_vars,
        &temp_env_vars,
        cli.inherit_env,
    );

    // Execute the command
    let program = &cli.cmd[0];
    let args = &cli.cmd[1..];

    let status = executor::execute(program, args, environment.vars())?;

    // temp_dirs is dropped here via RAII, cleaning up temp directories
    drop(temp_dirs);

    let code = executor::exit_code(&status);
    if code != 0 {
        std::process::exit(code);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;
    use crate::error::RunxError;

    use super::run;

    // Note: these tests use the sync error paths only.
    // Full async pipeline tests require network access (nodejs.org).

    #[tokio::test]
    async fn test_run_no_command_returns_error() {
        let cli = Cli::try_parse_from(["runx", "--with", "node@18"]).unwrap();
        let err = run(cli).await.unwrap_err();
        assert!(matches!(err, RunxError::NoCommand));
    }

    #[tokio::test]
    async fn test_run_no_tools_returns_error() {
        let cli = Cli::try_parse_from(["runx", "--", "node", "-v"]).unwrap();
        let err = run(cli).await.unwrap_err();
        assert!(matches!(err, RunxError::NoTools));
    }

    #[tokio::test]
    async fn test_run_clean_subcommand() {
        let cli = Cli::try_parse_from(["runx", "clean"]).unwrap();
        assert!(run(cli).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_clean_with_args() {
        let cli = Cli::try_parse_from(["runx", "clean", "--tool", "node", "--older-than", "30d"])
            .unwrap();
        assert!(run(cli).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_list_subcommand() {
        let cli = Cli::try_parse_from(["runx", "list"]).unwrap();
        assert!(run(cli).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_list_cached_with_tool() {
        let cli = Cli::try_parse_from(["runx", "list", "--cached", "node"]).unwrap();
        assert!(run(cli).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_init_subcommand() {
        let cli = Cli::try_parse_from(["runx", "init"]).unwrap();
        assert!(run(cli).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_unknown_tool_returns_error() {
        let cli = Cli::try_parse_from(["runx", "--with", "nonexistent-tool", "--", "echo", "hi"])
            .unwrap();
        let result = run(cli).await;
        assert!(result.is_err());
    }
}

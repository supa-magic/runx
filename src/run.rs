use std::collections::HashMap;
use std::path::PathBuf;

use std::env;

use crate::cache::Cache;
use crate::cli::{Cli, Command};
use crate::config;
use crate::download::download_and_install;
use crate::environment::{Environment, TempDirs};
use crate::error::RunxError;
use crate::executor;
use crate::platform::Target;
use crate::provider::{self, ArchiveFormat, Provider};
use crate::version::VersionSpec;

/// Dispatch CLI arguments to the appropriate subcommand handler.
pub async fn run(cli: Cli) -> Result<(), RunxError> {
    match cli.command {
        Some(Command::Clean {
            tool,
            older_than,
            yes,
        }) => {
            crate::clean::run(tool, older_than, yes, cli.dry_run)?;
        }
        Some(Command::List { cached, tool }) => {
            crate::list::run(cached, tool).await?;
        }
        Some(Command::Init) => {
            println!("init: scaffolding .runxrc");
        }
        None => {
            if cli.cmd.is_empty() {
                return Err(RunxError::NoCommand);
            }

            // Load .runxrc config and merge with CLI flags
            let cwd = env::current_dir().map_err(RunxError::NoCwd)?;
            let cfg = config::load_config(&cwd)?;

            // CLI --with flags override config tools entirely; if no CLI tools, use config
            let mut merged = cli;
            if merged.tools.is_empty() {
                merged.tools = cfg.tools;
            }

            // Config inherit_env is a default; CLI --inherit-env flag overrides.
            // Since --inherit-env is a bool flag (true if passed, false if not),
            // !merged.inherit_env reliably means "user did not pass --inherit-env".
            if let Some(inherit) = cfg.inherit_env
                && !merged.inherit_env
            {
                merged.inherit_env = inherit;
            }

            if merged.tools.is_empty() {
                return Err(RunxError::NoTools);
            }

            // Show config source in dry-run or verbose mode
            if let Some(ref source) = cfg.source
                && (merged.dry_run || merged.verbose)
            {
                eprintln!("Loaded config: {}", source.display());
            }

            run_command(&merged).await?;
        }
    }

    Ok(())
}

/// A resolved tool ready for download and environment setup.
struct ResolvedTool {
    name: String,
    version: semver::Version,
    provider: Box<dyn Provider>,
    cached: bool,
    download_url: Option<String>,
    archive_format: ArchiveFormat,
    install_dir: PathBuf,
}

/// Execute the main run workflow: resolve tools, download in parallel, build env, execute.
async fn run_command(cli: &Cli) -> Result<(), RunxError> {
    let target = Target::detect().map_err(RunxError::UnsupportedPlatform)?;
    let cache = Cache::new()?;

    // Phase 1: Resolve all tools sequentially (version resolution uses blocking HTTP)
    let mut resolved_tools = Vec::new();
    for tool_spec in &cli.tools {
        let provider = provider::get_provider(&tool_spec.name)?;

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

        let version = provider.resolve_version(&version_spec, &target)?;

        if !cli.quiet {
            eprintln!("Resolved {} → {}", tool_spec.name, version);
        }

        let cached = cache.is_cached(provider.name(), &version, &target);
        let install_dir = cache.install_path(provider.name(), &version, &target);

        let download_url = if !cached {
            let url = provider.download_url(&version, &target)?;
            if cli.dry_run {
                eprintln!("Would download: {url}");
                eprintln!("Would install to: {}", install_dir.display());
            }
            Some(url)
        } else {
            if !cli.quiet {
                eprintln!("Using cached {}@{}", tool_spec.name, version);
            }
            None
        };

        let archive_format = provider.archive_format(&target);

        resolved_tools.push(ResolvedTool {
            name: tool_spec.name.clone(),
            version,
            provider,
            cached,
            download_url,
            archive_format,
            install_dir,
        });
    }

    if cli.dry_run {
        eprintln!("Would execute: {:?}", cli.cmd);
        return Ok(());
    }

    // Phase 2: Download all uncached tools in parallel
    let downloads: Vec<_> = resolved_tools
        .iter()
        .filter(|t| !t.cached)
        .collect::<Vec<_>>();

    if !downloads.is_empty() {
        if !cli.quiet && downloads.len() > 1 {
            eprintln!("Downloading {} tools in parallel...", downloads.len());
        }

        let mut join_set = tokio::task::JoinSet::new();

        for tool in &downloads {
            let url = tool.download_url.clone().expect("uncached tool has URL");
            let install_dir = tool.install_dir.clone();
            let format = tool.archive_format;
            let quiet = cli.quiet;
            let name = tool.name.clone();

            join_set.spawn(async move {
                if !quiet {
                    eprintln!("Downloading {name}...");
                }
                let result = download_and_install(&url, &install_dir, format, None, quiet).await;
                if !quiet && result.is_ok() {
                    eprintln!("Installed {name}");
                }
                (name, result)
            });
        }

        // Collect results — report all errors, not just the first
        let mut errors = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok((name, Err(e))) => errors.push(format!("{name}: {e}")),
                Err(e) => errors.push(format!("task failed: {e}")),
                Ok((_, Ok(()))) => {}
            }
        }

        if !errors.is_empty() {
            return Err(RunxError::Download(
                crate::download::DownloadError::Extraction {
                    path: PathBuf::from("parallel downloads"),
                    reason: errors.join("; "),
                },
            ));
        }
    }

    // Phase 3: Collect bin paths, env vars, and temp dirs
    let mut all_bin_dirs: Vec<PathBuf> = Vec::new();
    let mut all_tool_env_vars: HashMap<String, String> = HashMap::new();
    let mut temp_dirs = TempDirs::new();

    for tool in &resolved_tools {
        // Bin paths
        let bin_paths = tool.provider.bin_paths(&tool.version, &target);
        for bin_path in &bin_paths {
            all_bin_dirs.push(tool.install_dir.join(bin_path));
        }

        // Env vars
        let env_vars = tool.provider.env_vars(&tool.install_dir);
        all_tool_env_vars.extend(env_vars);

        // Temp directories
        for env_var in tool.provider.temp_env_dirs() {
            let dir = temp_dirs.create(env_var)?;
            if !cli.quiet {
                eprintln!("  {env_var}={}", dir.display());
            }
        }
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

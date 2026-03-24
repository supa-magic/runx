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
        Some(Command::Init { tools, force }) => {
            crate::init::run(&tools, force)?;
        }
        Some(Command::Install { tool, list }) => {
            crate::install::install(tool, list).await?;
        }
        Some(Command::Uninstall { tool }) => {
            crate::install::uninstall(&tool)?;
        }
        Some(Command::Lock { update }) => {
            crate::lockfile::run(update)?;
        }
        Some(Command::Update { tool }) => {
            crate::update::run(tool, cli.dry_run).await?;
        }
        Some(Command::Plugin { action, arg }) => {
            crate::plugin::run_plugin_command(&action, arg.as_deref())?;
        }
        Some(Command::Completions { shell }) => {
            generate_completions(shell);
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
                // Check for lockfile — use locked versions if available
                if let Some(lockpath) = crate::lockfile::find_lockfile(&cwd) {
                    let lockfile = crate::lockfile::load_lockfile(&lockpath)?;
                    if !lockfile.tools.is_empty() {
                        if merged.dry_run || merged.verbose {
                            eprintln!("Using lockfile: {}", lockpath.display());
                        }
                        merged.tools = lockfile
                            .tools
                            .iter()
                            .map(|(name, locked)| crate::cli::ToolSpec {
                                name: name.clone(),
                                version: Some(locked.version.clone()),
                            })
                            .collect();
                    }
                }
                // Fall back to .runxrc if no lockfile
                if merged.tools.is_empty() {
                    merged.tools = cfg.tools;
                }
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

    // Shebang / script detection: if cmd[0] is a file path (not a binary name),
    // infer the interpreter from the tool spec and prepend it.
    let (program, args) = resolve_script_command(&cli.cmd, &cli.tools);

    let status = executor::execute(&program, &args, environment.vars())?;

    drop(temp_dirs);

    let code = executor::exit_code(&status);
    if code != 0 {
        std::process::exit(code);
    }

    Ok(())
}

/// Map a tool name to its default interpreter command for script execution.
///
/// Returns `None` if the tool doesn't have a natural script-running mode.
fn tool_interpreter(tool_name: &str) -> Option<Vec<String>> {
    match tool_name {
        "node" | "nodejs" => Some(vec!["node".to_string()]),
        "python" | "python3" => Some(vec!["python3".to_string()]),
        "deno" => Some(vec!["deno".to_string(), "run".to_string()]),
        "bun" | "bunx" => Some(vec!["bun".to_string(), "run".to_string()]),
        "go" | "golang" => Some(vec!["go".to_string(), "run".to_string()]),
        _ => None,
    }
}

/// Detect if cmd[0] is a script file and prepend the interpreter if needed.
///
/// When runx is invoked via shebang (`#!/usr/bin/env -S runx --with node@22 --`),
/// the kernel passes the script path as the first argument after `--`.
/// This function detects that case and prepends the interpreter command.
fn resolve_script_command(cmd: &[String], tools: &[crate::cli::ToolSpec]) -> (String, Vec<String>) {
    debug_assert!(!cmd.is_empty(), "cmd must not be empty");
    let first = &cmd[0];

    // If cmd[0] is already a known tool binary, use it as-is
    if provider::get_provider(first).is_ok() || !std::path::Path::new(first).is_file() {
        return (first.clone(), cmd[1..].to_vec());
    }

    // cmd[0] is a file — try to infer the interpreter from the tool spec
    if tools.len() == 1
        && let Some(mut interpreter) = tool_interpreter(&tools[0].name)
    {
        let script_path = first.clone();
        let mut args: Vec<String> = Vec::new();
        let program = interpreter.remove(0);
        args.extend(interpreter);
        args.push(script_path);
        args.extend_from_slice(&cmd[1..]);
        return (program, args);
    }

    // Fallback: use as-is
    (first.clone(), cmd[1..].to_vec())
}

/// Generate shell completions and print to stdout.
fn generate_completions(shell: crate::cli::ShellType) {
    use clap::CommandFactory;
    use clap_complete::Shell;

    let shell = match shell {
        crate::cli::ShellType::Bash => Shell::Bash,
        crate::cli::ShellType::Zsh => Shell::Zsh,
        crate::cli::ShellType::Fish => Shell::Fish,
    };

    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "runx", &mut std::io::stdout());
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::{Cli, Command};
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
    async fn test_run_init_subcommand_parses() {
        // Verify init subcommand parses correctly (don't actually run it,
        // since it writes to CWD which conflicts with parallel tests)
        let cli = Cli::try_parse_from(["runx", "init", "--with", "node@18", "--force"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Init { .. })));
    }

    #[tokio::test]
    async fn test_run_unknown_tool_returns_error() {
        let cli = Cli::try_parse_from(["runx", "--with", "nonexistent-tool", "--", "echo", "hi"])
            .unwrap();
        let result = run(cli).await;
        assert!(result.is_err());
    }

    // --- Shebang / script detection tests ---

    #[test]
    fn test_tool_interpreter_node() {
        let interp = super::tool_interpreter("node").unwrap();
        assert_eq!(interp, vec!["node"]);
    }

    #[test]
    fn test_tool_interpreter_python() {
        let interp = super::tool_interpreter("python").unwrap();
        assert_eq!(interp, vec!["python3"]);
    }

    #[test]
    fn test_tool_interpreter_deno() {
        let interp = super::tool_interpreter("deno").unwrap();
        assert_eq!(interp, vec!["deno", "run"]);
    }

    #[test]
    fn test_tool_interpreter_go() {
        let interp = super::tool_interpreter("go").unwrap();
        assert_eq!(interp, vec!["go", "run"]);
    }

    #[test]
    fn test_tool_interpreter_bun() {
        let interp = super::tool_interpreter("bun").unwrap();
        assert_eq!(interp, vec!["bun", "run"]);
    }

    #[test]
    fn test_tool_interpreter_unknown() {
        assert!(super::tool_interpreter("ruby").is_none());
    }

    #[test]
    fn test_resolve_script_command_binary_name() {
        let cmd = vec!["node".to_string(), "-v".to_string()];
        let tools = vec![];
        let (program, args) = super::resolve_script_command(&cmd, &tools);
        assert_eq!(program, "node");
        assert_eq!(args, vec!["-v"]);
    }

    #[test]
    fn test_resolve_script_command_nonexistent_file() {
        let cmd = vec!["nonexistent_file.js".to_string()];
        let tools = vec![crate::cli::ToolSpec {
            name: "node".to_string(),
            version: Some("22".to_string()),
        }];
        let (program, args) = super::resolve_script_command(&cmd, &tools);
        // File doesn't exist, so treated as a binary name
        assert_eq!(program, "nonexistent_file.js");
        assert!(args.is_empty());
    }

    #[test]
    fn test_resolve_script_command_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("test.js");
        std::fs::write(&script, "console.log('hello')").unwrap();

        let cmd = vec![script.to_string_lossy().to_string()];
        let tools = vec![crate::cli::ToolSpec {
            name: "node".to_string(),
            version: Some("22".to_string()),
        }];
        let (program, args) = super::resolve_script_command(&cmd, &tools);
        assert_eq!(program, "node");
        assert_eq!(args, vec![script.to_string_lossy().to_string()]);
    }

    #[test]
    fn test_resolve_script_command_real_file_with_extra_args() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("app.ts");
        std::fs::write(&script, "").unwrap();

        let cmd = vec![
            script.to_string_lossy().to_string(),
            "--port".to_string(),
            "3000".to_string(),
        ];
        let tools = vec![crate::cli::ToolSpec {
            name: "deno".to_string(),
            version: None,
        }];
        let (program, args) = super::resolve_script_command(&cmd, &tools);
        assert_eq!(program, "deno");
        assert_eq!(
            args,
            vec!["run", &script.to_string_lossy(), "--port", "3000"]
        );
    }

    #[test]
    fn test_resolve_script_command_multiple_tools_no_inference() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("test.js");
        std::fs::write(&script, "").unwrap();

        let cmd = vec![script.to_string_lossy().to_string()];
        let tools = vec![
            crate::cli::ToolSpec {
                name: "node".to_string(),
                version: None,
            },
            crate::cli::ToolSpec {
                name: "python".to_string(),
                version: None,
            },
        ];
        let (program, _) = super::resolve_script_command(&cmd, &tools);
        // Multiple tools — can't infer, use file as-is
        assert_eq!(program, script.to_string_lossy());
    }

    #[test]
    fn test_tool_interpreter_aliases() {
        // Verify all aliases map correctly
        assert_eq!(super::tool_interpreter("nodejs").unwrap(), vec!["node"]);
        assert_eq!(super::tool_interpreter("python3").unwrap(), vec!["python3"]);
        assert_eq!(
            super::tool_interpreter("golang").unwrap(),
            vec!["go", "run"]
        );
        assert_eq!(super::tool_interpreter("bunx").unwrap(), vec!["bun", "run"]);
    }
}

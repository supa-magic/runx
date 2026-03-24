mod cli;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

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
                eprintln!("Error: No command specified. Use -- to separate the command.");
                eprintln!("Example: runx --with node@18 -- node -v");
                std::process::exit(1);
            }

            if cli.tools.is_empty() {
                eprintln!("Error: No tools specified. Use --with to add tools.");
                eprintln!("Example: runx --with node@18 -- node -v");
                std::process::exit(1);
            }

            println!("run: tools={:?}, cmd={:?}", cli.tools, cli.cmd);
            println!(
                "  verbose={}, dry_run={}, inherit_env={}, quiet={}",
                cli.verbose, cli.dry_run, cli.inherit_env, cli.quiet
            );
        }
    }
}

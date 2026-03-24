# runx

Ephemeral environment runner -- run any command with specific tool versions without global installs. Written in Rust (edition 2024).

**Status:** All phases complete. Core CLI, 5 providers (Node.js, Python, Go, Deno, Bun), .runxrc config, lockfile, cache management, global install, auto-update, plugin system, shell completions, shebang runner, actionable error messages.

## Build & Test Commands

```bash
cargo build                              # build
cargo run                                # run
cargo fmt --check                        # check formatting
cargo clippy --all-targets --all-features # lint
cargo test                               # all tests (463 passing)
cargo test --doc                         # doctests only
cargo check                              # type check without building
```

Run all three checks before considering any task complete:
`cargo fmt --check && cargo clippy --all-targets --all-features && cargo test`

## Project Structure

```
src/
  main.rs              # Entry point, tokio async main, CLI dispatch
  cli.rs               # CLI argument parsing (clap): Cli, Command, ToolSpec, HumanDuration
  clean.rs             # Clean command: cache removal with filtering, confirmation, disk space reporting
  config.rs            # .runxrc TOML config file discovery and parsing (walks parent directories)
  init.rs              # Init command: interactive/non-interactive .runxrc scaffolding with comments
  install.rs           # Install/uninstall commands: global symlinks to ~/.runx/bin/ (.cmd shims on Windows)
  list.rs              # List command: supported tools, cached versions, upstream version queries
  lockfile.rs          # Lockfile (.runxrc.lock): version pinning for reproducible builds
  plugin.rs            # Plugin system: declarative TOML provider manifests in ~/.runx/plugins/
  run.rs               # Command dispatch: run_command workflow (resolve, download, build env, execute)
  update.rs            # Update command: check cached tools for newer patch versions
  error.rs             # RunxError enum (thiserror) aggregating all subsystem errors
  version.rs           # VersionSpec (Latest, Major, MajorMinor, Exact) with matching and resolution
  platform.rs          # Platform, Arch, Target enums with runtime detection and cross-platform helpers
  cache.rs             # Cache management at ~/.runx/cache/ (install, lookup, clean, list)
  download.rs          # HTTP streaming download, SHA256 verification, archive extraction (tar.gz, tar.xz, zip)
  environment.rs       # Isolated/inherited environment construction, TempDirs RAII guard
  executor.rs          # Child process spawning, signal forwarding (SIGTERM on Unix), exit code handling
  provider/
    mod.rs             # Provider trait, get_provider() dispatch, ProviderError, ArchiveFormat
    node.rs            # Node.js provider (nodejs.org dist index)
    python.rs          # Python provider (python-build-standalone GitHub releases)
    go.rs              # Go provider (go.dev official binary distributions)
    deno.rs            # Deno provider (GitHub releases)
    bun.rs             # Bun provider (GitHub releases)
    ruby.rs            # Ruby provider (ruby-builder GitHub releases)
    java.rs            # Java provider (Eclipse Adoptium/Temurin API)
    rust.rs            # Rust provider (rust-lang GitHub releases + post-install)
docs/prds/
  runx.md              # Product requirements document
```

## Supported Tools

| Tool | Aliases | Version Source |
|------|---------|---------------|
| Node.js | `node`, `nodejs` | `https://nodejs.org/dist/index.json` |
| Python | `python`, `python3` | `https://api.github.com/repos/indygreg/python-build-standalone/releases` |
| Go | `go`, `golang` | `https://go.dev/dl/?mode=json` |
| Deno | `deno` | `https://api.github.com/repos/denoland/deno/releases` |
| Bun | `bun`, `bunx` | `https://api.github.com/repos/oven-sh/bun/releases` |
| Ruby | `ruby`, `rb` | `https://api.github.com/repos/ruby/ruby-builder/releases` |
| Java | `java`, `jdk` | `https://api.adoptium.net/v3/info/available_releases` |
| Rust | `rust`, `rustc`, `cargo` | `https://api.github.com/repos/rust-lang/rust/releases` |

## CLI Flags

```
runx [OPTIONS] [--with <TOOL@VERSION>...] -- <command> [args...]
runx install [TOOL@VERSION...]
runx install --list
runx uninstall <TOOL>
runx clean [-y] [--tool <NAME>] [--older-than <DURATION>]
runx list [--cached] [tool]
runx init [--with <TOOL@VERSION>...] [--force]
runx lock [--update]
runx update [tool]
runx plugin list|add|remove [arg]
runx completions bash|zsh|fish
```

| Flag | Short | Description |
|------|-------|-------------|
| `--with <TOOL@VERSION>` | | Tool to include (repeatable) |
| `--verbose` | `-v` | Show download progress and debug output |
| `--quiet` | `-q` | Suppress progress output |
| `--dry-run` | | Show what would happen without doing it |
| `--inherit-env` | | Inherit user's full environment |

### Clean Command

| Usage | Description |
|-------|-------------|
| `runx clean` | Remove all cached binaries (with confirmation prompt) |
| `runx clean --tool node` | Remove only Node.js caches |
| `runx clean --older-than 30d` | Remove caches older than 30 days |
| `runx clean -y` | Skip confirmation prompt |
| `runx --dry-run clean` | Show what would be deleted and disk space without deleting |

### List Command

| Usage | Description |
|-------|-------------|
| `runx list` | Show all 5 supported tool providers with aliases and cache status |
| `runx list --cached` | Show cached tool versions with disk sizes |
| `runx list <tool>` | Query upstream and show available versions for a specific tool |

### Init Command

| Usage | Description |
|-------|-------------|
| `runx init` | Interactive tool selection, creates `.runxrc` with comments |
| `runx init --with node@18 --with python@3.11` | Non-interactive mode, creates `.runxrc` with specified tools |
| `runx init --force` | Overwrite existing `.runxrc` |

### Install Command

| Usage | Description |
|-------|-------------|
| `runx install node@22` | Download and symlink to `~/.runx/bin/` |
| `runx install --list` | Show globally installed tools |
| `runx install` | Install tools from `.runxrc` |

### Uninstall Command

| Usage | Description |
|-------|-------------|
| `runx uninstall node` | Remove symlinks from `~/.runx/bin/` |

> On Windows, `.cmd` shims are created instead of symlinks.

### Lock Command

| Usage | Description |
|-------|-------------|
| `runx lock` | Resolve `.runxrc` tools and write `.runxrc.lock` |
| `runx lock --update` | Re-resolve and update existing lockfile |

### Update Command

| Usage | Description |
|-------|-------------|
| `runx update` | Check all cached tools for newer patch versions |
| `runx update node` | Update a specific tool only |

### Plugin Command

| Usage | Description |
|-------|-------------|
| `runx plugin list` | Show installed plugins |
| `runx plugin add <path>` | Install a plugin from a `.toml` manifest |
| `runx plugin remove <name>` | Remove a plugin |

### Completions Command

| Usage | Description |
|-------|-------------|
| `runx completions bash` | Output shell completions for Bash |
| `runx completions zsh` | Output shell completions for Zsh |
| `runx completions fish` | Output shell completions for Fish |

## Rules, Skills & Agents

- **Rules** (`.claude/rules/rust.md`): Always-on Rust conventions -- error handling, safety, style, dependencies.
- **Skills** (`.claude/skills/`): `rust-engineer`, `skill`, `cleanup`, `github`, `dev`, `git`, `autopilot`, `audit`
- **Agents** (`.claude/agents/`):
  - `rust-reviewer` -- read-only code review (clippy, safety, idioms)
  - `rust-debugger` -- diagnoses compiler errors, borrow checker, async issues
  - `rust-tester` -- runs tests, analyzes failures, writes missing tests
  - `rust-refactorer` -- ownership cleanup, clone removal, clippy fixes
  - `docs-updater` -- keeps documentation in sync with codebase changes

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` 4 | CLI argument parsing (derive) |
| `tokio` 1 | Async runtime |
| `reqwest` 0.12 | HTTP client (streaming + blocking) |
| `serde` / `serde_json` | JSON parsing for version APIs |
| `semver` 1 | Version parsing and comparison |
| `flate2` + `tar` | .tar.gz extraction |
| `zip` 4 | .zip extraction |
| `indicatif` 0.17 | Progress bars |
| `sha2` 0.10 | SHA256 checksum verification |
| `tempfile` 3 | Atomic cache writes + temp directories |
| `dirs` 6 | Cross-platform home directory resolution |
| `ctrlc` 3 | Signal handling |
| `thiserror` 2 | Error derive macros |
| `toml` | TOML config file parsing (.runxrc) |
| `clap_complete` 4 | Shell completion generation (bash, zsh, fish) |
| `tokio-util` 0.7 | Async I/O utilities |
| `futures-util` 0.3 | Async stream utilities |
| `libc` 0.2 | Unix signal forwarding (Unix only) |

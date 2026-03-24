# runx

A cross-platform CLI tool that creates isolated, ephemeral environments with specific tool versions, runs a command, and exits. Like npx, but for any tool and with full environment isolation.

No daemon, no persistent containers, no impact on your existing tool installations.

## Features

- Run any command with pinned tool versions in a single invocation
- Automatic download and caching of tool binaries
- Full environment isolation -- your system tools are never affected
- Cross-platform: macOS (arm64, x86_64), Linux (x86_64, arm64), Windows (x86_64)
- Sub-second startup for cached tools

## Supported Tools

| Tool | Spec examples | Version source |
|------|---------------|----------------|
| **Node.js** | `node@18`, `node@20.11.0`, `node` | nodejs.org dist index |
| **Python** | `python@3.11`, `python@3.12.1`, `python3` | python-build-standalone (GitHub) |
| **Go** | `go@1.21`, `go@1.22.0`, `golang` | go.dev official downloads |
| **Deno** | `deno@1.40`, `deno@2.0.0`, `deno` | GitHub releases (denoland/deno) |
| **Bun** | `bun@1.1`, `bun@1.2.0`, `bunx` | GitHub releases (oven-sh/bun) |

## Installation

```bash
# Clone and build from source
git clone <repo-url> && cd runx
cargo build --release

# Binary will be at target/release/runx
```

## Usage

### Run a command with a specific tool version

```bash
# Run node -v with Node.js 18
runx --with node@18 -- node -v

# Run a Python script with Python 3.11
runx --with python@3.11 -- python3 script.py

# Run a Go program with Go 1.21
runx --with go@1.21 -- go run main.go

# Run a Deno script with Deno 2.0
runx --with deno@2.0 -- deno run server.ts

# Run a Bun script with Bun 1.2
runx --with bun@1.2 -- bun run index.ts
```

### Use multiple tools together

```bash
runx --with node@20 --with python@3.11 -- node process.js
```

### Version specifiers

```bash
runx --with node@18          -- node -v   # Latest 18.x.x
runx --with node@18.19       -- node -v   # Latest 18.19.x
runx --with node@18.19.1     -- node -v   # Exact version
runx --with node              -- node -v   # Latest stable
```

### Dry run

```bash
runx --with node@18 --dry-run -- node -v
# Shows what would be downloaded and executed without doing it
```

### Inherit your shell environment

By default, runx creates a clean, isolated environment. Use `--inherit-env` to pass through your existing environment variables:

```bash
runx --with node@18 --inherit-env -- node -v
```

### Cache management

```bash
runx list                          # Show supported tools
runx list --cached                 # Show cached tool versions with sizes
runx list node                     # Show info for a specific tool
runx clean                         # Remove all cached binaries
runx clean --tool node             # Remove only Node.js caches
runx clean --older-than 30d        # Remove caches older than 30 days
runx init                          # Scaffold a .runxrc config file
```

## How It Works

1. **Resolve** -- Queries upstream version APIs to resolve `node@18` to an exact version like `18.19.1`
2. **Download** -- Downloads the binary archive to a temp directory, verifies checksums, extracts it
3. **Cache** -- Moves the extracted binaries to `~/.runx/cache/<tool>/<version>/<platform>/`
4. **Environment** -- Builds an isolated environment with only the tool's binaries on PATH, plus minimal system paths (`/usr/bin`, `/bin`)
5. **Execute** -- Spawns the command as a child process with the constructed environment, forwards signals, and exits with the child's exit code

On subsequent runs with the same tool version, steps 2-3 are skipped and the cached binaries are used directly.

## Environment Isolation

By default, runx constructs a "clean room" environment for the child process:

| Category | Variables | Behavior |
|----------|-----------|----------|
| **Inherited** | `HOME`, `USER`, `TERM`, `LANG`, `SHELL`, `TMPDIR`, `LC_*`, `XDG_*` | Always passed through |
| **Constructed** | `PATH`, `NODE_HOME`, `PYTHONHOME`, `GOROOT` | Set by runx based on tool locations |
| **Blocked** | User's `PATH`, `NVM_DIR`, `PYENV_ROOT`, etc. | Not inherited (isolation) |

With `--inherit-env`, the full user environment is kept and tool paths are prepended to PATH.

## Architecture

```
src/
  main.rs          Entry point (tokio async)
  cli.rs           CLI parsing (clap derive)
  run.rs           Orchestration: resolve -> download -> env -> execute
  cache.rs         ~/.runx/cache/ management
  download.rs      Streaming HTTP download + archive extraction
  environment.rs   Isolated environment construction
  executor.rs      Child process spawning + signal forwarding
  platform.rs      OS/arch detection
  version.rs       Semver version resolution
  error.rs         Error types
  provider/
    mod.rs         Provider trait
    node.rs        Node.js provider
    python.rs      Python provider
    go.rs          Go provider
    deno.rs        Deno provider
    bun.rs         Bun provider
```

## License

See LICENSE file for details.

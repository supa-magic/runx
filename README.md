# runx

Ephemeral environment runner — run any command with specific tool versions, without installing anything globally.

Like `npx`, but for **any** runtime. No daemon, no containers, no impact on your system.

```bash
runx --with node@18 -- node -v       # v18.20.8
runx --with python@3.12 -- python3 -c "print('hello')"
runx --with go@1 -- go version
```

## Highlights

- **5 runtimes** — Node.js, Python, Go, Deno, Bun
- **Version pinning** — major (`@18`), minor (`@18.19`), or exact (`@18.19.1`)
- **Isolated environments** — your system PATH is never touched
- **Automatic caching** — first run downloads, subsequent runs are instant
- **Parallel downloads** — multiple `--with` flags download concurrently
- **Project config** — `.runxrc` files so your team uses the same versions
- **Cross-platform** — macOS, Linux, Windows (x64 and ARM64)

## Install

**Prebuilt binaries** — download from [Releases](https://github.com/supa-magic/runx/releases/latest):

```bash
# macOS (Apple Silicon)
curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-aarch64-apple-darwin.tar.gz | tar xz
sudo mv runx /usr/local/bin/

# macOS (Intel)
curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-x86_64-apple-darwin.tar.gz | tar xz
sudo mv runx /usr/local/bin/

# Linux (x64)
curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv runx /usr/local/bin/

# Linux (ARM64)
curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-aarch64-unknown-linux-gnu.tar.gz | tar xz
sudo mv runx /usr/local/bin/
```

**From source:**

```bash
git clone https://github.com/supa-magic/runx.git && cd runx
cargo build --release
# Binary at target/release/runx
```

## Quick Start

```bash
# Run a command with a specific tool version
runx --with node@18 -- node -v

# Use multiple tools together
runx --with node@20 --with python@3.12 -- node process.js

# Latest stable version (omit version)
runx --with deno -- deno --version

# See what would happen without doing it
runx --dry-run --with go@1 -- go version
```

## Supported Tools

| Tool | Spec examples | Source |
|------|---------------|--------|
| **Node.js** | `node@18`, `node@20.11.0`, `nodejs` | nodejs.org |
| **Python** | `python@3.12`, `python@3.12.1`, `python3` | python-build-standalone |
| **Go** | `go@1`, `go@1.22.0`, `golang` | go.dev |
| **Deno** | `deno@2`, `deno@2.0.0` | GitHub releases |
| **Bun** | `bun@1`, `bun@1.2.0`, `bunx` | GitHub releases |

### Version specifiers

```bash
runx --with node@22         -- node -v   # Latest 22.x.x
runx --with node@22.11      -- node -v   # Latest 22.11.x
runx --with node@22.11.0    -- node -v   # Exact version
runx --with node             -- node -v   # Latest stable
```

## Project Configuration

Create a `.runxrc` file in your project root so everyone on the team uses the same tool versions:

```toml
# .runxrc
tools = ["node@18", "python@3.12"]
inherit_env = false
```

Then just run commands — no `--with` flags needed:

```bash
runx -- node -v          # Uses node@18 from .runxrc
runx -- python3 -c "print('hi')"  # Uses python@3.12 from .runxrc
```

### How config works

- **Auto-discovered** — runx searches the current directory, then walks up parent directories
- **CLI overrides config** — `--with` flags replace config tools entirely
- **`inherit_env`** — set to `true` to pass through your shell environment (default: isolated)
- **Visibility** — use `--dry-run` or `--verbose` to see which config file was loaded

### Scaffold a config

```bash
runx init                                    # Interactive — prompts for tools
runx init --with node@18 --with python@3.12  # Non-interactive
runx init --force                            # Overwrite existing .runxrc
```

The generated file includes comments explaining each option.

## Environment Isolation

By default, runx creates a clean environment for the child process:

| Inherited | Constructed | Blocked |
|-----------|-------------|---------|
| `HOME`, `USER`, `TERM`, `LANG`, `SHELL`, `TMPDIR`, `LC_*`, `XDG_*` | `PATH`, `NODE_HOME`, `PYTHONHOME`, `GOROOT` | User's `PATH`, `NVM_DIR`, `PYENV_ROOT`, etc. |

Use `--inherit-env` to keep your full shell environment (tool paths are prepended to PATH):

```bash
runx --with node@18 --inherit-env -- node -v
```

## Cache Management

Tools are cached at `~/.runx/cache/`. Manage with:

```bash
runx list                      # Show supported tools and cache status
runx list --cached             # Show cached versions with disk sizes
runx list node                 # Query upstream for available versions
runx clean                     # Remove all cached binaries
runx clean --tool node         # Remove only Node.js caches
runx clean --older-than 30d    # Remove stale caches
```

## Shell Completions

```bash
# Bash
eval "$(runx completions bash)"

# Zsh
eval "$(runx completions zsh)"

# Fish
runx completions fish | source
```

## Script Runner / Shebang

Scripts can use runx directly in their shebang line, so they run with the right tool version automatically — no wrapper scripts needed.

### Shebang usage

```bash
#!/usr/bin/env -S runx --with node@22 --
console.log("Hello from Node 22!");
```

```bash
#!/usr/bin/env -S runx --with python@3.12 --
print("Hello from Python 3.12!")
```

```bash
#!/usr/bin/env -S runx --with deno@2 --
console.log("Hello from Deno 2!");
```

Make the script executable and run it directly:

```bash
chmod +x script.js
./script.js --flag arg1 arg2    # Args are passed through to the script
```

### How it works

When `cmd[0]` is a file path, runx auto-detects the interpreter from the `--with` flag and runs the file with it. This works with all 5 supported tools:

| `--with` | Interpreter |
|----------|-------------|
| `node@*` | `node` |
| `python@*` | `python3` |
| `go@*` | `go run` |
| `deno@*` | `deno run` |
| `bun@*` | `bun` |

Script arguments are passed through unchanged.

## How It Works

1. **Resolve** — queries upstream APIs to find the exact version (e.g., `node@18` → `18.20.8`)
2. **Download** — streams the binary archive, verifies checksums, extracts it
3. **Cache** — stores binaries at `~/.runx/cache/<tool>/<version>/<platform>/`
4. **Isolate** — builds a clean environment with only the tool on PATH
5. **Execute** — spawns the command, forwards signals, exits with the child's code

Cached tools skip steps 2–3, so repeat runs start instantly.

## CLI Reference

```
runx [OPTIONS] [-- <CMD>...]
runx <COMMAND>

Commands:
  list         List available tools and cached versions
  clean        Remove cached tool binaries
  init         Scaffold a .runxrc config file
  completions  Generate shell completions (bash, zsh, fish)

Options:
  --with <TOOL@VERSION>  Tool to include (repeatable)
  --dry-run              Show what would happen without doing it
  --inherit-env          Pass through the user's full environment
  -v, --verbose          Show download progress and debug output
  -q, --quiet            Suppress progress output
  -V, --version          Print version
```

## License

See [LICENSE](LICENSE) file for details.

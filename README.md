<p align="center">
  <h1 align="center">runx</h1>
  <p align="center">
    <strong>Run any command with the exact tool version you need. No installs. No conflicts. No cleanup.</strong>
  </p>
  <p align="center">
    <a href="https://github.com/supa-magic/runx/releases/latest"><img src="https://img.shields.io/github/v/release/supa-magic/runx?style=flat-square&color=blue" alt="Release"></a>
    <a href="https://github.com/supa-magic/runx/actions"><img src="https://img.shields.io/github/actions/workflow/status/supa-magic/runx/ci.yml?style=flat-square" alt="CI"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License"></a>
  </p>
</p>

---

```bash
$ runx --with node@22 -- node -v
v22.22.1

$ runx --with python@3.12 -- python3 -c "import sys; print(sys.version)"
3.12.13

$ runx --with go@1 -- go version
go version go1.26.1 darwin/arm64
```

**runx** downloads the exact tool version you ask for, runs your command in an isolated environment, and exits. Nothing is installed globally. Nothing touches your system PATH. If the tool was already downloaded, it starts instantly from cache.

Think of it as **npx for any runtime**.

---

## Why runx?

| Problem | Without runx | With runx |
|---------|-------------|-----------|
| "Works on my machine" | Everyone installs different versions | `.runxrc` pins exact versions for the team |
| Testing across versions | Manually install/switch with nvm, pyenv | `runx --with node@18 -- npm test` then `runx --with node@22 -- npm test` |
| CI/CD tool setup | 10 lines of YAML per tool | `runx --with node@22 -- npm run build` |
| Trying a new runtime | Install Deno globally, clean up later | `runx --with deno -- deno run server.ts` |
| Onboarding new devs | "Install Node 18, Python 3.12, Go 1.22..." | `git clone && runx -- npm start` |

## Quick Start

**Install** (one-time):

```bash
# macOS
curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-aarch64-apple-darwin.tar.gz | tar xz
sudo mv runx /usr/local/bin/

# Linux
curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv runx /usr/local/bin/
```

<details>
<summary>Other platforms</summary>

```bash
# macOS (Intel)
curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-x86_64-apple-darwin.tar.gz | tar xz

# Linux (ARM64)
curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-aarch64-unknown-linux-gnu.tar.gz | tar xz

# Windows — download from GitHub Releases
# https://github.com/supa-magic/runx/releases/latest
```

**From source:**
```bash
cargo install --git https://github.com/supa-magic/runx.git
```

</details>

**Run:**

```bash
runx --with node@22 -- node -v
```

That's it. Node 22 is downloaded (once), cached, and your command runs in an isolated environment.

## Supported Runtimes

| Runtime | Examples | Version source |
|---------|----------|----------------|
| **Node.js** | `node@18` `node@20.11.0` `nodejs` | nodejs.org |
| **Python** | `python@3.12` `python@3.12.1` `python3` | python-build-standalone |
| **Go** | `go@1` `go@1.22.0` `golang` | go.dev |
| **Deno** | `deno@2` `deno@2.0.0` | GitHub releases |
| **Bun** | `bun@1` `bun@1.2.0` `bunx` | GitHub releases |

Need another runtime? runx has a [plugin system](#plugins) — add any tool with a simple TOML file.

### Version pinning

```bash
runx --with node@22         -- node -v   # Latest 22.x.x (e.g., 22.22.1)
runx --with node@22.11      -- node -v   # Latest 22.11.x
runx --with node@22.11.0    -- node -v   # Exact version
runx --with node             -- node -v   # Latest stable
```

### Multiple tools

```bash
runx --with node@22 --with python@3.12 -- node -e "console.log('ready')"
```

Tools download in parallel.

## Team Configuration

Commit a `.runxrc` to your repo so every developer and CI job uses the same versions:

```toml
# .runxrc — commit this to your repo
tools = ["node@22", "python@3.12"]
```

Now everyone just runs:

```bash
runx -- npm start           # Uses node@22 from .runxrc
runx -- python3 manage.py   # Uses python@3.12 from .runxrc
```

No `--with` flags needed. No "did you install the right version?" questions.

```bash
# Scaffold a new config interactively
runx init

# Or non-interactively
runx init --with node@22 --with python@3.12
```

<details>
<summary>How config discovery works</summary>

- runx searches the current directory, then walks up parent directories (like `.gitignore`)
- CLI `--with` flags override the config entirely
- `inherit_env = true` passes your full shell environment through (default: isolated)
- Use `--dry-run` or `--verbose` to see which config file was loaded

</details>

### Lockfile for reproducibility

Pin exact versions + download URLs for fully reproducible builds:

```bash
runx lock                    # Resolve .runxrc → .runxrc.lock
runx lock --update           # Re-resolve and update
```

Commit `.runxrc.lock` — when it exists, runx skips version resolution entirely.

## Self-Contained Scripts

Scripts can declare their own runtime with a shebang line:

```js
#!/usr/bin/env -S runx --with node@22 --
console.log("Hello! I bring my own Node.js.");
console.log("Just chmod +x and run me.");
```

```python
#!/usr/bin/env -S runx --with python@3.12 --
import sys
print(f"Running Python {sys.version}")
```

```bash
chmod +x script.js
./script.js              # Downloads Node 22 if needed, runs the script
./script.js --port 3000  # Arguments pass through
```

No Dockerfile. No `package.json`. Just a script that works everywhere runx is installed.

## Global Install

Use runx as a lightweight version manager:

```bash
runx install node@22         # Symlink to ~/.runx/bin/node
runx install python@3.12     # Symlink to ~/.runx/bin/python3
node -v                      # v22.22.1 (from ~/.runx/bin)

runx install --list          # Show what's installed
runx uninstall node          # Remove it

# Install everything from .runxrc at once
runx install
```

Add to your shell profile once:
```bash
export PATH="$HOME/.runx/bin:$PATH"
```

## Plugins

Add any tool with a TOML manifest:

```toml
# ~/.runx/plugins/zig.toml
name = "zig"
download_url = "https://ziglang.org/builds/zig-{os}-{arch}-{version}.tar.xz"
archive_format = "tar.xz"
bin_path = "zig-{os}-{arch}-{version}"
```

```bash
runx plugin add ./zig.toml              # Install the plugin
runx --with zig@0.11.0 -- zig version   # Use it like any built-in tool
runx plugin list                         # Show installed plugins
runx plugin remove zig                   # Uninstall
```

## Environment Isolation

By default, runx builds a **clean-room environment** for every command:

| Inherited | Constructed | Blocked |
|-----------|-------------|---------|
| `HOME`, `USER`, `TERM`, `LANG`, `SHELL`, `TMPDIR` | `PATH` (tool bins + `/usr/bin`) | Your `PATH`, `NVM_DIR`, `PYENV_ROOT`, etc. |

Your system tools are never visible to the child process. This prevents version conflicts, leaked env vars, and "works on my machine" bugs.

Use `--inherit-env` to pass your full environment through when needed.

## Cache & Updates

```bash
runx list                      # Supported tools and cache status
runx list --cached             # What's downloaded and how much disk space
runx list node                 # Available versions from upstream
runx clean                     # Free disk space
runx clean --older-than 30d    # Remove only stale versions
runx update                    # Update cached tools to latest patches
```

Tools are stored at `~/.runx/cache/` and reused instantly on repeat runs.

## Shell Completions

```bash
eval "$(runx completions bash)"   # Bash
eval "$(runx completions zsh)"    # Zsh
runx completions fish | source    # Fish
```

## How It Works

```
runx --with node@22 -- node server.js
     │                  │
     ▼                  ▼
  1. Resolve         5. Execute
     node@22 →          node server.js
     22.22.1            in isolated env
     │
  2. Download (or skip if cached)
     │
  3. Cache at ~/.runx/cache/node/22.22.1/
     │
  4. Build clean environment
     PATH = [node bin] + [/usr/bin, /bin]
```

Steps 2–3 are skipped for cached tools — repeat runs start in milliseconds.

## CLI Reference

```
runx [OPTIONS] [-- <CMD>...]    Run a command with specified tools
runx install [TOOL@VERSION]     Install tools globally (~/.runx/bin/)
runx uninstall <TOOL>           Remove globally installed tools
runx list [--cached] [TOOL]     List tools, versions, cache status
runx clean [--tool] [--older-than]  Free disk space
runx init [--with TOOL@VER]     Create a .runxrc config file
runx lock [--update]            Generate .runxrc.lock for reproducibility
runx update [TOOL]              Update cached tools to latest patch
runx plugin list|add|remove     Manage custom tool providers
runx completions bash|zsh|fish  Shell tab-completion scripts

Options:
  --with <TOOL@VERSION>   Tool to include (repeatable)
  --dry-run               Preview without executing
  --inherit-env           Keep your full shell environment
  -v, --verbose           Show download progress
  -q, --quiet             Suppress output
  -V, --version           Print version
```

## Contributing

```bash
git clone https://github.com/supa-magic/runx.git && cd runx
cargo test                    # 338 tests
cargo clippy                  # Zero warnings policy
cargo fmt --check             # Enforced formatting
```

## License

[MIT](LICENSE)

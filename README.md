<p align="center">
  <h1 align="center">runx</h1>
  <p align="center">
    <strong>One command. Any runtime. Exact version. Zero installs.</strong>
  </p>
  <p align="center">
    <a href="https://github.com/supa-magic/runx/releases/latest"><img src="https://img.shields.io/github/v/release/supa-magic/runx?style=flat-square&color=blue" alt="Release"></a>
    <a href="https://github.com/supa-magic/runx/actions"><img src="https://img.shields.io/github/actions/workflow/status/supa-magic/runx/ci.yml?style=flat-square" alt="CI"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License"></a>
    <img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square" alt="Platforms">
  </p>
</p>

```bash
$ runx --with node@22 -- node -v
v22.22.1
```

That's it. Node 22 was downloaded, cached, and your command ran in a fully isolated environment. Your system wasn't touched. Next time, it starts instantly.

**runx** is a single binary that replaces nvm, pyenv, goenv, and a dozen YAML lines in your CI — for **Node.js, Python, Go, Deno, and Bun** (and [any tool via plugins](#-plugins)).

---

## Why developers love runx

> **"Install Node 18, Python 3.12, and Go 1.22. Oh, and make sure it's the same versions as production."**

Sound familiar? Here's how runx eliminates that:

```bash
# Test your app against Node 18 AND Node 22 — no nvm, no switching
runx --with node@18 -- npm test
runx --with node@22 -- npm test

# Run a Python script without touching your system Python
runx --with python@3.12 -- python3 train_model.py

# Try Deno without installing it
runx --with deno -- deno run https://examples.deno.land/hello-world.ts

# Use multiple runtimes together — they download in parallel
runx --with node@22 --with python@3.12 -- node orchestrate.js
```

| Problem | Before runx | With runx |
|---------|-------------|-----------|
| "Works on my machine" | Everyone has different versions | `.runxrc` pins versions for the whole team |
| Testing across versions | `nvm use 18`, test, `nvm use 22`, test | `runx --with node@18 -- npm test` |
| CI/CD tool setup | `actions/setup-node@v4` + `actions/setup-python@v5` + ... | `runx --with node@22 -- npm run build` |
| Onboarding a new dev | "Follow these 12 setup steps..." | `git clone && runx -- npm start` |
| Trying a new runtime | Install globally, use once, forget to uninstall | `runx --with bun -- bun run index.ts` |
| Environment pollution | `NVM_DIR`, `PYENV_ROOT` leaking everywhere | runx builds a clean-room env every time |

---

## Install

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

# Windows — download .zip from Releases
# https://github.com/supa-magic/runx/releases/latest
```

<details>
<summary>Build from source</summary>

```bash
# Requires Rust toolchain
cargo install --git https://github.com/supa-magic/runx.git
```

</details>

Single binary. No dependencies. No Node.js or Python required to run runx itself.

---

## Features

### Supported runtimes

| Runtime | Examples | Version source |
|---------|----------|----------------|
| **Node.js** | `node@18` `node@20.11.0` `nodejs` | nodejs.org |
| **Python** | `python@3.12` `python@3.12.1` `python3` | python-build-standalone |
| **Go** | `go@1` `go@1.22.0` `golang` | go.dev |
| **Deno** | `deno@2` `deno@2.0.0` | GitHub releases |
| **Bun** | `bun@1` `bun@1.2.0` `bunx` | GitHub releases |
| **Any tool** | via [plugins](#-plugins) | configurable |

### Smart version pinning

```bash
runx --with node@22         -- node -v   # Latest 22.x.x → 22.22.1
runx --with node@22.11      -- node -v   # Latest 22.11.x
runx --with node@22.11.0    -- node -v   # Exact version
runx --with node             -- node -v   # Latest stable
```

### Parallel downloads

```bash
# Downloads Node AND Python at the same time
runx --with node@22 --with python@3.12 -- node process.js
```

### Full environment isolation

Every command runs in a **clean-room environment** — your system PATH, `NVM_DIR`, `PYENV_ROOT`, and other tool managers are invisible:

| Always inherited | Constructed by runx | Blocked |
|-----------------|--------------------|---------|
| `HOME`, `USER`, `TERM`, `LANG`, `SHELL`, `TMPDIR` | `PATH` = tool bins + `/usr/bin` | Your `PATH`, `NVM_DIR`, `PYENV_ROOT`, etc. |

This means no version conflicts, no leaked env vars, and no "works on my machine" surprises.

Need your full environment? Add `--inherit-env`.

### Instant caching

First run downloads the tool. Every subsequent run starts in **milliseconds** from cache:

```bash
$ time runx --with node@22 -- node -e "console.log('fast')"
# First run: ~3s (download + extract)
# Cached run: ~0.1s
```

### Preview mode

```bash
runx --dry-run --with node@22 -- node -v
# Shows what would be downloaded and executed, without doing it
```

---

## Team Configuration

Stop asking "which version do I need?" — commit a `.runxrc` file:

```toml
# .runxrc — commit this to your repo
tools = ["node@22", "python@3.12"]
```

Now every developer and CI job runs:

```bash
runx -- npm start              # Picks up node@22 from .runxrc
runx -- python3 manage.py      # Picks up python@3.12 from .runxrc
```

**No flags. No setup. Same versions everywhere.**

```bash
runx init                                    # Interactive setup wizard
runx init --with node@22 --with python@3.12  # Non-interactive
```

<details>
<summary>Config details</summary>

- Auto-discovered by walking up parent directories (like `.gitignore`)
- CLI `--with` flags override the config entirely
- `inherit_env = true` passes your full shell environment through
- `--dry-run` and `--verbose` show which config file was loaded

</details>

### Lockfile for CI reproducibility

```bash
runx lock                    # Resolve .runxrc → .runxrc.lock (exact versions + URLs)
runx lock --update           # Re-resolve and update
```

When `.runxrc.lock` exists, runx skips version resolution entirely — same binary, every time. Commit it alongside `.runxrc`.

---

## Self-Contained Scripts

The killer feature: scripts that **bring their own runtime**.

```js
#!/usr/bin/env -S runx --with node@22 --
// This script runs with Node 22 — anywhere runx is installed.
// No package.json. No Dockerfile. Just chmod +x and go.

const http = require("http");
http.createServer((_, res) => res.end("Hello!")).listen(3000);
console.log("Server running on :3000");
```

```python
#!/usr/bin/env -S runx --with python@3.12 --
# This script runs with Python 3.12 — no virtualenv needed.
import sys
print(f"Python {sys.version}")
```

```bash
chmod +x server.js
./server.js                  # Downloads Node 22 if needed, starts the server
./server.js --port 8080      # Arguments pass through
```

Share scripts with your team, drop them in CI, hand them to clients — they just work.

---

## Global Install

Use runx as a **lightweight version manager** when you want tools permanently available:

```bash
runx install node@22              # Symlink → ~/.runx/bin/node
runx install python@3.12          # Symlink → ~/.runx/bin/python3
runx install                      # Install everything from .runxrc

node -v                           # v22.22.1
python3 --version                 # Python 3.12.13

runx install --list               # See what's installed
runx uninstall node               # Remove it
runx update                       # Update all to latest patches
runx update node                  # Update just Node.js
```

Add to your shell profile once:
```bash
export PATH="$HOME/.runx/bin:$PATH"
```

---

## Plugins

Add **any tool** — not just the 5 built-in runtimes — with a simple TOML file:

```toml
# ~/.runx/plugins/zig.toml
name = "zig"
aliases = ["ziglang"]
description = "Zig programming language"
download_url = "https://ziglang.org/builds/zig-{os}-{arch}-{version}.tar.xz"
archive_format = "tar.xz"
bin_path = "zig-{os}-{arch}-{version}"
```

```bash
runx plugin add ./zig.toml
runx --with zig@0.11.0 -- zig version   # Works like any built-in tool

runx plugin list                          # See installed plugins
runx plugin remove zig                    # Uninstall
```

Placeholders `{version}`, `{os}`, `{arch}`, `{triple}`, `{os_alt}`, `{arch_alt}` are expanded automatically. Share plugin files with your team or the community.

---

## Cache Management

```bash
runx list                      # See all tools and what's cached
runx list --cached             # Cached versions with disk sizes
runx list node                 # Query upstream for available versions

runx clean                     # Remove everything (with confirmation)
runx clean --tool node         # Remove only Node.js caches
runx clean --older-than 30d    # Remove stale versions
runx clean -y                  # Skip confirmation
```

---

## Shell Completions

```bash
eval "$(runx completions bash)"    # Bash — add to ~/.bashrc
eval "$(runx completions zsh)"     # Zsh — add to ~/.zshrc
runx completions fish | source     # Fish — add to config.fish
```

---

## How It Works

```
runx --with node@22 -- node server.js

  1. Resolve     node@22 → 22.22.1 (via nodejs.org API)
  2. Download    https://nodejs.org/dist/v22.22.1/node-v22.22.1-darwin-arm64.tar.gz
  3. Cache       ~/.runx/cache/node/22.22.1/macOS-aarch64/
  4. Isolate     PATH = [cached node/bin] + [/usr/bin, /bin]
  5. Execute     node server.js (in clean environment)
  6. Exit        Forward exit code, clean up temp dirs
```

Cached tools skip steps 1–3 — repeat runs start in **milliseconds**.

Written in Rust. Single binary. No runtime dependencies.

---

## CLI Reference

```
USAGE:
  runx [OPTIONS] [-- <CMD>...]       Run a command with specified tools
  runx <SUBCOMMAND>                  Manage tools, cache, and config

SUBCOMMANDS:
  init         Create .runxrc config file (interactive or --with flags)
  install      Install tools globally to ~/.runx/bin/
  uninstall    Remove globally installed tools
  list         List tools, cached versions, or upstream availability
  clean        Remove cached binaries to free disk space
  lock         Generate .runxrc.lock for reproducible builds
  update       Update cached tools to latest patch versions
  plugin       Manage custom tool provider plugins (list/add/remove)
  completions  Generate shell completion scripts (bash/zsh/fish)

OPTIONS:
  --with <TOOL@VERSION>   Tool to include (repeatable)
  --dry-run               Show what would happen without doing it
  --inherit-env           Pass through your full shell environment
  -v, --verbose           Show download progress and debug info
  -q, --quiet             Suppress all progress output
  -V, --version           Print version
  -h, --help              Print help
```

---

## Contributing

```bash
git clone https://github.com/supa-magic/runx.git && cd runx
cargo test                    # 343 tests
cargo clippy                  # Zero warnings policy
cargo fmt --check             # Enforced formatting
```

We welcome contributions! Check the [open issues](https://github.com/supa-magic/runx/issues) for ideas.

## License

[MIT](LICENSE)

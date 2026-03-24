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
v22.22.2
```

That's it. Node 22 was downloaded, cached, and your command ran in a fully isolated environment. Your system wasn't touched. Next time, it starts instantly.

**runx** is a single binary that replaces nvm, pyenv, goenv, and a dozen YAML lines in your CI — for **Node.js, Python, Go, Deno, Bun, Ruby, Java, and Rust** (and [any tool via plugins](#plugins)).

| Problem | Before runx | With runx |
|---------|-------------|-----------|
| "Works on my machine" | Everyone has different versions | `.runxrc` pins versions for the whole team |
| Testing across versions | `nvm use 18`, test, `nvm use 22`, test | `runx --with node@18 -- npm test` |
| CI/CD tool setup | `actions/setup-node` + `actions/setup-python` + ... | `runx --with node@22 -- npm run build` |
| Onboarding a new dev | "Follow these 12 setup steps..." | `git clone && runx -- npm start` |
| Trying a new runtime | Install globally, use once, forget to uninstall | `runx --with bun -- bun run index.ts` |
| Environment pollution | `NVM_DIR`, `PYENV_ROOT` leaking everywhere | Isolated clean-room env every time |

---

## Examples

```bash
runx --with node -- npm test
runx --with python -- python3 app.py
runx --with java -- javac Main.java && java Main
runx --with ruby -- ruby -e "puts 'hello from Ruby'"
runx --with rust -- cargo build --release
runx --with go -- go run main.go
```

<details>
<summary><b>Node.js</b> — run any version without nvm</summary>

```bash
# Latest stable — no version needed
runx --with node -- node -v

# Test against multiple versions
runx --with node@18 -- npm test
runx --with node@22 -- npm test

# Next.js, Vite, Astro — any npx tool
runx --with node -- npx create-next-app@latest my-app
runx --with node -- npx create-vite my-app --template react-ts
runx --with node -- npx create-astro@latest

# Deploy with Vercel, Wrangler, Netlify
runx --with node -- npx vercel deploy
runx --with node -- npx wrangler dev
```

</details>

<details>
<summary><b>Python</b> — isolated Python without pyenv or conda</summary>

```bash
# Latest stable
runx --with python -- python3 --version

# Django, FastAPI, Flask
runx --with python -- pip install django && django-admin startproject mysite
runx --with python@3.12 -- pip install "fastapi[standard]" && uvicorn main:app
runx --with python -- pip install flask && python3 app.py

# Data science
runx --with python@3.11 -- pip install pandas numpy && python3 analyze.py

# Test across versions
runx --with python@3.11 -- pytest
runx --with python@3.12 -- pytest
```

</details>

<details>
<summary><b>Go</b> — any Go version, zero goenv</summary>

```bash
# Build and run
runx --with go -- go run main.go
runx --with go -- go build -ldflags="-s -w" -o myapp .

# Gin, Hugo
runx --with go -- go run github.com/gin-gonic/gin/examples/basic
runx --with go -- go run github.com/gohugoio/hugo@latest new site mysite

# Test with a specific version
runx --with go@1.22 -- go test ./...
```

</details>

<details>
<summary><b>Deno</b> — secure TypeScript runtime</summary>

```bash
# Run TypeScript directly
runx --with deno -- deno run server.ts

# Fresh framework
runx --with deno -- deno run -A https://fresh.deno.dev my-fresh-app

# Built-in tools
runx --with deno -- deno fmt
runx --with deno -- deno lint
runx --with deno -- deno test
```

</details>

<details>
<summary><b>Bun</b> — blazing-fast JavaScript runtime</summary>

```bash
# Run TypeScript with zero config
runx --with bun -- bun run index.ts

# Elysia framework
runx --with bun -- bun create elysia my-app && cd my-app && bun run dev

# bunx (like npx, but faster)
runx --with bun -- bunx prisma generate
```

</details>

<details>
<summary><b>Ruby</b> — prebuilt Ruby, no compilation wait</summary>

```bash
# Rails, Sinatra, Jekyll
runx --with ruby -- gem install rails && rails new myapp
runx --with ruby -- gem install sinatra && ruby app.rb
runx --with ruby -- gem install jekyll && jekyll new myblog

# Test with RSpec
runx --with ruby -- gem install rspec && rspec
```

</details>

<details>
<summary><b>Java</b> — any JDK via Adoptium (JAVA_HOME set automatically)</summary>

```bash
# Compile and run
runx --with java@21 -- javac Main.java && java Main

# Spring Boot, Gradle, Quarkus
runx --with java@21 -- ./mvnw spring-boot:run
runx --with java@21 -- ./gradlew build
runx --with java@21 -- ./mvnw quarkus:dev

# Test across LTS versions
runx --with java@17 -- ./mvnw test
runx --with java@21 -- ./mvnw test

# jshell REPL
runx --with java -- jshell
```

</details>

<details>
<summary><b>Rust</b> — standalone toolchain, no rustup needed</summary>

```bash
# Build, test, lint
runx --with rust -- cargo build --release
runx --with rust -- cargo test
runx --with rust -- cargo clippy && cargo fmt --check

# New project
runx --with rust -- cargo init hello && cd hello && cargo run
```

</details>

<details>
<summary><b>Multiple runtimes</b> — download in parallel</summary>

```bash
runx --with node@22 --with python@3.12 -- npm run fullstack
runx --with java@21 --with node@22 -- ./build-all.sh
```

</details>

---

## Team Configuration

Commit a `.runxrc` file — every developer and CI job gets the same versions:

```toml
# .runxrc
tools = ["node@22", "python@3.12"]
```

```bash
runx -- npm start              # Picks up node@22 from .runxrc
runx -- python3 manage.py      # Picks up python@3.12
```

<details>
<summary><b>.runxrc examples for popular stacks</b></summary>

```toml
# Full-stack: Node frontend + Python backend
tools = ["node@22", "python@3.12"]
```

```toml
# Java Spring Boot
tools = ["java@21"]
```

```toml
# Monorepo
tools = ["node@22", "python@3.12", "go@1"]
inherit_env = true
```

</details>

### Version pinning

```bash
runx --with node@22         -- node -v   # Latest 22.x.x
runx --with node@22.11      -- node -v   # Latest 22.11.x
runx --with node@22.11.0    -- node -v   # Exact version
runx --with node             -- node -v   # Latest stable
```

### Lockfile

```bash
runx lock                    # Resolve .runxrc → .runxrc.lock (exact versions + URLs + checksums)
runx lock --update           # Re-resolve and update
```

When `.runxrc.lock` exists, runx skips version resolution — same binary, every time. Commit it alongside `.runxrc`.

### CI/CD

```yaml
# .github/workflows/ci.yml — no setup-node, no setup-python, just runx
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-x86_64-unknown-linux-gnu.tar.gz | tar xz
      - run: ./runx -- npm test
      - run: ./runx -- python3 -m pytest
```

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

---

## Self-Contained Scripts

Scripts that **bring their own runtime**. Add a shebang line — the runtime downloads on first run.

<details>
<summary><b>deploy.js</b> — Node.js deployment script</summary>

```js
#!/usr/bin/env -S runx --with node@22 --
// deploy.js — deploys to production with zero setup

const { execSync } = require("child_process");
const branch = execSync("git branch --show-current").toString().trim();

if (branch !== "main") {
  console.error(`Refusing to deploy from branch '${branch}'`);
  process.exit(1);
}

console.log("Building...");
execSync("npm run build", { stdio: "inherit" });
console.log("Deploying...");
execSync("npm run deploy", { stdio: "inherit" });
console.log("Done!");
```

</details>

<details>
<summary><b>analyze.py</b> — Python data analysis</summary>

```python
#!/usr/bin/env -S runx --with python@3.12 --
# analyze.py — quick CSV analysis, no virtualenv needed

import csv, sys, statistics

if len(sys.argv) < 2:
    print("Usage: ./analyze.py data.csv")
    sys.exit(1)

with open(sys.argv[1]) as f:
    rows = list(csv.DictReader(f))

print(f"Rows: {len(rows)}")
for col in rows[0]:
    try:
        vals = [float(r[col]) for r in rows]
        print(f"{col}: mean={statistics.mean(vals):.2f}, median={statistics.median(vals):.2f}")
    except ValueError:
        print(f"{col}: {len(set(r[col] for r in rows))} unique values")
```

</details>

<details>
<summary><b>health.rb</b> — Ruby health check</summary>

```ruby
#!/usr/bin/env -S runx --with ruby@3 --
# health.rb — check if services are responding

require "net/http"
require "uri"

services = {
  "API"      => "https://api.example.com/health",
  "Frontend" => "https://app.example.com",
  "Docs"     => "https://docs.example.com",
}

services.each do |name, url|
  begin
    res = Net::HTTP.get_response(URI(url))
    status = res.code.to_i < 400 ? "OK" : "FAIL"
    puts "#{name}: #{status} (#{res.code})"
  rescue => e
    puts "#{name}: DOWN (#{e.message})"
  end
end
```

</details>

<details>
<summary><b>server.ts</b> — Deno HTTP server</summary>

```ts
#!/usr/bin/env -S runx --with deno --
// server.ts — single-file HTTP server

const port = parseInt(Deno.args[0] ?? "3000");

Deno.serve({ port }, (req: Request) => {
  const url = new URL(req.url);
  console.log(`${req.method} ${url.pathname}`);
  return new Response(`Hello from Deno on port ${port}!\n`);
});
```

</details>

```bash
chmod +x deploy.js analyze.py health.rb server.ts

./deploy.js                    # Downloads Node 22 on first run, then deploys
./analyze.py sales.csv         # Downloads Python 3.12, analyzes the CSV
./health.rb                    # Downloads Ruby 3, checks all services
./server.ts 8080               # Downloads Deno, starts the server
```

Share scripts with your team, drop them in CI, hand them to clients — no setup instructions needed.

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
cargo install --git https://github.com/supa-magic/runx.git
```

</details>

Single binary. No dependencies. No Node.js or Python required to run runx itself.

---

## How It Works

```
runx --with node@22 -- node server.js

  1. Resolve     node@22 → 22.22.2 (via nodejs.org API)
  2. Download    https://nodejs.org/dist/v22.22.2/node-v22.22.2-darwin-arm64.tar.gz
  3. Cache       ~/.runx/cache/node/22.22.2/macOS-aarch64/
  4. Isolate     PATH = [cached node/bin] + [/usr/bin, /bin]
  5. Execute     node server.js (in clean environment)
  6. Exit        Forward exit code, clean up temp dirs
```

Cached tools skip steps 1-3 — repeat runs start in **milliseconds**.

### Environment isolation

Every command runs in a **clean-room environment** — your system PATH, `NVM_DIR`, `PYENV_ROOT`, and other tool managers are invisible:

| Always inherited | Constructed by runx | Blocked |
|-----------------|--------------------|---------|
| `HOME`, `USER`, `TERM`, `LANG`, `SHELL`, `TMPDIR` | `PATH` = tool bins + `/usr/bin` | Your `PATH`, `NVM_DIR`, `PYENV_ROOT`, etc. |

Need your full environment? Add `--inherit-env`.

---

## Global Install

Use runx as a **lightweight version manager** when you want tools permanently available:

```bash
runx install node@22              # Symlink → ~/.runx/bin/node
runx install python@3.12          # Symlink → ~/.runx/bin/python3
runx install                      # Install everything from .runxrc

runx install --list               # See what's installed
runx uninstall node               # Remove it
runx update                       # Update all to latest patches
```

Add to your shell profile once: `export PATH="$HOME/.runx/bin:$PATH"`

---

## Plugins

Add **any tool** — not just the 8 built-in runtimes — with a TOML manifest:

```toml
# ~/.runx/plugins/zig.toml
name = "zig"
aliases = ["ziglang"]
download_url = "https://ziglang.org/builds/zig-{os}-{arch}-{version}.tar.xz"
archive_format = "tar.xz"
bin_path = "zig-{os}-{arch}-{version}"
```

```bash
runx plugin add ./zig.toml
runx --with zig@0.11.0 -- zig version
runx plugin list
runx plugin remove zig
```

Placeholders: `{version}`, `{os}`, `{arch}`, `{triple}`, `{os_alt}`, `{arch_alt}`.

---

## Cache Management

```bash
runx list                      # All tools and cache status
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
eval "$(runx completions bash)"    # add to ~/.bashrc
eval "$(runx completions zsh)"     # add to ~/.zshrc
runx completions fish | source     # add to config.fish
```

---

## CLI Reference

```
USAGE:
  runx [OPTIONS] [-- <CMD>...]       Run a command with specified tools
  runx <SUBCOMMAND>                  Manage tools, cache, and config

SUBCOMMANDS:
  init         Create .runxrc config file
  install      Install tools globally to ~/.runx/bin/
  uninstall    Remove globally installed tools
  list         List tools, cached versions, or upstream availability
  clean        Remove cached binaries
  lock         Generate .runxrc.lock for reproducible builds
  update       Update cached tools to latest patch versions
  plugin       Manage plugin providers (list/add/remove)
  completions  Generate shell completions (bash/zsh/fish)

OPTIONS:
  --with <TOOL@VERSION>   Tool to include (repeatable)
  --dry-run               Show what would happen without doing it
  --inherit-env           Pass through your full shell environment
  -v, --verbose           Show download progress and debug info
  -q, --quiet             Suppress all progress output
  -h, --help              Print help
```

---

## Contributing

```bash
git clone https://github.com/supa-magic/runx.git && cd runx
cargo test                    # 479 tests
cargo clippy                  # Zero warnings policy
cargo fmt --check             # Enforced formatting
```

[Open issues](https://github.com/supa-magic/runx/issues) — contributions welcome.

## License

[MIT](LICENSE)

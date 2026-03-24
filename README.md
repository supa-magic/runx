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

[Install](#install) | [Examples](#examples) | [Configuration](#configuration) | [CI/CD](#cicd) | [CLI Reference](#cli-reference)

**Why runx?**

- **"Works on my machine"** — commit a `.runxrc`, everyone gets the same versions
- **Testing across versions** — `runx --with node@18 -- npm test`, then `node@22`, done
- **CI/CD** — one binary, no `actions/setup-node` + `actions/setup-python` + ...
- **Onboarding** — `git clone && runx -- npm start` instead of 12 setup steps
- **Try anything** — `runx --with bun -- bun run index.ts`, no global install
- **No pollution** — isolated env every time, no `NVM_DIR` or `PYENV_ROOT` leaking

---

## Examples

```bash
runx --with node -- npm test                # Node.js (latest)
runx --with python@3.12 -- python3 app.py   # Python (specific)
runx --with go -- go run main.go            # Go
runx --with ruby -- rails server            # Ruby
runx --with java@21 -- ./mvnw spring-boot:run  # Java
runx --with rust -- cargo build --release   # Rust
```

<details>
<summary><b>Node.js</b> — run any version without nvm</summary>

```bash
# Test against multiple versions
runx --with node@18 -- npm test
runx --with node@22 -- npm test

# Next.js, Vite, Astro — any npx tool
runx --with node -- npx create-next-app@latest my-app
runx --with node -- npx create-vite my-app --template react-ts

# Deploy with Vercel, Wrangler
runx --with node -- npx vercel deploy
runx --with node -- npx wrangler dev
```

</details>

<details>
<summary><b>Python</b> — isolated Python without pyenv or conda</summary>

```bash
# Django
runx --with python -- pip install django
runx --with python -- django-admin startproject mysite

# FastAPI
runx --with python@3.12 -- pip install "fastapi[standard]"
runx --with python@3.12 -- uvicorn main:app

# Data science
runx --with python@3.11 -- pip install pandas numpy
runx --with python@3.11 -- python3 analyze.py

# Test across versions
runx --with python@3.11 -- pytest
runx --with python@3.12 -- pytest
```

</details>

<details>
<summary><b>Go</b> — any Go version, zero goenv</summary>

```bash
runx --with go -- go run main.go
runx --with go -- go build -ldflags="-s -w" -o myapp .
runx --with go@1.22 -- go test ./...
runx --with go -- go run github.com/gohugoio/hugo@latest new site mysite
```

</details>

<details>
<summary><b>Deno</b> — secure TypeScript runtime</summary>

```bash
runx --with deno -- deno run server.ts
runx --with deno -- deno run -A https://fresh.deno.dev my-fresh-app
runx --with deno -- deno fmt
runx --with deno -- deno lint
runx --with deno -- deno test
```

</details>

<details>
<summary><b>Bun</b> — blazing-fast JavaScript runtime</summary>

```bash
runx --with bun -- bun run index.ts
runx --with bun -- bun create elysia my-app
runx --with bun -- bun run --cwd my-app dev
runx --with bun -- bunx prisma generate
```

</details>

<details>
<summary><b>Ruby</b> — prebuilt Ruby, no compilation wait</summary>

```bash
runx --with ruby -- gem install rails
runx --with ruby -- rails new myapp

runx --with ruby -- gem install sinatra
runx --with ruby -- ruby app.rb

runx --with ruby -- gem install jekyll
runx --with ruby -- jekyll new myblog
```

</details>

<details>
<summary><b>Java</b> — any JDK via Adoptium (JAVA_HOME set automatically)</summary>

```bash
runx --with java@21 -- javac Main.java
runx --with java@21 -- java Main
runx --with java@21 -- ./mvnw spring-boot:run
runx --with java@21 -- ./gradlew build
runx --with java@17 -- ./mvnw test
runx --with java -- jshell
```

</details>

<details>
<summary><b>Rust</b> — standalone toolchain, no rustup needed</summary>

```bash
runx --with rust -- cargo build --release
runx --with rust -- cargo test
runx --with rust -- cargo clippy
runx --with rust -- cargo fmt --check
runx --with rust -- cargo init hello
```

</details>

<details>
<summary><b>AI / ML</b> — Python + Node for AI development</summary>

```bash
# OpenAI / Anthropic SDK
runx --with python@3.12 -- pip install anthropic
runx --with python@3.12 -- python3 agent.py

# LangChain + FastAPI
runx --with python@3.12 -- pip install langchain fastapi uvicorn
runx --with python@3.12 -- uvicorn api:app

# Hugging Face transformers
runx --with python@3.11 -- pip install torch transformers
runx --with python@3.11 -- python3 train.py

# Node.js AI SDK (Vercel AI)
runx --with node -- npx create-ai-app my-ai-app

# Full-stack AI: Python backend + Node frontend
runx --with python@3.12 --with node@22 -- ./start-ai-app.sh
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

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/supa-magic/runx/main/install.sh | sh
```

Auto-detects your OS and architecture. Installs to `/usr/local/bin` (or `~/.runx/bin` if no sudo).

<details>
<summary>Install a specific version</summary>

```bash
RUNX_VERSION=v0.3.0 curl -fsSL https://raw.githubusercontent.com/supa-magic/runx/main/install.sh | sh
```

</details>

<details>
<summary>Manual install</summary>

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

</details>

<details>
<summary>Build from source</summary>

```bash
cargo install --git https://github.com/supa-magic/runx.git
```

</details>

Single binary. No dependencies. No Node.js or Python required to run runx itself.

Try it without committing — see what runx would do:

```bash
runx --dry-run --with node@22 -- node -v
# Shows what would be downloaded and executed, without doing it
```

---

## Configuration

Create a `.runxrc` to pin tool versions for your project:

```bash
runx init                                    # Interactive wizard
runx init --with node@22 --with python@3.12  # Non-interactive
```

```toml
# .runxrc — commit this to your repo
tools = ["node@22", "python@3.12"]
```

```bash
runx -- npm start              # Picks up node@22 from .runxrc
runx -- python3 manage.py      # Picks up python@3.12
```

### Version pinning

```bash
runx --with node@22         -- node -v   # Latest 22.x.x
runx --with node@22.11      -- node -v   # Latest 22.11.x
runx --with node@22.11.0    -- node -v   # Exact version
runx --with node             -- node -v   # Latest stable
```

### Lockfile

```bash
runx lock                    # Resolve .runxrc → .runxrc.lock (exact versions + URLs)
runx lock --update           # Re-resolve and update
```

When `.runxrc.lock` exists, runx skips version resolution — same binary, every time. Commit it alongside `.runxrc`.

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
# AI / ML project
tools = ["python@3.12", "node@22"]
inherit_env = true  # pass through API keys (OPENAI_API_KEY, ANTHROPIC_API_KEY)
```

```toml
# Monorepo
tools = ["node@22", "python@3.12", "go@1"]
```

</details>

<details>
<summary>Advanced config options</summary>

- Auto-discovered by walking up parent directories (like `.gitignore`)
- CLI `--with` flags override the config entirely
- `inherit_env = true` passes your full shell environment through
- `--dry-run` and `--verbose` show which config file was loaded

</details>

---

## CI/CD

Replace `actions/setup-node`, `actions/setup-python`, and friends with a single binary:

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: curl -sL https://github.com/supa-magic/runx/releases/latest/download/runx-x86_64-unknown-linux-gnu.tar.gz | tar xz
      - run: ./runx -- npm test
      - run: ./runx -- python3 -m pytest
```

With a `.runxrc.lock`, CI downloads the exact same binaries every run — no version drift.

### Docker

With a `.runxrc`, your Dockerfile doesn't need `nvm`, `pyenv`, or multi-stage builds:

```toml
# .runxrc
tools = ["node@22", "python@3.12"]
```

```dockerfile
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y curl
RUN curl -fsSL https://raw.githubusercontent.com/supa-magic/runx/main/install.sh | sh
COPY .runxrc .runxrc.lock* ./

RUN runx -- npm ci
RUN runx -- npm run build
RUN runx -- pip install -r requirements.txt

# For production, install globally to avoid runx overhead on every start
RUN runx install
CMD ["python3", "-m", "uvicorn", "main:app", "--host", "0.0.0.0"]
```

Same `.runxrc` used by developers, CI, and Docker — versions stay in sync everywhere. The `.runxrc.lock` ensures the exact same binaries in every build.

<details>
<summary><b>Compare: traditional multi-stage Dockerfile</b></summary>

```dockerfile
# Without runx: 2 base images, apt-get, version management scattered
FROM node:22-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM python:3.12-slim
RUN apt-get update && apt-get install -y ...
COPY requirements.txt .
RUN pip install -r requirements.txt
COPY --from=frontend /app/frontend/dist ./static
CMD ["uvicorn", "main:app", "--host", "0.0.0.0"]
```

</details>

---

## Self-Contained Scripts

Scripts that **bring their own runtime**. Add a shebang line — the runtime downloads on first run.

<details>
<summary><b>deploy.js</b> — Node.js deployment script</summary>

```js
#!/usr/bin/env -S runx --with node@22 --
const { execSync } = require("child_process");
const branch = execSync("git branch --show-current").toString().trim();

if (branch !== "main") {
  console.error(`Refusing to deploy from branch '${branch}'`);
  process.exit(1);
}

execSync("npm run build", { stdio: "inherit" });
execSync("npm run deploy", { stdio: "inherit" });
console.log("Done!");
```

</details>

<details>
<summary><b>analyze.py</b> — Python data analysis</summary>

```python
#!/usr/bin/env -S runx --with python@3.12 --
import csv, sys, statistics

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
<summary><b>ask.py</b> — CLI AI assistant (uses Anthropic API)</summary>

```python
#!/usr/bin/env -S runx --with python@3.12 --inherit-env --
# ask.py — ask Claude from the command line
# Usage: ./ask.py "explain this error" < traceback.txt

import sys
try:
    import anthropic
except ImportError:
    import subprocess
    subprocess.check_call([sys.executable, "-m", "pip", "install", "anthropic", "-q"])
    import anthropic

prompt = sys.argv[1] if len(sys.argv) > 1 else "Summarize this input"
stdin = sys.stdin.read() if not sys.stdin.isatty() else ""

client = anthropic.Anthropic()  # uses ANTHROPIC_API_KEY from env
msg = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    messages=[{"role": "user", "content": f"{prompt}\n\n{stdin}".strip()}],
)
print(msg.content[0].text)
```

</details>

<details>
<summary>More: health.rb, server.ts</summary>

```ruby
#!/usr/bin/env -S runx --with ruby@3 --
# health.rb — check if services are responding
require "net/http"
require "uri"

{"API" => "https://api.example.com/health", "Docs" => "https://docs.example.com"}.each do |name, url|
  res = Net::HTTP.get_response(URI(url)) rescue nil
  puts "#{name}: #{res&.code&.to_i.to_i < 400 ? 'OK' : 'DOWN'}"
end
```

```ts
#!/usr/bin/env -S runx --with deno --
// server.ts — single-file HTTP server
const port = parseInt(Deno.args[0] ?? "3000");
Deno.serve({ port }, (req: Request) => {
  console.log(`${req.method} ${new URL(req.url).pathname}`);
  return new Response(`Hello from Deno on port ${port}!\n`);
});
```

</details>

```bash
chmod +x deploy.js analyze.py health.rb server.ts
./deploy.js                    # Downloads Node 22 on first run, then deploys
./analyze.py sales.csv         # Downloads Python 3.12, analyzes the CSV
```

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

Every command runs in a **clean-room environment**:
- **Inherited:** `HOME`, `USER`, `TERM`, `LANG`, `SHELL`, `TMPDIR`, plus `LC_*` and `XDG_*` prefixed vars
- **Constructed:** `PATH` = tool bins + `/usr/bin:/bin`
- **Blocked:** your `PATH`, `NVM_DIR`, `PYENV_ROOT`, and everything else

Need your full environment? Add `--inherit-env`.

---

## More

### Global install

Use runx as a lightweight version manager:

```bash
runx install node@22              # Symlink → ~/.runx/bin/node
runx install python@3.12          # Symlink → ~/.runx/bin/python3
runx install                      # Install everything from .runxrc
runx install --list               # See what's installed
runx uninstall node               # Remove it
runx update                       # Update all to latest patches
```

Add to your shell profile: `export PATH="$HOME/.runx/bin:$PATH"`

### Plugins

Add any tool with a TOML manifest:

```toml
# ~/.runx/plugins/zig.toml
name = "zig"
download_url = "https://ziglang.org/builds/zig-{os}-{arch}-{version}.tar.xz"
archive_format = "tar.xz"
bin_path = "zig-{os}-{arch}-{version}"
```

```bash
runx plugin add ./zig.toml
runx --with zig@0.11.0 -- zig version
```

Placeholders: `{version}`, `{os}`, `{arch}`, `{triple}`, `{os_alt}`, `{arch_alt}`.

### Cache management

```bash
runx list                      # All tools and cache status
runx list --cached             # Cached versions with disk sizes
runx clean                     # Remove everything (with confirmation)
runx clean --tool node         # Remove only Node.js caches
runx clean --older-than 30d    # Remove stale versions
```

### Shell completions

```bash
eval "$(runx completions bash)"    # add to ~/.bashrc
eval "$(runx completions zsh)"     # add to ~/.zshrc
runx completions fish | source     # add to config.fish
```

### Uninstall

```bash
rm /usr/local/bin/runx             # Remove the binary
rm -rf ~/.runx                     # Remove cache and plugins
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
  -V, --version           Print version
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

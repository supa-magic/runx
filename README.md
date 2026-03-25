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

<p align="center">
  <img src="demo.gif" alt="runx demo" width="800">
</p>

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

Scripts that **bring their own runtime**. Copy any of these, `chmod +x`, and run — the runtime downloads automatically on first execution.

<details open>
<summary><b>serve.js</b> — instant file server (Node.js)</summary>

```js
#!/usr/bin/env -S runx --with node@22 --
// serve.js — serve the current directory over HTTP
// Usage: ./serve.js [port]

const http = require("http");
const fs = require("fs");
const path = require("path");

const port = process.argv[2] || 3000;
const dir = process.cwd();

const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css",
  ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg" };

http.createServer((req, res) => {
  const file = path.join(dir, req.url === "/" ? "index.html" : req.url);
  fs.readFile(file, (err, data) => {
    if (err) { res.writeHead(404); return res.end("Not found"); }
    res.writeHead(200, { "Content-Type": mime[path.extname(file)] || "text/plain" });
    res.end(data);
  });
  console.log(`${req.method} ${req.url}`);
}).listen(port, () => console.log(`Serving ${dir} at http://localhost:${port}`));
```

```bash
chmod +x serve.js
./serve.js           # Serves current directory on :3000
./serve.js 8080      # Custom port
```

</details>

<details>
<summary><b>loc.py</b> — count lines of code (Python)</summary>

```python
#!/usr/bin/env -S runx --with python@3.12 --
# loc.py — count lines of code by language
# Usage: ./loc.py [directory]

import os, sys
from collections import defaultdict

exts = {".py": "Python", ".js": "JavaScript", ".ts": "TypeScript", ".rs": "Rust",
        ".go": "Go", ".rb": "Ruby", ".java": "Java", ".c": "C", ".cpp": "C++",
        ".html": "HTML", ".css": "CSS", ".sh": "Shell", ".toml": "TOML", ".yaml": "YAML"}

root = sys.argv[1] if len(sys.argv) > 1 else "."
stats = defaultdict(lambda: {"files": 0, "lines": 0})
skip = {".git", "node_modules", "target", "__pycache__", ".venv", "dist", "build"}

for dirpath, dirnames, filenames in os.walk(root):
    dirnames[:] = [d for d in dirnames if d not in skip]
    for f in filenames:
        ext = os.path.splitext(f)[1]
        if ext in exts:
            try:
                lines = sum(1 for _ in open(os.path.join(dirpath, f)))
                stats[exts[ext]]["files"] += 1
                stats[exts[ext]]["lines"] += lines
            except (OSError, UnicodeDecodeError):
                pass

if not stats:
    print("No source files found.")
    sys.exit(0)

print(f"{'Language':<14} {'Files':>6} {'Lines':>8}")
print(f"{'─' * 14} {'─' * 6} {'─' * 8}")
for lang, s in sorted(stats.items(), key=lambda x: -x[1]["lines"]):
    print(f"{lang:<14} {s['files']:>6} {s['lines']:>8}")
print(f"{'─' * 14} {'─' * 6} {'─' * 8}")
print(f"{'Total':<14} {sum(s['files'] for s in stats.values()):>6} {sum(s['lines'] for s in stats.values()):>8}")
```

```bash
chmod +x loc.py
./loc.py             # Count lines in current directory
./loc.py ~/projects  # Count lines in any directory
```

</details>

<details>
<summary><b>server.ts</b> — HTTP server with routing (Deno)</summary>

```ts
#!/usr/bin/env -S runx --with deno --
// server.ts — HTTP server with JSON API
// Usage: ./server.ts [port]

const port = parseInt(Deno.args[0] ?? "3000");

Deno.serve({ port }, (req: Request) => {
  const url = new URL(req.url);
  console.log(`${req.method} ${url.pathname}`);

  if (url.pathname === "/api/time") {
    return Response.json({ time: new Date().toISOString(), timezone: Intl.DateTimeFormat().resolvedOptions().timeZone });
  }
  if (url.pathname === "/api/echo" && req.method === "POST") {
    return req.text().then(body => Response.json({ echo: body }));
  }
  if (url.pathname === "/") {
    return new Response("<h1>runx + Deno</h1><p>Try <a href='/api/time'>/api/time</a></p>", { headers: { "Content-Type": "text/html" } });
  }
  return new Response("Not found", { status: 404 });
});

console.log(`Server running at http://localhost:${port}`);
```

```bash
chmod +x server.ts
./server.ts          # Start server on :3000
curl localhost:3000/api/time
```

</details>

<details>
<summary><b>sysinfo.rb</b> — system info report (Ruby)</summary>

```ruby
#!/usr/bin/env -S runx --with ruby@3 --
# sysinfo.rb — print system information

info = {
  "Hostname"     => `hostname`.strip,
  "OS"           => RUBY_PLATFORM,
  "Ruby"         => RUBY_VERSION,
  "CPU Cores"    => (File.read("/proc/cpuinfo").scan(/^processor/i).length rescue `sysctl -n hw.ncpu`.strip),
  "Memory"       => (File.read("/proc/meminfo").match(/MemTotal:\s+(\d+)/)[1].to_i / 1024 rescue `sysctl -n hw.memsize`.strip.to_i / 1024 / 1024).to_s + " MB",
  "Disk Free"    => `df -h /`.split("\n").last.split[3],
  "Uptime"       => `uptime`.strip.match(/(up.*?),\s*\d+ user/)[1],
  "Shell"        => ENV["SHELL"] || "unknown",
  "User"         => ENV["USER"] || "unknown",
  "Working Dir"  => Dir.pwd,
}

max_key = info.keys.map(&:length).max
info.each { |k, v| puts "  #{k.ljust(max_key)}  #{v}" }
```

```bash
chmod +x sysinfo.rb
./sysinfo.rb         # Prints system info — no gems needed
```

</details>

<details>
<summary><b>json.js</b> — JSON formatter from stdin (Node.js)</summary>

```js
#!/usr/bin/env -S runx --with node --
// json.js — pretty-print and query JSON from stdin or file
// Usage: cat data.json | ./json.js
//        ./json.js < package.json
//        curl -s api.example.com | ./json.js

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", chunk => input += chunk);
process.stdin.on("end", () => {
  try {
    const data = JSON.parse(input);
    console.log(JSON.stringify(data, null, 2));
  } catch (e) {
    console.error(`Invalid JSON: ${e.message}`);
    process.exit(1);
  }
});
```

```bash
chmod +x json.js
echo '{"name":"runx","version":"0.4.0"}' | ./json.js
curl -s https://api.github.com/repos/supa-magic/runx | ./json.js
```

</details>

Every script above works immediately after `chmod +x` — no project setup, no `npm install`, no API keys. Share them with your team or drop them in any repo.

> **Windows note:** Shebangs (`#!`) are a Unix feature. On Windows, run scripts explicitly: `runx --with node@22 -- node serve.js`

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

Version resolution (step 1) is resilient to transient network failures: runx retries on 429, 500, 502, 503, and 504 responses up to 3 times with exponential backoff (1s, 2s, 4s), and respects `Retry-After` headers on rate-limit responses. Use `--verbose` to see retry messages.

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
runx list --cached java        # Cached versions for a specific tool
runx clean                     # Remove everything (with confirmation)
runx clean node                # Remove all Node.js caches
runx clean java@21             # Remove only Java 21.x versions
runx clean java@21.0.10        # Remove exact version
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
  -v, --verbose           Show download progress, debug info, and retry messages
  -q, --quiet             Suppress all progress output
  -V, --version           Print version
  -h, --help              Print help
```

---

## Contributing

```bash
git clone https://github.com/supa-magic/runx.git && cd runx
cargo test                    # 483 tests
cargo clippy                  # Zero warnings policy
cargo fmt --check             # Enforced formatting
```

[Open issues](https://github.com/supa-magic/runx/issues) — contributions welcome.

## License

[MIT](LICENSE)

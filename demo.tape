# runx demo — https://github.com/supa-magic/runx
# Record with: vhs demo.tape

Output demo.gif
Set Theme "Dracula"
Set FontSize 16
Set Width 900
Set Height 550
Set Padding 20
Set TypingSpeed 35ms
Set Shell "bash"
Env PATH "/usr/local/bin:/Users/ab/.runx/bin:/opt/homebrew/bin:/usr/bin:/bin"
Env HOME "/Users/ab"

# Install
Type "echo '→ Install runx with one command'"
Enter
Sleep 500ms
Type "curl -fsSL https://raw.githubusercontent.com/supa-magic/runx/main/install.sh | sh"
Enter
Sleep 8s

Type 'export PATH="$HOME/.runx/bin:$PATH"'
Enter
Sleep 500ms
Type "clear"
Enter
Sleep 500ms

# Node
Type "echo '→ Node.js — no nvm needed'"
Enter
Sleep 500ms
Type "runx --with node@22 -- node -v"
Enter
Sleep 5s

# Python
Type "echo '→ Python — no pyenv, no conda'"
Enter
Sleep 500ms
Type "runx --with python@3.12 -- python3 --version"
Enter
Sleep 5s

# Cached
Type "echo '→ Second run is instant (cached)'"
Enter
Sleep 500ms
Type "runx --with node@22 -- node -v"
Enter
Sleep 2s

# Vite + React
Type "echo '→ Create a React app with Vite'"
Enter
Sleep 500ms
Type "runx --with node -- npx create-vite my-app --template react"
Enter
Sleep 10s

# .runxrc
Type "clear"
Enter
Sleep 500ms
Type "echo '→ Pin versions for your team'"
Enter
Sleep 500ms
Type 'echo '"'"'tools = ["node@22"]'"'"' > .runxrc && cat .runxrc'
Enter
Sleep 2s
Type "runx -- node -v"
Enter
Sleep 3s

Sleep 2s

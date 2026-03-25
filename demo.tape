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
Sleep 1s
Type "curl -fsSL https://raw.githubusercontent.com/supa-magic/runx/main/install.sh | sh"
Enter
Sleep 10s
Type 'export PATH="$HOME/.runx/bin:$PATH"'
Enter
Sleep 1s
Type "clear"
Enter
Sleep 500ms

# Node
Type "echo"
Enter
Type "echo '→ Node.js — no nvm needed'"
Enter
Sleep 1s
Type "runx --with node@22 -- node -v"
Enter
Sleep 6s

# Go (more reliable than Python which can 504)
Type "echo"
Enter
Type "echo '→ Go — no goenv'"
Enter
Sleep 1s
Type "runx --with go -- go version"
Enter
Sleep 6s

# Cached
Type "echo"
Enter
Type "echo '→ Second run — instant from cache'"
Enter
Sleep 1s
Type "runx --with node@22 -- node -v"
Enter
Sleep 3s

# .runxrc
Type "clear"
Enter
Sleep 500ms
Type "echo"
Enter
Type "echo '→ Pin versions for your whole team'"
Enter
Sleep 1s
Type 'echo '"'"'tools = ["node@22"]'"'"' > .runxrc'
Enter
Sleep 500ms
Type "cat .runxrc"
Enter
Sleep 2s
Type "echo"
Enter
Type "runx -- node -v"
Enter
Sleep 4s

Sleep 2s

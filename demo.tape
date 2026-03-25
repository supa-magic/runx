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
Env npm_config_yes "true"

# Install
Type "echo '📦 Install runx'"
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

# React + Vite frontend
Type "echo"
Enter
Type "echo '⚛️  Create a React app with Vite'"
Enter
Sleep 1s
Type "runx --with node@22 -- npx create-vite my-app --template react"
Enter
Sleep 12s
Type "clear"
Enter
Sleep 500ms

# Python backend
Type "echo"
Enter
Type "echo '🐍 Python backend'"
Enter
Sleep 1s
Type "runx --with python@3.12 -- python3 --version"
Enter
Sleep 6s

# Both together
Type "echo"
Enter
Type "echo '🚀 Full-stack: Node + Python together'"
Enter
Sleep 1s
Type "runx --with node@22 --with python@3.12 -- node -v"
Enter
Sleep 6s

Sleep 2s

# runx demo — https://github.com/supa-magic/runx
# Record with: runx clean -y; rm -rf my-app; vhs demo.tape

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
Type "curl -fsSL https://raw.githubusercontent.com/supa-magic/runx/main/install.sh | sh"
Enter
Sleep 10s
Type 'export PATH="$HOME/.runx/bin:$PATH"'
Enter
Sleep 1s
Type "clear"
Enter
Sleep 500ms

# React + Vite
Type "runx --with node -- npx --yes create-vite my-app --template react"
Enter
Sleep 15s
Type "clear"
Enter
Sleep 500ms

# Go backend
Type "runx --with go -- go version"
Enter
Sleep 6s

# Both together
Type "runx --with node --with go -- node -v"
Enter
Sleep 4s

Sleep 2s

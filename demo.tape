# runx demo — https://github.com/supa-magic/runx
# Record with: vhs demo.tape

Output demo.gif
Set Theme "Dracula"
Set FontSize 16
Set Width 900
Set Height 500
Set Padding 20
Set TypingSpeed 40ms

# Hero: Node.js
Type "# Run any version of Node.js — no nvm needed"
Enter
Sleep 500ms
Type "runx --with node@22 -- node -v"
Enter
Sleep 4s

# Python
Type "# Python — no pyenv, no conda"
Enter
Sleep 500ms
Type "runx --with python@3.12 -- python3 --version"
Enter
Sleep 4s

# Go
Type "# Go — no goenv"
Enter
Sleep 500ms
Type "runx --with go -- go version"
Enter
Sleep 4s

# Cached — instant
Type "# Cached — starts instantly"
Enter
Sleep 500ms
Type "time runx --with node@22 -- node -v"
Enter
Sleep 3s

# Version pinning with .runxrc
Type "# Pin versions for your whole team"
Enter
Sleep 500ms
Type "cat .runxrc"
Enter
Sleep 2s
Type "runx -- node -v"
Enter
Sleep 3s

Sleep 2s

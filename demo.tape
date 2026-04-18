# demo.tape — record the hero demo gif for README.
#
# Render with:
#   vhs demo.tape            # produces demo.gif
#
# Requires:
#   - vhs (https://github.com/charmbracelet/vhs)
#   - `hey` v0.5.0+ installed on PATH (cargo install ai-in-terminal)
#   - an authenticated backend
#
# For a tight, snappy demo, export ANTHROPIC_API_KEY before running vhs so
# the claude backend uses the direct-API fast path (~2s) instead of the
# subprocess (~6s). Otherwise bump the Sleep after Enter to ~10s.
#
# The generated demo.gif is NOT gitignored — it ships as the README hero.

Output demo.gif

Set Shell zsh
Set FontSize 22
Set Width 1800
Set Height 1000
Set Framerate 60
Set Padding 40
Set TypingSpeed 55ms
Set CursorBlink true

# Play back the gif at 1.75x so the 6s claude-subprocess wait feels snappy.
# Typing is also sped up (55ms → ~31ms per char) but stays readable.
Set PlaybackSpeed 1.75

# Pure black background, soft neutral foreground, Dracula-ish accents so the
# Claude sparkle (orange), Codex arrow (blue), and risk chips read cleanly.
Set Theme { "name": "hey-black", "background": "#000000", "foreground": "#e6e6e6", "cursor": "#ffb86c", "selection": "#44475a", "black": "#000000", "red": "#ff5555", "green": "#50fa7b", "yellow": "#f1fa8c", "blue": "#8be9fd", "purple": "#bd93f9", "cyan": "#8be9fd", "white": "#e6e6e6", "brightBlack": "#6272a4", "brightRed": "#ff6e6e", "brightGreen": "#69ff94", "brightYellow": "#ffffa5", "brightBlue": "#d6acff", "brightPurple": "#ff92df", "brightCyan": "#a4ffff", "brightWhite": "#ffffff" }

Hide
Type "clear"
Enter
Show

Sleep 600ms
Type "hey list the 5 largest files in this repo"
Sleep 400ms
Enter

# Holds until claude returns and renders the confirm prompt. Claude's `-p`
# subprocess path takes ~6s; with ANTHROPIC_API_KEY exported the direct API
# is ~2s and this sleep can drop to 3s.
Sleep 8s

# Hold on the "▶ run?" prompt so the viewer can read it before the gif loops.
Sleep 2500ms

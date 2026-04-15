# hey

[![crates.io](https://img.shields.io/crates/v/ai-in-terminal.svg?style=flat-square&logo=rust)](https://crates.io/crates/ai-in-terminal)
[![downloads](https://img.shields.io/crates/d/ai-in-terminal.svg?style=flat-square)](https://crates.io/crates/ai-in-terminal)
[![license](https://img.shields.io/crates/l/ai-in-terminal.svg?style=flat-square)](./LICENSE)
[![ci](https://img.shields.io/github/actions/workflow/status/subinium/hey-cli/ci.yml?branch=main&style=flat-square&label=ci)](https://github.com/subinium/hey-cli/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/subinium/hey-cli?style=flat-square&label=release)](https://github.com/subinium/hey-cli/releases/latest)

> `hey` — talk to your terminal in plain English. Speaks **Claude**, **Codex**, and **OpenRouter**.

```
$ hey find 3 largest files in Downloads

  ╭─ hey · ✱ claude · headless
  │
  │  ✱  here you go
  │
  │  $ eza --icons --color=always --git -lt ~/Downloads | head -3
  │
  │  sort by newest and show the top three
  │
  ╰─❯  [Y]es  [n]o  [e]dit  › _
```

`hey` is for the moment you *know* what you want from the shell but can't remember whether it's `find -mtime -7` or `-7 -mtime`, which `sed` flag strips trailing whitespace, or how on earth `awk` builds a field separator. You type the intent, `hey` hands you the command, you confirm, it runs.

Nothing more. No interactive chat, no file reading, no project context. Just: *request → command → execute*.

## Why

- `claude code` is overkill for *"give me a regex for Korean characters"*.
- `sgpt` / `ai-shell` default to OpenAI and feel heavy.
- GitHub Copilot CLI requires you to leave your shell.

`hey` is a single ~2.7 MB Rust binary with sub-50ms cold start. It speaks three backends interchangeably — if you already pay for Claude or Codex, you pay nothing extra.

## Install

### From crates.io (recommended)

```sh
cargo install ai-in-terminal
```

This installs the `hey` binary to `~/.cargo/bin/hey`. Make sure `~/.cargo/bin` is on your `PATH`.

### From pre-built binaries

Grab a tarball for your platform from the [latest release](https://github.com/subinium/hey-cli/releases/latest) and drop the `hey` binary into your `PATH`:

```sh
# macOS (Apple Silicon)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-aarch64-apple-darwin.tar.gz | tar xz
sudo mv hey /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-x86_64-apple-darwin.tar.gz | tar xz

# Linux (x86_64)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-x86_64-unknown-linux-gnu.tar.gz | tar xz
```

### From source

```sh
git clone https://github.com/subinium/hey-cli
cd hey-cli
cargo install --path .
```

> The crate on crates.io is **`ai-in-terminal`**; the installed binary is **`hey`**.

### First-time setup

`hey` needs at least one backend configured. Pick whichever you already have:

```sh
# Option A — Claude Code (uses your existing login, nothing to configure)
which claude   # make sure Claude Code is installed and on PATH

# Option B — Codex CLI (uses your existing login)
which codex

# Option C — OpenRouter (HTTP API, fastest cold start)
export OPENROUTER_API_KEY=sk-or-...
```

If more than one is available, `hey` picks them in order: `claude → codex → openrouter`. Override with `-c`, `-x`, `-b openrouter`, or `AIT_BACKEND`.

## Quick start

```sh
$ hey find the 5 largest files in ~/Downloads

  ╭─ hey · ✱ claude · headless
  │
  │  ✱  here you go
  │
  │  $ du -ah ~/Downloads | sort -rh | head -5
  │
  │  list everything under Downloads with sizes, sort descending, keep the top five
  │
  ╰─❯  [Y]es  [n]o  [e]dit  › _
```

Press **Enter** to run the command, **n** to abort, **e** to edit it first.

## Usage

```sh
hey <natural-language prompt>
hey <backend> <prompt>   # force a specific backend
```

The first positional word is treated as a backend selector if it matches `claude`, `codex`, `openrouter` (alias: `or`), or `auto`. Otherwise the whole line is the prompt.

```sh
hey find all files over 100mb modified this week
hey claude explain this regex: \d{3}-\d{4}
hey codex summarize the last 10 commits
hey openrouter kill the process on port 3000
hey or show disk usage by directory    # or = openrouter
```

> **Flags go before the prompt**: `hey -n claude list files`, not `hey claude -n list files`.

### Flags

| Flag | Description |
|---|---|
| `-y`, `--yes` | Skip the confirm prompt, run immediately (blocked commands still blocked) |
| `-n`, `--dry-run` | Print the command, don't run it |
| `-e`, `--explain` | Force the model to add a one-line explanation (pipes get one automatically) |
| `-c`, `--claude` | Force Claude Code backend |
| `-x`, `--codex` | Force Codex CLI backend |
| `-b`, `--backend <name>` | `auto` (default) / `claude` / `codex` / `openrouter` |
| `-m`, `--model <id>` | Override model (OpenRouter only) |
| `--raw` | Disable the eza/bat/tree prettifier, run exactly what the model said |

## Backends

`hey` picks the first available backend in this order unless you override it:

| Backend | Icon | Color | Voice | How it runs | Auth |
|---|---|---|---|---|---|
| **Claude Code** | `✱` | orange `215` | *"here you go"* | `claude -p --system-prompt …` | your Claude Code login |
| **Codex CLI** | `☁` | sky `111` | *"computed"* | `codex exec -o …` | your Codex login |
| **OpenRouter** | `◆` | amber `222` | *"cooked"* | HTTPS → `/v1/chat/completions` | `OPENROUTER_API_KEY` |

Each backend gets its own header icon and a small voice line — so you know at a glance who answered. When a command is risky, the voice changes ("*this one has a sharp edge*", "*careful — you should run this one yourself*").

```
  ╭─ hey · ☁ codex · exec              ← Codex
  │
  │  ☁  computed
  │
  │  $ git log --oneline --since='10 commits ago'
  │
  ╰─
```

### Configuring backends

```sh
export AIT_BACKEND=claude          # default backend
export AIT_MODEL=anthropic/claude-haiku-4.5   # default OpenRouter model
export OPENROUTER_API_KEY=sk-or-...
```

No config file. Everything is flags + env.

## Pretty by default

`hey` runs your command through a post-processor that swaps in modern replacements when they're installed:

| When the model says… | `hey` runs… | Condition |
|---|---|---|
| `ls …` | `eza --icons --color=always --git …` | `eza` on PATH |
| `tree …` | `eza --tree --icons --color=always …` | `eza` on PATH |
| `tree …` | `tree -C …` | fallback |
| `cat file.json` | `jq --color-output . file.json` | `jq` on PATH |
| `cat file` | `bat --color=always --style=numbers --paging=never file` | `bat` on PATH, no pipe |
| `grep …` | `grep --color=always …` | `--color` not already set |
| `diff …` | `diff … \| delta` | `delta` on PATH |

Bare `ls` without `eza` still gets BSD color via `CLICOLOR_FORCE=1` in the child env.

Opt out per-invocation with `--raw`.

## Risk gate

Some commands should never execute without a human in the loop, no matter what flag you passed. `hey` has a small hard-coded list:

**Always blocked** (printed + copied to clipboard, never executed):
`rm`, `del`, `rmdir`, `shred`, `unlink`, `find … -delete`, `find … -exec rm`, `xargs rm`, `dd`, `mkfs`, `fdisk`, `wipefs`, `sfdisk`, `parted`, `> /dev/sd*`, `git reset --hard`, `git clean -fd`, the classic fork bomb.

When a command is blocked, the box shows a red **BLOCKED** chip, a short risk note, and the command is `pbcopy`'d to your clipboard so you can paste it into your shell after you've decided it's safe.

```
  ╭─ hey · ✱ claude · headless   BLOCKED
  │
  │  ✱  careful — you should run this one yourself
  │
  │  $ find . -name "*.log" -delete
  │
  │  removes all .log files recursively
  │
  │  ▲  `find -delete` removes files — copied to clipboard
  │
  ╰─
  copied to clipboard · paste & run manually
```

**Soft-warned** (runs, but shows a yellow **warn** chip):
`sudo`, `mv` without `-i`, `chmod`, `chown`, `>` (truncating redirect), `curl … | sh`, `kill`/`killall`/`pkill`.

This is *not* a safety net. You are. The gate is a tripwire for obvious foot-guns.

## Confirmation prompt

```
  ╰─❯  [Y]es  [n]o  [e]dit  › 
```

- `y` / enter — run it
- `n` — abort
- `e` — edit the command first; empty input keeps the original

## Keyboard workflow

1. Type `hey <what you want>`
2. Scan the printed command for 1 second
3. Hit Enter
4. Done

Pair with a shell keybinding if you want it even faster:

```zsh
# zsh: bind ^G to drop `hey ` at the cursor
bindkey -s '^G' 'hey '
```

## Recipes

```sh
hey find files newer than a week, sort by size
hey claude explain what this awk does: {for(i=1;i<=NF;i++)a[$i]++}END{for(k in a)print k,a[k]}
hey codex generate a commit message from the staged diff
hey convert all .png in ~/Pictures to jpg, keep originals
hey kill everything listening on port 5173
hey or show git log for the last month by myself
hey -y git branches merged into main    # auto-run
```

## Configuration

| Env var | Effect |
|---|---|
| `AIT_BACKEND` | Default backend (`auto`/`claude`/`codex`/`openrouter`) |
| `AIT_MODEL` | Default OpenRouter model id |
| `OPENROUTER_API_KEY` | Enables the OpenRouter backend |

## Design notes

- **Rust** for startup speed. Stripped binary is ~2.7 MB; cold-start is dominated by the network or subprocess call, not by `hey` itself.
- **Single file** (`src/main.rs`). No plugin system, no TOML config, no lifecycle hooks.
- **Backends are subprocesses** for Claude & Codex. Zero extra auth plumbing — if `claude` or `codex` works in your shell, it works in `hey`.
- **Tools disabled** in Claude backend: `hey` passes `--disallowedTools Bash,Edit,Write,Read,...` so Claude Code can only return text. No filesystem access, no tool use, no session state.
- **Sandboxed system prompt**: Claude's default agent prompt is *replaced* (not appended to) with `hey`'s synth-only prompt. This prevents it from refusing with *"this directory isn't in my allowed paths"*.
- **Conservative rewrites**: only commands whose first token matches a known preset get rewritten; everything else passes through untouched. `--raw` turns even those off.
- **Risk gate is a hard block**, not a prompt. Destructive commands cannot be `-y`'d into execution. You paste them yourself.

## Roadmap

- [ ] History / recall (`hey --last`, `hey --retry`)
- [ ] `--copy` flag for always-clipboard mode
- [ ] Shell integration to pre-fill the prompt line instead of executing
- [ ] Per-directory `.heyrc` for project-specific presets
- [ ] Streaming output (token-by-token)
- [ ] Community risk-rule contributions

PRs welcome. Keep it small.

## License

MIT

---

<sub>`hey` started as `ait` (agent in terminal) — the package name on crates.io is still `ai-in-terminal` for that reason. The personas are stylistic; Claude, Codex, and OpenRouter are trademarks of their respective owners and `hey` is not affiliated with any of them.</sub>

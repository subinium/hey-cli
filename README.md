<div align="center">

# `hey`

### talk to your terminal in plain English

*one binary · three backends · zero config*

[![crates.io](https://img.shields.io/crates/v/ai-in-terminal.svg?style=for-the-badge&logo=rust&color=orange)](https://crates.io/crates/ai-in-terminal)
[![downloads](https://img.shields.io/crates/d/ai-in-terminal.svg?style=for-the-badge&color=green)](https://crates.io/crates/ai-in-terminal)
[![license](https://img.shields.io/crates/l/ai-in-terminal.svg?style=for-the-badge&color=blue)](./LICENSE)
[![ci](https://img.shields.io/github/actions/workflow/status/subinium/hey-cli/ci.yml?branch=main&style=for-the-badge&label=ci)](https://github.com/subinium/hey-cli/actions/workflows/ci.yml)

```sh
cargo install ai-in-terminal
```

</div>

---

## 🎬 See it

```
$ hey find the 5 largest files in Downloads

  ╭─ hey · ✱ claude · headless
  │
  │  ✱  here you go
  │
  │  $ du -ah ~/Downloads | sort -rh | head -5
  │
  │  list everything under Downloads with sizes, sort descending, take top 5
  │
  ╰─❯  [Y]es  [n]o  [e]dit  › _
```

Press **Enter** → it runs. **n** → abort. **e** → edit first.

That's the whole workflow.

---

## Why `hey`?

<table>
<tr>
<td width="33%" valign="top">

### ⚡ Instant

Sub-50ms cold start. ~2.7 MB binary. No config, no chat window, no project context to load. You type the intent, you get the command.

</td>
<td width="33%" valign="top">

### 🧠 Three brains

Speaks **Claude Code**, **Codex CLI**, and **OpenRouter** interchangeably. Each has its own icon, color, and voice. Pick one or let `hey` auto-route.

</td>
<td width="33%" valign="top">

### 🛡 Safe by default

Hard-coded risk gate blocks `rm`, `dd`, `mkfs`, `find -delete`, even when they're hidden inside `sh -c`. Dangerous commands go to your clipboard, never your shell.

</td>
</tr>
</table>

---

## 30-second tour

```sh
# ask in plain English — pick a backend automatically
hey find all files over 100mb modified this week

# force a backend by name
hey claude   explain this regex: \d{3}-\d{4}
hey codex    summarize the last 10 commits
hey openrouter kill the process on port 3000

# shortcuts: or = openrouter
hey or show disk usage by directory

# flags go before the prompt
hey -n   find files newer than a week    # dry-run, don't execute
hey -y   git branches merged into main   # auto-run, no confirm
hey -e   rename all .jpeg to .jpg        # with explanation
hey --raw ls                              # skip the eza/bat rewriter
```

---

## Three voices, one CLI

<table>
<tr>
<th>Backend</th><th>Icon</th><th>Voice</th><th>How it runs</th><th>Auth</th>
</tr>
<tr>
<td><b>Claude Code</b></td>
<td align="center"><code>✱</code></td>
<td><i>"here you go"</i></td>
<td><code>claude -p --system-prompt …</code></td>
<td>your existing Claude Code login</td>
</tr>
<tr>
<td><b>Codex CLI</b></td>
<td align="center"><code>☁</code></td>
<td><i>"computed"</i></td>
<td><code>codex exec -o …</code></td>
<td>your existing Codex login</td>
</tr>
<tr>
<td><b>OpenRouter</b></td>
<td align="center"><code>◆</code></td>
<td><i>"cooked"</i></td>
<td>HTTPS → <code>/v1/chat/completions</code></td>
<td><code>OPENROUTER_API_KEY</code></td>
</tr>
</table>

Each backend gets its own header:

```
  ╭─ hey · ✱ claude · headless         ← orange sparkle, "here you go"
  ╭─ hey · ☁ codex · exec              ← sky-blue cloud, "computed"
  ╭─ hey · ◆ openrouter · haiku-4.5    ← amber diamond, "cooked"
```

When a command is risky, the voice changes too: *"this one has a sharp edge"* or *"careful — you should run this one yourself"*.

---

## Install

### From crates.io

```sh
cargo install ai-in-terminal
```

The crate is `ai-in-terminal`; the installed binary is **`hey`**.

### From pre-built binaries

Grab a tarball for your platform from the [latest release](https://github.com/subinium/hey-cli/releases/latest):

```sh
# macOS (Apple Silicon)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-aarch64-apple-darwin.tar.gz | tar xz
sudo mv hey /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-x86_64-apple-darwin.tar.gz | tar xz

# Linux (x86_64)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-x86_64-unknown-linux-gnu.tar.gz | tar xz

# Linux (aarch64)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-aarch64-unknown-linux-gnu.tar.gz | tar xz
```

### From source

```sh
git clone https://github.com/subinium/hey-cli
cd hey-cli
cargo install --path .
```

### First-time setup

Pick whichever backend you already have — `hey` figures out the rest:

```sh
# Option A — Claude Code (nothing to configure if you're already logged in)
which claude

# Option B — Codex CLI
which codex

# Option C — OpenRouter (fastest cold start)
export OPENROUTER_API_KEY=sk-or-...
```

If more than one is available, `hey` picks them in order: `claude → codex → openrouter`.

---

## Risk gate

Some commands should never run through an AI-generated autopilot. `hey` has a hard-coded list — these are **always blocked**, even with `-y`:

```
rm · del · rmdir · shred · unlink
find ... -delete · find ... -exec rm · xargs rm
dd · mkfs · fdisk · wipefs · sfdisk · parted
> /dev/sd*  ·  :(){ :|:& };:   ← fork bomb
git reset --hard · git clean -fd
```

**The gate is substring-aware** — it catches `rm` hidden inside `sh -c 'rm ...'`, `find -exec sh -c 'rm "$1"'`, and similar wrappers. Blocked commands are **copied to your clipboard** so you can paste them yourself after you've decided they're safe.

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

Soft warnings (runs, but shows a yellow `warn` chip): `sudo`, `cd`, `mv` without `-i`, `chmod`, `chown`, `>` (truncating redirect), `curl | sh`, `kill`/`killall`/`pkill`.

> **This is not a safety net.** It's a tripwire for obvious foot-guns. You're still responsible for what you run.

---

## Pretty by default

When the model picks a common tool, `hey` transparently swaps in its modern replacement — but only when there's zero risk of flag mismatches:

<table>
<tr><th>Model says</th><th>hey runs</th><th>When</th></tr>
<tr><td><code>ls</code> (no flags)</td><td><code>eza --icons --color=always --git --long --header</code></td><td><code>eza</code> installed</td></tr>
<tr><td><code>tree</code> (no flags)</td><td><code>eza --tree --icons --color=always</code></td><td><code>eza</code> installed</td></tr>
<tr><td><code>tree</code> (fallback)</td><td><code>tree -C</code></td><td>always</td></tr>
<tr><td><code>cat foo.json</code></td><td><code>jq --color-output . foo.json</code></td><td><code>jq</code> installed</td></tr>
<tr><td><code>cat foo</code></td><td><code>bat --color=always --style=numbers --paging=never foo</code></td><td><code>bat</code> installed</td></tr>
<tr><td><code>grep</code></td><td><code>grep --color=always</code></td><td>no <code>--color</code> set</td></tr>
<tr><td><code>diff</code></td><td><code>diff … | delta</code></td><td><code>delta</code> installed</td></tr>
</table>

Anything with flags passes through untouched. Bare `ls` without `eza` still gets BSD color via `CLICOLOR_FORCE=1`. Use `--raw` to disable all rewrites per-invocation.

---

## Flags & env

<details>
<summary><b>All flags</b></summary>

| Flag | Description |
|---|---|
| `-y`, `--yes` | Skip the confirm prompt, run immediately (blocked commands still blocked) |
| `-n`, `--dry-run` | Print the command, don't run it |
| `-e`, `--explain` | Force the model to add a one-line explanation (pipes get one automatically) |
| `-c`, `--claude` | Force Claude Code backend |
| `-x`, `--codex` | Force Codex CLI backend |
| `-b`, `--backend <name>` | `auto` (default) / `claude` / `codex` / `openrouter` |
| `-m`, `--model <id>` | Override model (OpenRouter only) |
| `--raw` | Disable the eza/bat/tree prettifier |

> Flags must come **before** the prompt / backend name: `hey -n claude list files`, not `hey claude -n list files`.

</details>

<details>
<summary><b>Environment variables</b></summary>

| Var | Effect |
|---|---|
| `OPENROUTER_API_KEY` | Enables the OpenRouter backend |
| `AIT_BACKEND` | Default backend (`auto` / `claude` / `codex` / `openrouter`) |
| `AIT_MODEL` | Default OpenRouter model id (e.g. `anthropic/claude-haiku-4.5`) |

No config file. Everything is flags + env.

</details>

---

## Power-user tricks

**Bind a key in zsh** to drop `hey ` at the cursor:

```zsh
bindkey -s '^G' 'hey '
```

Now `Ctrl-G` → `hey ` → type your question → Enter → done.

**Recipe book:**

```sh
hey find files newer than a week, sort by size
hey claude explain what this awk does: {for(i=1;i<=NF;i++)a[$i]++}END{for(k in a)print k,a[k]}
hey codex generate a commit message from the staged diff
hey convert all .png in ~/Pictures to jpg, keep originals
hey kill everything listening on port 5173
hey or show git log for the last month by myself
hey -y git branches merged into main
```

---

## How it stays small

- **Rust**, single `src/main.rs`, ~800 lines. No plugin system, no TOML config, no lifecycle hooks.
- **Backends are subprocesses** for Claude & Codex — zero extra auth plumbing. If `claude` works in your shell, it works in `hey`.
- **Claude backend runs with all tools disabled** (`--disallowedTools Bash,Edit,Write,Read,...`) and a *replaced* system prompt, so it can only return text. No filesystem access, no tool use, no session state — just pure synth.
- **Risk gate is a hard block**, not a prompt. Destructive commands cannot be `-y`'d into execution.
- **Conservative rewrites**: only bare commands get prettified; anything with flags passes through.

---

## Roadmap

- [ ] History / recall — `hey --last`, `hey --retry`
- [ ] Shell function wrapper so `cd`/`export`/`source` can actually change the parent shell
- [ ] Per-directory `.heyrc` for project-specific system prompt additions
- [ ] Streaming output
- [ ] Community risk-rule contributions
- [ ] Homebrew tap

PRs welcome — keep them small.

---

<div align="center">

**MIT** · built in an afternoon · `hey` has no affiliation with Anthropic, OpenAI, or OpenRouter

<sub>The crate is <code>ai-in-terminal</code> on crates.io; <code>hey</code> started as the acronym <code>ait</code> (<i>agent in terminal</i>)</sub>

</div>

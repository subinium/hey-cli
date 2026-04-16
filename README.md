<div align="center">

# `hey`

### talk to your terminal in natural language

*works in English, Korean, Japanese, or whatever you type · one binary · three backends · zero config*

[![crates.io](https://img.shields.io/crates/v/ai-in-terminal.svg?style=for-the-badge&logo=rust&color=orange)](https://crates.io/crates/ai-in-terminal)
[![downloads](https://img.shields.io/crates/d/ai-in-terminal.svg?style=for-the-badge&color=green)](https://crates.io/crates/ai-in-terminal)
[![license](https://img.shields.io/crates/l/ai-in-terminal.svg?style=for-the-badge&color=blue)](./LICENSE)
[![ci](https://img.shields.io/github/actions/workflow/status/subinium/hey-cli/ci.yml?branch=main&style=for-the-badge&label=ci)](https://github.com/subinium/hey-cli/actions/workflows/ci.yml)

```sh
cargo install ai-in-terminal
```

</div>

---

## See it in action

```
$ hey claude find the 5 largest files in Downloads

  ✱ claude · headless

   ▐▛███▜▌  here you go
  ▝▜█████▛▘
    ▘▘ ▝▝

  $ du -ah ~/Downloads | sort -rh | head -5

  list everything under Downloads with sizes, sort descending, take top 5

  ▶ run? Y (default) / N
```

That's the whole workflow: **type → read → enter**.

---

## Meet the crew

Each backend has its own **character**, **color**, and **voice**.

<table>
<tr>
<td align="center" width="33%">

```
   ▐▛███▜▌
  ▝▜█████▛▘
    ▘▘ ▝▝
```

**✱ Claude**

*"here you go"*

`claude -p` · your subscription

</td>
<td align="center" width="33%">

```
  ▄
   ▀▄
  ▄▀ ▄▄▄▄▄
```

**☁ Codex**

*"computed"*

`codex exec` · your subscription

</td>
<td align="center" width="33%">

```
    /\
   <◆>
    \/
```

**◆ OpenRouter**

*"cooked"*

HTTP API · `OPENROUTER_API_KEY`

</td>
</tr>
</table>

`hey` auto-detects: **claude → codex → openrouter**. Your subscription always comes first.

---

## 30-second tour

```sh
# just ask — hey picks the best available backend
hey find all files over 100mb modified this week
hey 최근 일주일 안에 수정된 파일 보여줘
hey 100メガより大きいファイルを探して

# or name your backend
hey claude   explain this regex: \d{3}-\d{4}
hey codex    summarize the last 10 commits
hey openrouter kill the process on port 3000

# flags go before the prompt
hey -n  find files newer than a week     # dry-run
hey -y  git branches merged into main    # auto-run
hey -e  rename all .jpeg to .jpg         # with explanation
```

---

## Why `hey`?

<table>
<tr>
<td width="33%" valign="top">

### ⚡ Instant

Sub-50ms binary start. One sentence in, one command out. No chat session, no project context, no waiting for a UI to load.

</td>
<td width="33%" valign="top">

### 💸 Subscription-first

Already pay for Claude Code or Codex? Use them inline — zero extra cost. OpenRouter is the fallback, not the default.

</td>
<td width="33%" valign="top">

### 🛡 Safe by default

`rm`, `dd`, `mkfs`, `find -delete` are **always blocked** — even wrapped in `sh -c`. Blocked commands go to your clipboard, never your shell.

</td>
</tr>
</table>

---

## Install

### From crates.io (recommended)

```sh
cargo install ai-in-terminal
```

> **Why two names?** The crate is `ai-in-terminal` (searchable), the binary is `hey` (what you type). Cargo handles this — no alias, no symlink.

### From pre-built binaries

Grab from the [latest release](https://github.com/subinium/hey-cli/releases/latest):

```sh
# macOS (Apple Silicon)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-aarch64-apple-darwin.tar.gz | tar xz
sudo mv hey /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-x86_64-apple-darwin.tar.gz | tar xz

# Linux (x86_64 / aarch64)
curl -L https://github.com/subinium/hey-cli/releases/latest/download/hey-x86_64-unknown-linux-gnu.tar.gz | tar xz
```

### First-time setup

Pick whichever backend you already have:

```sh
which claude   # Option A — Claude Code (nothing to configure)
which codex    # Option B — Codex CLI
export OPENROUTER_API_KEY=sk-or-...  # Option C — OpenRouter
```

---

## Risk gate

Some commands should **never** auto-run. `hey` hard-blocks these even with `-y`:

```
rm · del · rmdir · shred · unlink
find -delete · find -exec rm · xargs rm
dd · mkfs · fdisk · wipefs · git reset --hard
```

The gate unwraps `sh -c '...'` wrappers — `rm` hidden inside `find -exec sh -c 'rm "$1"'` is still caught.

```
  ✱ claude · headless   BLOCKED

   ▐▛███▜▌  careful — you should run this one yourself
  ▝▜█████▛▘
    ▘▘ ▝▝

  $ find . -name "*.log" -delete

  ▲  `find -delete` removes files — copied to clipboard

  copied to clipboard · paste & run manually
```

Soft warnings (yellow `warn` chip, still runs): `sudo`, `mv`, `chmod`, `curl | sh`, `kill`.

---

## Pretty by default

`hey` swaps bare commands for modern alternatives when installed:

| Model says | `hey` runs | When |
|---|---|---|
| `ls` | `eza --icons --color=always --git` | `eza` installed, no flags |
| `tree` | `eza --tree --icons --color=always` | `eza` installed, no flags |
| `cat foo.json` | `jq --color-output . foo.json` | `jq` installed |
| `cat foo` | `bat --color=always --paging=never foo` | `bat` installed |
| `grep` | `grep --color=always` | no `--color` set |
| `diff` | `diff … \| delta` | `delta` installed |

Commands with flags pass through untouched. Use `--raw` to disable all rewrites.

---

## Security

- **Content filter** blocks prompts containing API keys (`sk-ant-`, `AKIA`, `ghp_`, etc.) and private key headers before they reach any backend.
- **Risk gate** is substring-aware — catches destructive commands even inside shell wrappers.
- **Tools disabled** in Claude backend — `--disallowedTools` strips all filesystem access; the model can only return text.
- **No telemetry**, no analytics, no data collection. Your prompts go to the backend you chose and nowhere else.

Override the content filter with `--allow-sensitive` when you genuinely need to reference a key pattern.

---

<details>
<summary><b>All flags & env vars</b></summary>

### Flags

| Flag | Description |
|---|---|
| `-y`, `--yes` | Skip confirm, run immediately (blocked commands still blocked) |
| `-n`, `--dry-run` | Print the command, don't run it |
| `-e`, `--explain` | Force a one-line explanation |
| `-c`, `--claude` | Force Claude Code backend |
| `-x`, `--codex` | Force Codex CLI backend |
| `-b`, `--backend <name>` | `auto` / `claude` / `codex` / `openrouter` |
| `-m`, `--model <id>` | Override model (OpenRouter only) |
| `--raw` | Disable eza/bat/tree prettifier |
| `--allow-sensitive` | Allow prompts with API key patterns |

> Flags go **before** the prompt: `hey -n claude list files`

### Environment variables

| Var | Effect |
|---|---|
| `OPENROUTER_API_KEY` | Enables OpenRouter backend |
| `ANTHROPIC_API_KEY` | Enables direct Anthropic API (~2s vs ~6s for Claude) |
| `AIT_BACKEND` | Default backend |
| `AIT_MODEL` | Default OpenRouter model id |

</details>

<details>
<summary><b>Recipe book</b></summary>

```sh
hey find files newer than a week, sort by size
hey claude explain what this awk does: {for(i=1;i<=NF;i++)a[$i]++}END{for(k in a)print k,a[k]}
hey codex generate a commit message from the staged diff
hey convert all .png in ~/Pictures to jpg, keep originals
hey kill everything listening on port 5173
hey or show git log for the last month by myself
hey -y git branches merged into main
hey 이 폴더에서 가장 큰 파일 5개 찾아줘
hey このディレクトリのRustファイルの行数を数えて
```

</details>

<details>
<summary><b>How it stays small</b></summary>

- **Rust**, 12 modules, ~1000 lines total. No plugin system, no TOML config, no lifecycle hooks.
- **Backends are subprocesses** for Claude & Codex — if `claude` works in your shell, it works in `hey`.
- **Direct Anthropic API** when `ANTHROPIC_API_KEY` is set — skips the `claude` subprocess for ~2s response time.
- **Auto-fallback chain** — if a backend fails (rate limit, auth), `hey` transparently retries the next one.
- **Conservative rewrites** — only bare commands get prettified; anything with flags passes through.

</details>

---

## Roadmap

- [ ] `hey --last` / `hey --retry` — command history & recall
- [ ] `hey init zsh` — shell function so `cd`/`export` work in the parent shell
- [ ] Per-directory `.heyrc` for project-specific prompts
- [ ] `hey doctor` — diagnose backends, tools, auth
- [ ] Streaming output
- [ ] Homebrew tap

PRs welcome — keep them small.

---

<div align="center">

**MIT** · built by [@subinium](https://github.com/subinium) · `hey` is not affiliated with Anthropic, OpenAI, or OpenRouter

<sub>The crate is <code>ai-in-terminal</code>; <code>hey</code> started as <code>ait</code> (<i>agent in terminal</i>)</sub>

</div>

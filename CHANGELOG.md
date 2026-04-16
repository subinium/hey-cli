# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.2] - 2026-04-16

### Changed

- **Claude backend fast-path** — when `ANTHROPIC_API_KEY` is set, `hey claude ...` calls the Anthropic Messages API directly via HTTP (~2s) instead of spawning `claude -p` (~6s). Falls back to the subprocess when no key is present.
- **Code split into 12 modules** — `main.rs` went from 1056 lines to 224. Backend logic, risk gate, presets, sanitizer, UI, and style constants each live in their own file.
- **ANSI constants extracted** — duplicated `\x1b[...` escape codes replaced with named constants in `src/style.rs` (`GRAY`, `RESET`, `DIM`, `BOLD_WHITE`, etc.).
- **`which_bin()` now cached** — binary lookups (`eza`, `bat`, `jq`, `delta`, `claude`, `codex`) are scanned once at startup into a `OnceLock<HashMap>` instead of re-scanning PATH on every call.
- **`prettify_command` inlined** — thin wrapper removed; callers use `apply_presets()` directly.
- **Function-level `use` imports** moved to module scope.

## [0.2.1] - 2026-04-16

### Added

- **Content filter** — prompts containing API keys (`sk-ant-`, `sk-or-`, `AKIA...`, `ghp_`, `gho_`, `glpat-`, `xoxb-`, `xoxp-`), private key headers (`-----BEGIN RSA PRIVATE`, etc.) are blocked before being sent to any backend. Shows which pattern matched. Override with `--allow-sensitive`.

### Improved

- **System prompt compressed** — ~600 tokens → ~120 tokens, saving ~480 input tokens per request. Prompt text is stable to maximize OpenRouter's automatic 5-minute cache TTL.
- **max_tokens reduced** — 512 → 256. Shell commands are rarely > 50 tokens; tighter cap reduces latency and cost.
- **HTTP-Referer/X-Title headers** — updated from old `ait` branding to `hey-cli`.

## [0.2.0] - 2026-04-15

### Added

- **Auto-fallback chain** — when `hey` runs in auto mode and the first backend fails (rate limit, auth, network), it surfaces a short inline notice and transparently retries with the next available backend. Chain is `claude → codex → openrouter`, filtered by what's installed.
- **Codex rate-limit detection** — `hey` now inspects Codex stdout/stderr/output-file for rate-limit and quota markers and produces a friendly `"codex is rate-limited — try 'hey claude ...' or 'hey openrouter ...' instead"` instead of a raw subprocess error.
- **Codex auth-error detection** — `"not authenticated"` errors from `codex exec` now produce a clear `"run 'codex login' first"` message.

### Fixed

- `hey:` error prefix (was still showing the old `ait:` project name)
- Codex backend swallowing stdout — useful diagnostic info is now aggregated from stdout, stderr, and the `-o` output file before deciding what error to show.

## [0.1.0] - 2026-04-15

Initial release.

### Added

- `hey <prompt>` — convert natural language to a shell command and confirm before running
- Three interchangeable backends: Claude Code headless, Codex CLI headless, OpenRouter HTTP API
- Auto backend selection: `claude → codex → openrouter`
- Subcommand-style backend override: `hey claude ...`, `hey codex ...`, `hey openrouter ...`
- Per-backend personas with icons and voices (`✱ claude`, `☁ codex`, `◆ openrouter`)
- Risk gate that blocks destructive commands (`rm`, `dd`, `mkfs`, `find -delete`, `-exec rm`, shell-wrapped `rm`, fork bomb, `git reset --hard`, etc.) and copies them to the clipboard instead
- Soft warnings for `sudo`, `cd`, `mv`, `chmod`, `chown`, truncating redirects, `curl | sh`, `kill`
- Conservative command presets: bare `ls`/`tree`/`cat` get rewritten to `eza`/`jq`/`bat` when available
- `--raw` flag to disable all rewrites
- `-y` / `--yes` auto-confirm, `-n` / `--dry-run`, `-e` / `--explain`
- Fenced-code-block sanitizer so model responses with triple-backtick wrappers are parsed correctly
- Strict prose detection — bail out if the backend returns non-command text

[0.2.2]: https://github.com/subinium/hey-cli/releases/tag/v0.2.2
[0.2.1]: https://github.com/subinium/hey-cli/releases/tag/v0.2.1
[0.2.0]: https://github.com/subinium/hey-cli/releases/tag/v0.2.0
[0.1.0]: https://github.com/subinium/hey-cli/releases/tag/v0.1.0

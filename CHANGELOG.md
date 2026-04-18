# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-04-18

### Fixed

- **Shell command line no longer erased** — `print_thinking` counted the `art[0]` / fallback `thinking…` row as an extra line advance even though it had no trailing `\n`. `clear_thinking` then walked one line too far up, and the final `\r\x1b[J` wiped the user's original `$ hey …` prompt line from the terminal. The off-by-one is fixed, so the shell command that invoked `hey` stays visible in scrollback.
- **Thinking block spacing matches the result view** — added a blank row between the backend label and the mascot so the thinking layout has the same vertical rhythm as `print_command_block`'s output. Previously the label and mascot touched during thinking, then a gap appeared once the result rendered.

## [0.4.0] - 2026-04-17

Security audit + UX pass. Breaking in a few narrow places but safer everywhere.

### Security

- **Risk gate rewritten** with segment-aware parsing so command substitutions (`$(rm ...)`, `<(rm ...)`, `>(rm ...)`, backticks) and process substitution now classify correctly instead of silently bypassing as Safe.
- **Whitespace obfuscation blocked** — `rm` hidden by tabs, `$IFS`, `${IFS}` is caught.
- **Interpreter guard** — `python -c`, `perl -e`, `node -e`, `ruby -e`, `awk 'BEGIN{...'` surface Warn so obfuscated destructive calls don't land as Safe.
- **Decoded shell pipes blocked** — `base64 -d | sh`, `xxd -r | bash`, `openssl enc -d | sh` now Block.
- **More destructive primitives blocked** — `truncate`, `: > file`, `true > file`, `cp /dev/null target`, `git reset --hard`, `git clean -fd`, `git push --force`.
- **`rm` false-positive fixed** — v0.3 blocked anything containing the token `rm` (including `grep rm logfile`). v0.4 only blocks when `rm` is a command-position first token of a segment.
- **Content filter expanded** — fine-grained GitHub PATs (`ghu_`, `ghs_`, `ghr_`, `github_pat_`), Stripe, SendGrid, HuggingFace, JWT, Google service-account JSON, Azure storage, and any URL with embedded credentials (`scheme://user:pass@host`).
- **Codex tempfile hardened** — replaced predictable `/tmp/ait-codex-<pid>.txt` with `tempfile::NamedTempFile` (O_EXCL, mode 0600, auto-cleanup on Drop). Closes a symlink/TOCTOU vector.
- **Codex prompt via stdin** — was visible in `ps` / `/proc/<pid>/cmdline`; now piped via stdin.
- **HTTP response size cap (1 MiB)** — OpenRouter and direct Anthropic API responses are Content-Length-checked and capped during read.
- **ANSI escape sanitization** — model output and backend error text is stripped of ESC sequences (CSI / OSC) before display, closing OSC-52 clipboard-injection.
- **`hey doctor` no longer leaks key body** — shows only the public prefix (`sk-or-v1-****`).
- 19 unit tests added for the risk gate.

### UX

- **`hey doctor`** — new diagnostic subcommand. Shows detected backends, preset tools, shell/TTY state, env-var config, and the active auto chain.
- **`hey init <shell>`** — emits a shell completion script (bash / zsh / fish / powershell / elvish).
- **Stdin prompt support** — `echo "list docker containers" | hey --yes`. Requires `--yes` or `--dry-run` since confirm can't read.
- **EOF = abort** — pressing Ctrl-D (or piping `hey foo < /dev/null`) now aborts with `aborted (no input)` instead of silently running.
- **Warn requires explicit `y`** — for `Risk::Warn` commands the default is capital `N`; blank Enter aborts.
- **Stdout-not-TTY refusal** — `hey foo | head` used to hang; now errors with a suggestion to pass `--yes` or `--dry-run`.
- **Edit → copy & edit in shell** — `e` at the confirm prompt copies the command to your clipboard, reports whether the copy succeeded, and aborts. Paste in your shell for readline editing.
- **Richer help text and error messages** — `--help` documents subcommand-style backend selection; `OPENROUTER_API_KEY not set` points at `openrouter.ai/keys`; Claude `not authenticated` surfaces `claude login`.
- **Narrow-terminal-safe thinking animation** — uses `\x1b[J` so wrapped lines don't leave cruft on <70-col terminals.

### Breaking changes

- `or` shorthand for `openrouter` removed — too ambiguous with the English preposition.
- Backend subcommand matching is now case-sensitive lowercase — `hey Claude is fast` is a prompt.
- `Risk::Warn` no longer defaults to Yes on blank Enter.
- EOF on the confirm prompt aborts instead of running.
- `hey foo | head` refuses without `--yes` / `--dry-run`.

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

[0.4.0]: https://github.com/subinium/hey-cli/releases/tag/v0.4.0
[0.2.2]: https://github.com/subinium/hey-cli/releases/tag/v0.2.2
[0.2.1]: https://github.com/subinium/hey-cli/releases/tag/v0.2.1
[0.2.0]: https://github.com/subinium/hey-cli/releases/tag/v0.2.0
[0.1.0]: https://github.com/subinium/hey-cli/releases/tag/v0.1.0

use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "hey", version, about = "hey — natural language → shell command", long_about = None)]
pub(crate) struct Cli {
    /// Natural-language request. Prefix with `claude`, `codex`, or `openrouter` to pick a backend.
    /// e.g. `hey claude list big files`, `hey find files newer than a week`
    #[arg(trailing_var_arg = true, required = true)]
    pub prompt: Vec<String>,

    /// Skip confirmation and run immediately
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Print the command but don't execute it
    #[arg(short = 'n', long = "dry-run")]
    pub dry_run: bool,

    /// Also ask Claude to explain the command
    #[arg(short = 'e', long = "explain")]
    pub explain: bool,

    /// Override the model id (OpenRouter only)
    #[arg(short = 'm', long = "model", env = "AIT_MODEL")]
    pub model: Option<String>,

    /// Backend to use (auto picks claude → codex → openrouter)
    #[arg(short = 'b', long = "backend", env = "AIT_BACKEND", value_enum, default_value_t = Backend::Auto)]
    pub backend: Backend,

    /// Shortcut for `--backend claude` (Claude Code headless)
    #[arg(short = 'c', long = "claude", conflicts_with_all = ["backend", "codex"])]
    pub claude: bool,

    /// Shortcut for `--backend codex` (Codex CLI headless)
    #[arg(short = 'x', long = "codex", conflicts_with_all = ["backend", "claude"])]
    pub codex: bool,

    /// Disable command post-processing presets (eza/bat/tree -C)
    #[arg(long = "raw")]
    pub raw: bool,

    /// Allow sending prompts that contain sensitive patterns (API keys, private keys)
    #[arg(long = "allow-sensitive")]
    pub allow_sensitive: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Backend {
    /// Pick claude → codex → openrouter automatically
    Auto,
    /// OpenRouter HTTP API (requires OPENROUTER_API_KEY)
    Openrouter,
    /// Claude Code headless (`claude -p`) — uses your existing login
    Claude,
    /// Codex CLI headless (`codex exec`) — uses your existing login
    Codex,
}

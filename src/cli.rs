use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "hey",
    version,
    about = "natural language to shell commands, with personality",
    long_about = "natural-language to shell commands, with personality.\n\n\
        Your request goes to the first available backend (claude \u{2192} codex \u{2192} openrouter) \
        and comes back as a shell command ready to run. No quotes needed — just type what you want.\n\n\
        The first word may be a backend selector (`claude`, `codex`, `openrouter`, `auto`) or a \
        subcommand (`doctor`, `init <shell>`). Otherwise everything is the prompt.",
    after_help = "Examples:\n  \
        hey find files bigger than 100mb\n  \
        hey claude explain this regex '^[a-z]+$'\n  \
        hey codex list the 3 largest pdfs in ~/Downloads\n  \
        hey openrouter show git commits from last week\n  \
        echo 'list my docker containers' | hey --yes\n  \
        hey --dry-run tar the src folder\n  \
        hey doctor                # print environment diagnostics\n  \
        hey init zsh > ~/.zsh/completions/_hey"
)]
pub(crate) struct Cli {
    /// Natural-language request, or a subcommand (`doctor`, `init <shell>`).
    ///
    /// The first word may optionally be `claude`, `codex`, `openrouter`, or `auto`
    /// to pick a backend — e.g. `hey claude list big files`. Otherwise the whole
    /// argument list is treated as the prompt: `hey find files newer than a week`.
    #[arg(trailing_var_arg = true, required = false)]
    pub prompt: Vec<String>,

    /// Skip confirmation and run immediately
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Print the command but don't execute it
    #[arg(short = 'n', long = "dry-run")]
    pub dry_run: bool,

    /// Also ask the backend to explain the command
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

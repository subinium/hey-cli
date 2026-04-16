//! `hey init <shell>` — print a shell-completion script to stdout so the user
//! can redirect it into their completion path. Supports bash, zsh, fish,
//! elvish, and PowerShell via `clap_complete`.

use anyhow::{anyhow, Result};
use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::Cli;

pub(crate) fn run(shell_name: Option<&str>) -> Result<()> {
    let name = shell_name.ok_or_else(|| {
        anyhow!("missing shell name — try `hey init zsh`, `hey init bash`, or `hey init fish`")
    })?;
    let shell = parse_shell(name)?;
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    Ok(())
}

fn parse_shell(name: &str) -> Result<Shell> {
    match name.to_lowercase().as_str() {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        "fish" => Ok(Shell::Fish),
        "elvish" => Ok(Shell::Elvish),
        "powershell" | "pwsh" => Ok(Shell::PowerShell),
        other => Err(anyhow!(
            "unsupported shell: `{other}` — try one of: bash, zsh, fish, elvish, powershell"
        )),
    }
}

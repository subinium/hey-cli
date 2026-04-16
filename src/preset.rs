use crate::backend::which_bin;

pub(crate) fn apply_presets(command: &str) -> String {
    let trimmed = command.trim_start();
    let leading = &command[..command.len() - trimmed.len()];
    let Some(first_word) = trimmed.split_whitespace().next() else {
        return command.to_string();
    };
    let rest = trimmed[first_word.len()..].to_string();
    let rest_has_flag = rest.split_whitespace().any(|w| w.starts_with('-'));

    let replaced = match first_word {
        // `ls` and `tree` share some flags with eza but diverge on others (-t, -h, -S).
        // Only rewrite when the user's command has NO flags, so paths/globs still apply
        // but any flag falls through to the original binary untouched.
        "ls" if which_bin("eza") && !rest_has_flag => {
            format!("eza --icons --color=always --git --long --header{rest}")
        }
        "tree" if which_bin("eza") && !rest_has_flag => {
            format!("eza --tree --icons --color=always{rest}")
        }
        "tree" if !rest.contains(" -C") => {
            format!("tree -C{rest}")
        }
        "cat"
            if which_bin("bat")
                && !command.contains('|')
                && !command.contains('>')
                && !rest_has_flag =>
        {
            let looks_json = rest.trim_end().ends_with(".json");
            if looks_json && which_bin("jq") {
                format!("jq --color-output .{rest}")
            } else {
                format!("bat --color=always --style=numbers --paging=never{rest}")
            }
        }
        "grep" if !rest.contains("--color") && !rest.contains("-I") => {
            format!("grep --color=always{rest}")
        }
        "diff" if which_bin("delta") && !rest_has_flag => {
            format!("diff{rest} | delta")
        }
        _ => return command.to_string(),
    };
    format!("{leading}{replaced}")
}

use crate::backend::which_bin;

/// Returns the rewritten command and, if the rewrite swapped the binary,
/// a short label describing what changed (e.g. `"cat → jq"`). Pure flag
/// additions like `tree → tree -C` and `grep → grep --color` return `None`
/// for the label since those are unlikely to be confused with model output.
pub(crate) fn apply_presets(command: &str) -> (String, Option<&'static str>) {
    let trimmed = command.trim_start();
    let leading = &command[..command.len() - trimmed.len()];
    let Some(first_word) = trimmed.split_whitespace().next() else {
        return (command.to_string(), None);
    };
    let rest = trimmed[first_word.len()..].to_string();
    let rest_has_flag = rest.split_whitespace().any(|w| w.starts_with('-'));

    let (replaced, label): (String, Option<&'static str>) = match first_word {
        // `ls` and `tree` share some flags with eza but diverge on others (-t, -h, -S).
        // Only rewrite when the user's command has NO flags, so paths/globs still apply
        // but any flag falls through to the original binary untouched.
        "ls" if which_bin("eza") && !rest_has_flag => (
            format!("eza --icons --color=always --git --long --header{rest}"),
            Some("ls → eza"),
        ),
        "tree" if which_bin("eza") && !rest_has_flag => (
            format!("eza --tree --icons --color=always{rest}"),
            Some("tree → eza"),
        ),
        "tree" if !rest.contains(" -C") => (format!("tree -C{rest}"), None),
        "cat"
            if which_bin("bat")
                && !command.contains('|')
                && !command.contains('>')
                && !rest_has_flag =>
        {
            let looks_json = rest.trim_end().ends_with(".json");
            if looks_json && which_bin("jq") {
                (format!("jq --color-output .{rest}"), Some("cat → jq"))
            } else {
                (
                    format!("bat --color=always --style=numbers --paging=never{rest}"),
                    Some("cat → bat"),
                )
            }
        }
        "grep" if !rest.contains("--color") && !rest.contains("-I") => {
            (format!("grep --color=always{rest}"), None)
        }
        "diff" if which_bin("delta") && !rest_has_flag => {
            (format!("diff{rest} | delta"), Some("diff → delta"))
        }
        _ => return (command.to_string(), None),
    };
    (format!("{leading}{replaced}"), label)
}

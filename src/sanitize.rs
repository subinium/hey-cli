pub(crate) fn sanitize_command(raw: &str) -> String {
    let trimmed = raw.trim();

    // If the model wrapped the command in a fenced block anywhere in the output,
    // extract the first block's content — that's almost always the actual command.
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        // Strip optional language tag like "bash", "sh", "zsh", "shell"
        let after_lang = after_fence
            .split_once('\n')
            .map(|(lang, rest)| {
                let l = lang.trim();
                if l.is_empty() || l.chars().all(|c| c.is_ascii_alphabetic()) {
                    rest
                } else {
                    after_fence
                }
            })
            .unwrap_or(after_fence);
        if let Some(end) = after_lang.find("```") {
            return after_lang[..end].trim().to_string();
        }
    }

    // No fences. If the first line starts with obvious prose (CJK, refusals),
    // try to find the first line that looks like a shell command.
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.first().map(|l| looks_like_prose(l)).unwrap_or(false) {
        for line in &lines {
            if looks_like_command(line) {
                return line.trim().to_string();
            }
        }
    }

    trimmed.to_string()
}

pub(crate) fn looks_like_prose(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    // Starts with CJK, or ends with sentence punctuation, or contains typical refusal words
    if t.chars()
        .next()
        .map(|c| (c as u32) > 0x3000)
        .unwrap_or(false)
    {
        return true;
    }
    if t.ends_with('.') || t.ends_with('。') || t.ends_with('요') || t.ends_with('다') {
        return true;
    }
    false
}

pub(crate) fn looks_like_command(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return false;
    }
    // Reject lines that contain CJK characters — almost always prose.
    if t.chars()
        .any(|c| (c as u32) >= 0x2E80 && (c as u32) <= 0x9FFF)
    {
        return false;
    }
    // Reject lines that end with sentence punctuation — also almost always prose.
    if let Some(last) = t.chars().last() {
        if matches!(last, '.' | '!' | '?' | ':') && !t.ends_with("..") && !t.ends_with("/.") {
            return false;
        }
    }
    // Accept subshells, env assignments, pipelines, and anything else that starts with
    // a reasonable shell token. Be permissive here — we already trust the model's
    // fence/sanitize pipeline, this is just a prose sanity gate.
    let first_char = t.chars().next().unwrap_or(' ');
    if matches!(first_char, '(' | '{' | '!' | '$' | '/' | '.' | '~') {
        return true;
    }
    let first = t.split_whitespace().next().unwrap_or("");
    if first.is_empty() {
        return false;
    }
    // Env-assignment prefix like `FOO=bar` or `NODE_ENV=prod`.
    if let Some(eq) = first.find('=') {
        let name = &first[..eq];
        return !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    }
    first.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.' || c == '+'
    })
}

pub(crate) fn split_command_and_explanation(output: &str) -> (String, Option<String>) {
    let mut cmd_lines = Vec::new();
    let mut expl_lines = Vec::new();
    let mut in_expl = false;
    for line in output.lines() {
        if in_expl || line.trim_start().starts_with('#') {
            in_expl = true;
            let l = line.trim_start().trim_start_matches('#').trim();
            if !l.is_empty() {
                expl_lines.push(l.to_string());
            }
        } else {
            cmd_lines.push(line);
        }
    }
    let cmd = cmd_lines.join("\n").trim().to_string();
    let expl = if expl_lines.is_empty() {
        None
    } else {
        Some(expl_lines.join(" "))
    };
    (cmd, expl)
}

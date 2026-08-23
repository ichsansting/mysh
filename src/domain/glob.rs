use std::path::Path;

/// Whether a Directory-mode ignore pattern excludes `rel` (path relative to the
/// tracked directory). A pattern without `/` matches the file name (any depth);
/// one with `/` matches the whole relative path. `*` spans within a segment is
/// all these lists need — dotfile noise is `*.log`, `cache/*`, `*.swp`.
pub fn is_ignored(rel: &Path, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        if pattern.contains('/') {
            matches(pattern, &rel.to_string_lossy())
        } else {
            rel.file_name()
                .is_some_and(|name| matches(pattern, &name.to_string_lossy()))
        }
    })
}

/// Classic glob match: `*` = any run (including empty), `?` = any one char.
fn matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    matches_at(&p, &t)
}

fn matches_at(p: &[char], t: &[char]) -> bool {
    match p.first() {
        None => t.is_empty(),
        Some('*') => (0..=t.len()).any(|skip| matches_at(&p[1..], &t[skip..])),
        Some('?') => !t.is_empty() && matches_at(&p[1..], &t[1..]),
        Some(c) => t.first() == Some(c) && matches_at(&p[1..], &t[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_spans_and_question_takes_one() {
        assert!(matches("*.log", "debug.log"));
        assert!(matches("?.log", "a.log"));
        assert!(!matches("?.log", "ab.log"));
        assert!(!matches("*.log", "debug.txt"));
        assert!(matches("*", "anything"));
    }

    #[test]
    fn bare_patterns_match_the_file_name_at_any_depth() {
        let patterns = vec!["*.log".to_string()];
        assert!(is_ignored(Path::new("nested/deep/debug.log"), &patterns));
        assert!(!is_ignored(Path::new("nested/deep/config.toml"), &patterns));
    }

    #[test]
    fn slash_patterns_match_the_relative_path() {
        let patterns = vec!["cache/*".to_string()];
        assert!(is_ignored(Path::new("cache/blob"), &patterns));
        assert!(!is_ignored(Path::new("other/blob"), &patterns));
    }
}

use crate::apply::walk_source_files;
use serde_json::{Map, Value};
use std::io;
use std::path::{Path, PathBuf};

/// The Source-side suffix that marks a file as an Overlay: it declares specific keys
/// to enforce onto an existing (or not-yet-existing) Target file, rather than
/// replacing that file's content wholesale the way a plain tracked file would.
///
/// Unlike `Fragment` (composes a whole Target from pieces it fully owns), an Overlay's
/// Target file has content mysh never owns — only the declared keys are ever read or
/// written; everything else in the file is invisible to mysh, always.
pub const SUFFIX: &str = "overlay";

/// Whether `path`'s file name marks it as an Overlay (`<name>.<ext>.overlay`).
pub fn is_overlay_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == SUFFIX)
}

/// An Overlay file's Target path: `relative` with its `.overlay` suffix stripped.
pub fn target_name(relative: &Path) -> PathBuf {
    relative.with_extension("")
}

/// Overlay files anywhere under `source` (skipping `.git`/`.mysh`), returned as paths
/// relative to `source`, sorted.
pub fn find_overlay_files(source: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = walk_source_files(source)?
        .iter()
        .map(|p| p.strip_prefix(source).expect("entry is under source").to_path_buf())
        .filter(|p| is_overlay_file(p))
        .collect();
    out.sort();
    Ok(out)
}

/// Parses an Overlay file's own content as the keys/values it declares. Dispatches on
/// `target_path`'s extension — the type left once `.overlay` is stripped — to pick a
/// format; only `.json` is implemented today, so any other extension is rejected
/// outright rather than guessed at. `overlay_path` is used only to name the offending
/// file in error messages.
pub fn parse_declared(overlay_path: &Path, target_path: &Path, content: &[u8]) -> Result<Map<String, Value>, String> {
    match target_path.extension().and_then(|e| e.to_str()) {
        Some("json") => parse_json_object(overlay_path, content),
        other => Err(format!(
            "{}: unsupported overlay type{} (only .json is supported)",
            overlay_path.display(),
            other.map(|ext| format!(" (.{ext})")).unwrap_or_default(),
        )),
    }
}

/// Shallow-merges `declared` onto `existing` (Target's current raw content — pass an
/// empty slice when the file doesn't exist yet, treated the same as `{}`): each
/// declared key replaces that key's whole value in the result, even if it's an object;
/// every other key already in `existing` keeps its value and its position (`Map` is
/// order-preserving, via the `preserve_order` feature — without it every write would
/// alphabetically re-sort every key mysh doesn't own, not just the declared ones).
/// The file is still re-serialized (pretty-printed) whenever a declared key's value
/// actually changes, so exact whitespace/indentation isn't preserved — only content
/// and order are. Fails loudly, rather than discarding it, if `existing` is non-empty
/// but not a valid JSON object.
pub fn merge(target_path: &Path, existing: &[u8], declared: &Map<String, Value>) -> Result<Vec<u8>, String> {
    let mut object = parse_json_object(target_path, existing)?;
    for (key, value) in declared {
        object.insert(key.clone(), value.clone());
    }
    let mut out = serde_json::to_vec_pretty(&Value::Object(object)).map_err(|e| e.to_string())?;
    out.push(b'\n');
    Ok(out)
}

/// Whether every key in `declared` already has that exact value in `existing` — i.e.
/// no enforcement work remains. Same fail-loudly behavior as `merge` for unparseable
/// `existing` content.
pub fn keys_match(target_path: &Path, existing: &[u8], declared: &Map<String, Value>) -> Result<bool, String> {
    let object = parse_json_object(target_path, existing)?;
    Ok(declared.iter().all(|(key, value)| object.get(key) == Some(value)))
}

/// `content` as a JSON object: empty content is treated as `{}` (the "file doesn't
/// exist yet" case, for both an Overlay's own declared content and Target's current
/// content); non-empty content that fails to parse, or parses to something other than
/// a JSON object, is an error rather than silently discarded. `label` names the file in
/// error messages.
fn parse_json_object(label: &Path, content: &[u8]) -> Result<Map<String, Value>, String> {
    if content.is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_slice(content).map_err(|e| format!("{}: invalid JSON: {e}", label.display()))? {
        Value::Object(map) => Ok(map),
        _ => Err(format!("{}: content must be a JSON object", label.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn is_overlay_file_matches_only_the_overlay_suffix() {
        assert!(is_overlay_file(Path::new(".claude.json.overlay")));
        assert!(!is_overlay_file(Path::new(".claude.json")));
        assert!(!is_overlay_file(Path::new("bashrc")));
    }

    #[test]
    fn target_name_strips_only_the_overlay_extension() {
        assert_eq!(target_name(Path::new(".claude.json.overlay")), Path::new(".claude.json"));
    }

    #[test]
    fn parse_declared_reads_a_json_object() {
        let declared = parse_declared(
            Path::new(".claude.json.overlay"),
            Path::new(".claude.json"),
            br#"{"hasCompletedOnboarding": true}"#,
        )
        .unwrap();
        assert_eq!(declared.get("hasCompletedOnboarding"), Some(&json!(true)));
    }

    #[test]
    fn parse_declared_rejects_a_non_object_json_value() {
        let err =
            parse_declared(Path::new("x.json.overlay"), Path::new("x.json"), b"[1, 2, 3]").unwrap_err();
        assert!(err.contains("must be a JSON object"), "{err}");
    }

    #[test]
    fn parse_declared_rejects_invalid_json() {
        let err = parse_declared(Path::new("x.json.overlay"), Path::new("x.json"), b"{not json").unwrap_err();
        assert!(err.contains("invalid JSON"), "{err}");
    }

    #[test]
    fn parse_declared_rejects_unsupported_extensions() {
        let err =
            parse_declared(Path::new("x.yaml.overlay"), Path::new("x.yaml"), b"a: 1").unwrap_err();
        assert!(err.contains("unsupported overlay type"), "{err}");
        assert!(err.contains(".yaml"), "{err}");
    }

    #[test]
    fn merge_creates_an_object_from_empty_existing_content() {
        let mut declared = Map::new();
        declared.insert("hasCompletedOnboarding".to_string(), json!(true));

        let merged = merge(Path::new(".claude.json"), b"", &declared).unwrap();
        let value: Value = serde_json::from_slice(&merged).unwrap();
        assert_eq!(value, json!({"hasCompletedOnboarding": true}));
    }

    #[test]
    fn merge_preserves_untouched_keys_and_overwrites_declared_ones() {
        let mut declared = Map::new();
        declared.insert("hasCompletedOnboarding".to_string(), json!(true));

        let existing = br#"{"hasCompletedOnboarding": false, "projects": {"a": 1}}"#;
        let merged = merge(Path::new(".claude.json"), existing, &declared).unwrap();
        let value: Value = serde_json::from_slice(&merged).unwrap();
        assert_eq!(value, json!({"hasCompletedOnboarding": true, "projects": {"a": 1}}));
    }

    #[test]
    fn merge_overwrites_an_object_value_wholesale_not_deeply() {
        let mut declared = Map::new();
        declared.insert("nested".to_string(), json!({"x": 1}));

        let existing = br#"{"nested": {"x": 0, "y": 2}}"#;
        let merged = merge(Path::new(".claude.json"), existing, &declared).unwrap();
        let value: Value = serde_json::from_slice(&merged).unwrap();
        assert_eq!(value, json!({"nested": {"x": 1}}));
    }

    #[test]
    fn merge_preserves_the_original_key_order_of_untouched_keys() {
        let mut declared = Map::new();
        declared.insert("hasCompletedOnboarding".to_string(), json!(true));

        // Deliberately non-alphabetical order: if merge re-sorted keys, "zebra" would
        // move before "hasCompletedOnboarding" in the output.
        let existing = br#"{"zebra": 1, "hasCompletedOnboarding": false, "apple": 2}"#;
        let merged = merge(Path::new(".claude.json"), existing, &declared).unwrap();
        let merged_str = String::from_utf8(merged).unwrap();

        let zebra_pos = merged_str.find("zebra").unwrap();
        let onboarding_pos = merged_str.find("hasCompletedOnboarding").unwrap();
        let apple_pos = merged_str.find("apple").unwrap();
        assert!(zebra_pos < onboarding_pos && onboarding_pos < apple_pos, "{merged_str}");
    }

    #[test]
    fn merge_fails_loudly_on_unparseable_existing_content() {
        let declared = Map::new();
        let err = merge(Path::new(".claude.json"), b"not json at all", &declared).unwrap_err();
        assert!(err.contains("invalid JSON"), "{err}");
    }

    #[test]
    fn keys_match_true_when_missing_file_and_no_declared_keys() {
        assert!(keys_match(Path::new(".claude.json"), b"", &Map::new()).unwrap());
    }

    #[test]
    fn keys_match_false_when_target_is_missing_and_keys_are_declared() {
        let mut declared = Map::new();
        declared.insert("hasCompletedOnboarding".to_string(), json!(true));
        assert!(!keys_match(Path::new(".claude.json"), b"", &declared).unwrap());
    }

    #[test]
    fn keys_match_false_when_a_declared_value_differs() {
        let mut declared = Map::new();
        declared.insert("hasCompletedOnboarding".to_string(), json!(true));
        let existing = br#"{"hasCompletedOnboarding": false}"#;
        assert!(!keys_match(Path::new(".claude.json"), existing, &declared).unwrap());
    }

    #[test]
    fn keys_match_true_when_declared_values_already_match_alongside_other_keys() {
        let mut declared = Map::new();
        declared.insert("hasCompletedOnboarding".to_string(), json!(true));
        let existing = br#"{"hasCompletedOnboarding": true, "projects": {}}"#;
        assert!(keys_match(Path::new(".claude.json"), existing, &declared).unwrap());
    }
}

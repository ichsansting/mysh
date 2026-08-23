use crate::error::{Error, IoCtx, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// The declared keys of a `.overlay` Source file — the only part of its Target
/// mysh enforces; every other key belongs to other programs and is never read
/// into Source. The merge type comes from the filename left after stripping
/// `.overlay` (`.json` today; anything else is rejected, not guessed at).
pub fn read_declared(path: &Path) -> Result<Map<String, Value>> {
    let target_name = path.with_extension("");
    if target_name.extension().is_none_or(|e| e != "json") {
        return Err(Error::Overlay {
            path: path.to_path_buf(),
            detail: "only .json overlay targets are supported".to_string(),
        });
    }
    let bytes = fs::read(path).at("read", path)?;
    as_object(&bytes).map_err(|detail| Error::Overlay { path: path.to_path_buf(), detail })
}

/// Whether every declared key already has its declared value in the live
/// Target content (`None` = file doesn't exist). Matching means apply has
/// nothing to do — and records nothing.
pub fn keys_match(live: Option<&[u8]>, declared: &Map<String, Value>) -> bool {
    let Some(bytes) = live else { return false };
    let Ok(obj) = as_object(bytes) else { return false };
    declared.iter().all(|(key, value)| obj.get(key) == Some(value))
}

/// Shallow-merges the declared keys onto the live content (starting from `{}`
/// when the Target doesn't exist yet), leaving every other key untouched.
/// Key order is preserved (serde_json's preserve_order is load-bearing here).
pub fn merge(live: Option<&[u8]>, declared: &Map<String, Value>, path: &Path) -> Result<Vec<u8>> {
    let mut obj = match live {
        None => Map::new(),
        Some(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Map::new(),
        Some(bytes) => as_object(bytes)
            .map_err(|detail| Error::Overlay { path: path.to_path_buf(), detail })?,
    };
    for (key, value) in declared {
        obj.insert(key.clone(), value.clone());
    }
    let mut out = serde_json::to_vec_pretty(&Value::Object(obj)).expect("object serializes");
    out.push(b'\n');
    Ok(out)
}

fn as_object(bytes: &[u8]) -> std::result::Result<Map<String, Value>, String> {
    match serde_json::from_slice(bytes) {
        Ok(Value::Object(obj)) => Ok(obj),
        Ok(_) => Err("not a JSON object".to_string()),
        Err(e) => Err(format!("invalid JSON: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared(json: &str) -> Map<String, Value> {
        match serde_json::from_str(json).unwrap() {
            Value::Object(obj) => obj,
            _ => panic!("test fixture must be an object"),
        }
    }

    #[test]
    fn merge_preserves_undeclared_keys_and_their_order() {
        let live = br#"{"zeta": 1, "alpha": 2, "flag": false}"#;
        let merged = merge(Some(live), &declared(r#"{"flag": true}"#), Path::new("x")).unwrap();
        let text = String::from_utf8(merged).unwrap();
        let zeta = text.find("zeta").unwrap();
        let alpha = text.find("alpha").unwrap();
        assert!(zeta < alpha, "key order must be preserved: {text}");
        assert!(text.contains("\"flag\": true"));
        assert!(text.contains("\"zeta\": 1"));
    }

    #[test]
    fn merge_starts_from_empty_object_when_target_is_absent_or_blank() {
        let d = declared(r#"{"a": 1}"#);
        for live in [None, Some(b"  \n".as_slice())] {
            let merged = merge(live, &d, Path::new("x")).unwrap();
            let value: Value = serde_json::from_slice(&merged).unwrap();
            assert_eq!(value["a"], 1);
        }
    }

    #[test]
    fn merge_rejects_a_non_object_target() {
        assert!(merge(Some(b"[1,2]"), &declared("{}"), Path::new("x")).is_err());
        assert!(merge(Some(b"not json"), &declared("{}"), Path::new("x")).is_err());
    }

    #[test]
    fn keys_match_ignores_extra_keys_but_not_differing_values() {
        let d = declared(r#"{"flag": true}"#);
        assert!(keys_match(Some(br#"{"other": 9, "flag": true}"#), &d));
        assert!(!keys_match(Some(br#"{"flag": false}"#), &d));
        assert!(!keys_match(Some(b"garbage"), &d));
        assert!(!keys_match(None, &d));
    }
}

use crate::confirm::confirm;
use crate::diff;
use crate::git;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

/// Captures `Target` drift into `Source`, commits, and pushes to `Remote` — local
/// wins. Always shows the pending diff and requires explicit confirmation on `input`
/// before mutating anything; declining leaves `Source`, `Target`, and `Remote`
/// unchanged.
pub fn save(source: &Path, target: &Path, input: &mut dyn BufRead) -> Result<String, String> {
    let drifted: Vec<_> = diff::diff(source, target)?
        .into_iter()
        .filter(|d| d.target_drift)
        .collect();
    if drifted.is_empty() {
        return Ok("nothing to save\n".to_string());
    }

    print!("{}", diff::format_drifts(&drifted));
    if !confirm(input, "save these changes to Source and push? [y/N] ")? {
        return Ok("aborted\n".to_string());
    }

    for d in &drifted {
        let dest = source.join(&d.path);
        match fs::read(target.join(&d.path)) {
            Ok(content) => {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&dest, content).map_err(|e| e.to_string())?;
            }
            // Deleted on Target: drift is captured as a deletion in Source too.
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                fs::remove_file(&dest).ok();
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    git::commit(source, "mysh save")?;
    git::push(source)?;
    Ok("saved\n".to_string())
}

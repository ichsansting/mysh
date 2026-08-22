use crate::confirm::confirm;
use crate::diff;
use crate::git;
use crate::secret::{self, PassphraseFn};
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

/// Captures `Target` drift into `Source`, commits, and pushes to `Remote` — local
/// wins. Always shows the pending diff and requires explicit confirmation on `input`
/// before mutating anything; declining leaves `Source`, `Target`, and `Remote`
/// unchanged. An edited `Secret` is re-encrypted (fresh salt/nonce) back into its
/// `Source` `.age` file, never written there as plaintext. Refuses (with no prompt) if
/// any drifted path is a `Fragment`-composed target — there's no unambiguous fragment
/// to attribute a merged-file edit back to — or an `Overlay` target, which is
/// Source-authoritative by design: a drifted declared key is fixed by `apply`/`reset`
/// re-enforcing Source's value, never captured back into Source.
pub fn save(
    source: &Path,
    target: &Path,
    input: &mut dyn BufRead,
    get_passphrase: &mut PassphraseFn,
) -> Result<String, String> {
    let drifted: Vec<_> = diff::diff(source, target, get_passphrase)?
        .into_iter()
        .filter(|d| d.target_drift)
        .collect();
    if drifted.is_empty() {
        return Ok("nothing to save\n".to_string());
    }
    if let Some(fragment) = drifted.iter().find(|d| d.is_fragment) {
        return Err(format!(
            "'{}' is composed from fragments in {}; edit the fragment directly instead of saving",
            fragment.path.display(),
            fragment.source_path.display(),
        ));
    }
    if let Some(overlay) = drifted.iter().find(|d| d.is_overlay) {
        return Err(format!(
            "'{}' is enforced by the overlay {}; Source's declared value wins, so save it there instead",
            overlay.path.display(),
            overlay.source_path.display(),
        ));
    }

    print!("{}", diff::format_drifts(&drifted));
    if !confirm(input, "save these changes to Source and push? [y/N] ")? {
        return Ok("aborted\n".to_string());
    }

    for d in &drifted {
        let dest = source.join(&d.source_path);
        match fs::read(target.join(&d.path)) {
            Ok(content) => {
                let to_write = if d.is_secret {
                    secret::encrypt(&content, &get_passphrase()?)?
                } else {
                    content
                };
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&dest, to_write).map_err(|e| e.to_string())?;
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

use crate::apply;
use crate::confirm::confirm;
use crate::diff;
use crate::git;
use crate::secret::PassphraseFn;
use std::io::BufRead;
use std::path::Path;

/// Discards local drift in both `Source` and `Target`, force-fetches `Remote`, and
/// re-applies — remote wins. Always shows the pending diff and requires explicit
/// confirmation on `input` before mutating anything; declining leaves `Source`,
/// `Target`, and `Remote` unchanged. A `Secret` is re-decrypted fresh from `Source`
/// into `Target`, discarding any local plaintext drift.
pub fn reset(
    source: &Path,
    target: &Path,
    input: &mut dyn BufRead,
    get_passphrase: &mut PassphraseFn,
) -> Result<String, String> {
    let drifts = diff::diff(source, target, get_passphrase)?;
    if drifts.is_empty() {
        return Ok("nothing to reset\n".to_string());
    }

    print!("{}", diff::format_drifts(&drifts));
    if !confirm(input, "reset Source and Target to Remote, discarding local drift? [y/N] ")? {
        return Ok("aborted\n".to_string());
    }

    let upstream = git::upstream_ref(source)?;
    git::reset_hard(source, &upstream)?;
    apply::apply(source, target, get_passphrase)?;
    Ok("reset\n".to_string())
}

use crate::error::{IoCtx, Result};
use crate::infra::crypto;
use crate::infra::prompt::PassphraseFn;
use crate::domain::render;
use std::fs;
use std::path::Path;

/// Renders a `.frag` directory into its single composed Target content:
/// members concatenate in lexical filename order, `.age` members decrypted
/// first — the only registration a new fragment needs is existing in the dir.
pub fn compose(frag_dir: &Path, passphrase: &mut PassphraseFn) -> Result<Vec<u8>> {
    let mut names: Vec<_> = fs::read_dir(frag_dir)
        .at("read directory", frag_dir)?
        .map(|entry| entry.at("read directory", frag_dir).map(|e| e.path()))
        .collect::<Result<Vec<_>>>()?;
    names.sort();
    let mut composed = Vec::new();
    for path in names {
        let content = fs::read(&path).at("read", &path)?;
        if render::is_secret(&path) {
            composed.extend(crypto::decrypt(&content, &passphrase()?, &path)?);
        } else {
            composed.extend(content);
        }
    }
    Ok(composed)
}

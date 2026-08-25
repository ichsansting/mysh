use crate::domain::FINGERPRINTS_REL;
use crate::error::{IoCtx, Result};
use crate::infra::fsx;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// The last content hash a Target-producing unit was known to render to,
/// recorded at Apply and Save — the two moments Source and Target are known
/// to agree. Lets `diff --quick` detect Target drift for Secret/Fragment
/// units without decrypting or composing them: it just compares the live
/// Target's hash against the one recorded here. Disposable — unlike the
/// Application Log, losing this file only costs a stale/unknown prompt
/// reading until the next Apply or Save, never correctness of a real command.
pub struct Fingerprints {
    path: PathBuf,
    entries: BTreeMap<PathBuf, u64>,
}

impl Fingerprints {
    pub fn at(target_dir: &Path) -> Result<Fingerprints> {
        let path = target_dir.join(FINGERPRINTS_REL);
        let entries = match fs::read_to_string(&path) {
            Ok(text) => parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e).at("read", &path),
        };
        Ok(Fingerprints { path, entries })
    }

    /// `None` means no Fingerprint has ever been recorded for this unit on
    /// this device — e.g. before its first Apply. Callers must treat that as
    /// unknown, not clean: there is nothing to compare against yet.
    pub fn get(&self, rel: &Path) -> Option<u64> {
        self.entries.get(rel).copied()
    }

    pub fn set(&mut self, rel: PathBuf, content: &[u8]) {
        self.entries.insert(rel, hash_of(content));
    }

    /// Writes only when content changed — an unrecorded/idempotent Apply or
    /// Save must never bump this file's mtime (mirrors `fsx::write_if_changed`'s
    /// contract, which every other rendered-content write already relies on).
    /// Nothing recorded at all (e.g. an Overlay-only Source) never creates the
    /// file in the first place — an all-Overlay Apply must stay a true no-op.
    pub fn save(&self) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let mut text = String::new();
        for (rel, hash) in &self.entries {
            text.push_str(&format!("{}\t{hash:x}\n", rel.display()));
        }
        fsx::write_if_changed(&self.path, text.as_bytes(), None)
    }
}

fn parse(text: &str) -> BTreeMap<PathBuf, u64> {
    let mut entries = BTreeMap::new();
    for line in text.lines() {
        if let Some((rel, hash)) = line.split_once('\t')
            && let Ok(hash) = u64::from_str_radix(hash, 16)
        {
            entries.insert(PathBuf::from(rel), hash);
        }
    }
    entries
}

/// `DefaultHasher::new()` (not `HashMap`'s randomized `RandomState`) is fixed-key
/// and deterministic across process runs — required here since the hash is
/// written by one `mysh` invocation and read back by a later, separate one.
pub fn hash_of(content: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrecorded_path_is_unknown() {
        let dir = tempdir();
        let fp = Fingerprints::at(&dir).unwrap();
        assert_eq!(fp.get(Path::new(".netrc")), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn set_then_save_then_reload_round_trips() {
        let dir = tempdir();
        let mut fp = Fingerprints::at(&dir).unwrap();
        fp.set(PathBuf::from(".netrc"), b"machine x login y");
        fp.save().unwrap();

        let reloaded = Fingerprints::at(&dir).unwrap();
        assert_eq!(
            reloaded.get(Path::new(".netrc")),
            Some(hash_of(b"machine x login y"))
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn different_content_hashes_differently() {
        assert_ne!(hash_of(b"a"), hash_of(b"b"));
    }

    fn tempdir() -> PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("mysh-fingerprint-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}

use std::fs;
use std::io;
use std::path::Path;

/// Renders every plain file in `source` to its mirrored relative path under `target`
/// via identity copy. Skips `.git` (Source is a git working tree). Idempotent: a file
/// is only (re)written when its content differs from what's already at the target path.
pub fn apply(source: &Path, target: &Path) -> Result<(), String> {
    render(source, target).map_err(|e| e.to_string())
}

fn render(source: &Path, target: &Path) -> io::Result<()> {
    for entry in walk_files(source)? {
        let relative = entry.strip_prefix(source).expect("entry is under source");
        let dest = target.join(relative);
        copy_if_changed(&entry, &dest)?;
    }
    Ok(())
}

fn walk_files(dir: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                continue;
            }
            files.extend(walk_files(&path)?);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

fn copy_if_changed(src: &Path, dest: &Path) -> io::Result<()> {
    let content = fs::read(src)?;
    if fs::read(dest).map(|existing| existing == content).unwrap_or(false) {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, content)
}

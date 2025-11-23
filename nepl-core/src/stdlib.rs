use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibFile {
    pub path: PathBuf,
    pub contents: String,
}

pub fn default_stdlib_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../stdlib")
}

pub fn load_stdlib_files(root: impl AsRef<Path>) -> Result<Vec<StdlibFile>, std::io::Error> {
    let root = root.as_ref();
    let mut files = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "nepl") {
            let contents = fs::read_to_string(path)?;
            let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
            files.push(StdlibFile {
                path: relative,
                contents,
            });
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_files_from_default_root() {
        let files = load_stdlib_files(default_stdlib_root()).expect("stdlib should load");
        assert!(!files.is_empty());
    }
}

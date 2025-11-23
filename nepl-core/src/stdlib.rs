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
    if !root.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("stdlib root {root:?} not found"),
        ));
    }
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

    #[test]
    fn stdlib_includes_expected_modules() {
        let files = load_stdlib_files(default_stdlib_root()).expect("stdlib should load");
        let mut names: Vec<_> = files
            .iter()
            .map(|file| file.path.to_string_lossy().to_string())
            .collect();
        names.sort();

        assert!(names.contains(&"std.nepl".to_string()));
        assert!(names.contains(&"math.nepl".to_string()));
        assert!(names.contains(&"logic.nepl".to_string()));
        assert!(names.contains(&"bit.nepl".to_string()));
        assert!(names.contains(&"string.nepl".to_string()));
        assert!(names.contains(&"vec.nepl".to_string()));
        assert!(names.contains(&"platform/wasm_core.nepl".to_string()));
        assert!(names.contains(&"platform/wasi.nepl".to_string()));

        let math_file = files
            .iter()
            .find(|file| file.path == std::path::PathBuf::from("math.nepl"))
            .expect("math module missing");
        assert!(math_file.contents.contains("permutation"));
        assert!(math_file.contents.contains("combination"));

        let string_file = files
            .iter()
            .find(|file| file.path == std::path::PathBuf::from("string.nepl"))
            .expect("string module missing");
        assert!(string_file.contents.contains("concat"));
        assert!(string_file.contents.contains("len"));

        let vec_file = files
            .iter()
            .find(|file| file.path == std::path::PathBuf::from("vec.nepl"))
            .expect("vec module missing");
        assert!(vec_file.contents.contains("push"));
        assert!(vec_file.contents.contains("pop"));
    }
}

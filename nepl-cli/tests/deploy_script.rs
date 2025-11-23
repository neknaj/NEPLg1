use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{ensure, Result};
use tempfile::tempdir;

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts/fetch_editorsample.sh")
}

fn run_git(repo: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()?;
    ensure!(status.success(), "git command {:?} failed", args);
    Ok(())
}

fn initialize_remote_repo(root: &Path, initial_contents: &str) -> Result<PathBuf> {
    let repo_path = root.join("editorsample-remote");
    fs::create_dir_all(&repo_path)?;
    run_git(&repo_path, &["init", "--quiet"])?;
    run_git(&repo_path, &["config", "user.email", "test@example.com"])?;
    run_git(&repo_path, &["config", "user.name", "Test User"])?;

    fs::write(repo_path.join("index.html"), initial_contents)?;
    run_git(&repo_path, &["add", "index.html"])?;
    run_git(&repo_path, &["commit", "-m", "initial content"])?;

    Ok(repo_path)
}

fn commit_change(repo: &Path, new_contents: &str, message: &str) -> Result<()> {
    fs::write(repo.join("index.html"), new_contents)?;
    run_git(repo, &["add", "index.html"])?;
    run_git(repo, &["commit", "-m", message])
}

fn run_fetch(repo: &Path, dest_root: &Path, reference: Option<&str>) -> Result<()> {
    let mut command = Command::new("bash");
    command.arg(script_path());
    command.env("EDITOR_SAMPLE_REPO", repo);
    command.env("EDITOR_SAMPLE_DEST", dest_root);
    if let Some(reference) = reference {
        command.env("EDITOR_SAMPLE_REF", reference);
    }
    let status = command.status()?;
    ensure!(status.success(), "fetch script failed");
    Ok(())
}

#[test]
fn clones_editor_sample_into_custom_destination() -> Result<()> {
    let temp = tempdir()?;
    let remote = initialize_remote_repo(temp.path(), "first version")?;
    let destination = temp.path().join("workspace");

    run_fetch(&remote, &destination, None)?;

    let cloned = destination.join("web/vendor/editorsample/index.html");
    let contents = fs::read_to_string(cloned)?;
    assert_eq!(contents, "first version");
    Ok(())
}

#[test]
fn updates_existing_clone_after_remote_change() -> Result<()> {
    let temp = tempdir()?;
    let remote = initialize_remote_repo(temp.path(), "first version")?;
    let destination = temp.path().join("workspace");

    run_fetch(&remote, &destination, None)?;
    let cloned = destination.join("web/vendor/editorsample/index.html");
    let first = fs::read_to_string(&cloned)?;
    assert_eq!(first, "first version");

    commit_change(&remote, "second version", "update content")?;
    run_fetch(&remote, &destination, None)?;

    let updated = fs::read_to_string(&cloned)?;
    assert_eq!(updated, "second version");
    Ok(())
}

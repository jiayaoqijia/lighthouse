use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CLIENT_NAME: &str = "Lighthouse";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = Path::new(&manifest_dir);

    // The crate version is inherited from the workspace.
    let semantic_version = env::var("CARGO_PKG_VERSION").unwrap();

    // Worktrees keep HEAD in a private git directory and branch refs in the common directory.
    if let Some(head_path) = git_path(manifest_path, "HEAD") {
        println!("cargo:rerun-if-changed={}", head_path.display());
        if let Ok(head_content) = fs::read_to_string(&head_path)
            && let Some(reference) = head_content.trim().strip_prefix("ref: ")
            && let Some(reference_path) = git_path(manifest_path, reference)
        {
            if reference_path.exists() {
                println!("cargo:rerun-if-changed={}", reference_path.display());
            } else if let Some(parent) = reference_path.ancestors().skip(1).find(|p| p.is_dir()) {
                // A commit to a packed branch creates a new loose ref in this directory.
                println!("cargo:rerun-if-changed={}", parent.display());
            }
        }
    }
    if let Some(packed_refs) = git_path(manifest_path, "packed-refs").filter(|p| p.exists()) {
        println!("cargo:rerun-if-changed={}", packed_refs.display());
    }

    // Construct Lighthouse version string without commit hash.
    let base_version = format!("{}/v{}", CLIENT_NAME, semantic_version);

    let commit_hash = get_git_hash(7);
    let commit_prefix = get_git_hash(8);

    // If commit hash is valid, construct the full version string.
    let version = if !commit_hash.is_empty() && commit_hash.len() >= 7 {
        format!("{}-{}", base_version, commit_hash)
    } else {
        base_version
    };

    println!("cargo:rustc-env=GIT_VERSION={}", version);
    println!("cargo:rustc-env=GIT_COMMIT_PREFIX={}", commit_prefix);
    println!("cargo:rustc-env=CLIENT_NAME={}", CLIENT_NAME);
    println!("cargo:rustc-env=SEMANTIC_VERSION={}", semantic_version);
}

fn git_path(manifest_dir: &Path, name: &str) -> Option<PathBuf> {
    let output = Command::new("git")
        .current_dir(manifest_dir)
        .args(["rev-parse", "--git-path", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    Some(if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    })
}

fn get_git_hash(len: usize) -> String {
    Command::new("git")
        .args(["rev-parse", &format!("--short={}", len), "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| {
            // Fallback commit prefix for execution engine reporting.
            if len == 8 {
                "00000000".to_string()
            } else {
                String::new()
            }
        })
}

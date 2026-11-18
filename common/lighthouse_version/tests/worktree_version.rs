use std::fs;
use std::path::Path;
use std::process::Command;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .args([
            "-c",
            "user.name=Version Test",
            "-c",
            "user.email=version@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn version(worktree: &Path, target: &Path) -> String {
    let output = Command::new("cargo")
        .current_dir(worktree.join("common/lighthouse_version"))
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .args(["run", "--quiet", "--offline"])
        .env("CARGO_TARGET_DIR", target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

#[test]
fn cached_build_version_tracks_a_worktree_branch_commit() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo with spaces");
    let worktree = temp.path().join("worktree with spaces");
    let target = temp.path().join("target");
    fs::create_dir_all(repo.join("common/lighthouse_version/src")).unwrap();
    fs::write(
        repo.join("common/lighthouse_version/build.rs"),
        include_str!("../build.rs"),
    )
    .unwrap();
    fs::write(
        repo.join("common/lighthouse_version/Cargo.toml"),
        "[package]\nname=\"version-probe\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .unwrap();
    fs::write(
        repo.join("common/lighthouse_version/src/main.rs"),
        "fn main() { println!(\"{}\", env!(\"GIT_VERSION\")); }\n",
    )
    .unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["add", "common/lighthouse_version"]);
    git(&repo, &["commit", "-m", "first"]);
    let primary_target = temp.path().join("primary-target");
    let primary_first = git(&repo, &["rev-parse", "--short=7", "HEAD"]);
    assert!(version(&repo, &primary_target).ends_with(&primary_first));
    git(&repo, &["commit", "--allow-empty", "-m", "primary branch"]);
    let primary_second = git(&repo, &["rev-parse", "--short=7", "HEAD"]);
    assert!(version(&repo, &primary_target).ends_with(&primary_second));
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "probe",
            worktree.to_str().unwrap(),
            "HEAD",
        ],
    );
    assert!(worktree.join(".git").is_file());
    let first = git(&worktree, &["rev-parse", "--short=7", "HEAD"]);
    assert!(version(&worktree, &target).ends_with(&first));
    git(&worktree, &["commit", "--allow-empty", "-m", "second"]);
    let second = git(&worktree, &["rev-parse", "--short=7", "HEAD"]);
    assert_ne!(first, second);
    assert!(
        version(&worktree, &target).ends_with(&second),
        "cached Cargo build retained the old worktree commit"
    );

    git(&worktree, &["pack-refs", "--all", "--prune"]);
    assert!(version(&worktree, &target).ends_with(&second));
    git(
        &worktree,
        &["commit", "--allow-empty", "-m", "packed branch"],
    );
    let third = git(&worktree, &["rev-parse", "--short=7", "HEAD"]);
    assert!(version(&worktree, &target).ends_with(&third));
    git(&worktree, &["checkout", "--detach"]);
    git(
        &worktree,
        &["commit", "--allow-empty", "-m", "detached HEAD"],
    );
    let fourth = git(&worktree, &["rev-parse", "--short=7", "HEAD"]);
    assert!(version(&worktree, &target).ends_with(&fourth));
}

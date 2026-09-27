//! Checks run before deleting anything inside a project (SPEC section 7).
//! Git is only read, never written: Sweepr never commits, stashes or resets anything.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::Serialize;

use crate::process;

const GIT_TIMEOUT: Duration = Duration::from_secs(20);

/// Why an artifact folder must not be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// Git tracks files inside the folder (real case: two `build` folders with 171 and 183 tracked files).
    TrackedFiles { count: usize },
    /// The folder holds its own git repository.
    ContainsRepository,
    /// A `.env` file sits at the top of the folder.
    EnvFile { path: PathBuf },
    /// Git could not answer: in doubt, the folder is kept.
    GitError { message: String },
}

/// State of a repository, needed before deleting a whole project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepoStatus {
    pub root: PathBuf,
    /// Lines of `git status --short`.
    pub changes: Vec<String>,
    /// Commits on local branches that no remote has.
    pub unpushed: usize,
    pub has_remote: bool,
    /// Unix time of the last commit.
    pub last_commit: Option<i64>,
}

/// Whether git can be used. On macOS, `/usr/bin/git` without the command line tools
/// opens an installation dialog, so they are checked first. Checked once per run.
pub fn git_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        if cfg!(target_os = "macos") {
            let tools = process::run("xcode-select", &["-p"], Path::new("/"), Duration::from_secs(5));
            if !tools.map(|o| o.status.success()).unwrap_or(false) {
                return false;
            }
        }
        process::has_program("git")
    })
}

fn git(args: &[&str], cwd: &Path) -> Result<String, String> {
    let output = process::run("git", args, cwd, GIT_TIMEOUT).map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Root of the repository containing `dir`, if any.
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    git(&["rev-parse", "--show-toplevel"], dir)
        .ok()
        .map(|s| PathBuf::from(s.trim()))
}

/// Number of files git tracks inside `target`, asked from its parent folder so the
/// enclosing repository is found whatever its depth.
pub fn tracked_count(target: &Path) -> Result<usize, String> {
    let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
        return Err(format!("`{}` has no parent folder", target.display()));
    };
    let name = name.to_string_lossy();
    // `:(literal)` stops git from reading `*` or `?` in the folder name as a pattern.
    let pathspec = format!(":(literal){name}");
    match git(&["ls-files", "-z", "--", &pathspec], parent) {
        Ok(listed) => Ok(listed.split('\0').filter(|s| !s.is_empty()).count()),
        // Outside any repository nothing is tracked. Git's messages are in English (`LC_ALL=C`).
        Err(message) if message.contains("not a git repository") => Ok(0),
        Err(message) => Err(message),
    }
}

/// Runs every artifact check on `target`. `Ok` means the folder may be deleted.
pub fn check_artifact(target: &Path) -> Result<(), Blocked> {
    if target.join(".git").exists() {
        return Err(Blocked::ContainsRepository);
    }
    if let Some(path) = env_files(target).into_iter().next() {
        return Err(Blocked::EnvFile { path });
    }
    match tracked_count(target) {
        Ok(0) => Ok(()),
        Ok(count) => Err(Blocked::TrackedFiles { count }),
        Err(message) => Err(Blocked::GitError { message }),
    }
}

/// `.env` files directly inside `dir`.
pub fn env_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(".env"))
        .map(|e| e.path())
        .collect();
    found.sort();
    found
}

/// Uncommitted changes, unpushed commits and last commit of the repository at `root`.
pub fn repo_status(root: &Path) -> Result<RepoStatus, String> {
    let changes = git(&["status", "--short"], root)?.lines().map(str::to_string).collect();
    let has_remote = !git(&["remote"], root)?.trim().is_empty();
    let unpushed = git(&["log", "--branches", "--not", "--remotes", "--format=%H"], root)
        .map(|s| s.lines().count())
        .unwrap_or(0);
    let last_commit = git(&["log", "-1", "--format=%ct"], root)
        .ok()
        .and_then(|s| s.trim().parse().ok());
    Ok(RepoStatus {
        root: root.to_path_buf(),
        changes,
        unpushed,
        has_remote,
        last_commit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_in(dir: &Path, args: &[&str]) {
        let mut full = vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ];
        full.extend_from_slice(args);
        let output = process::run("git", &full, dir, GIT_TIMEOUT).unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A repository with a clean project, an ignored `node_modules` and a `build` folder
    /// that has tracked files despite being ignored.
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("my project");
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(root.join("build")).unwrap();
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), "").unwrap();
        fs::write(root.join("build/keep.txt"), "versioned").unwrap();
        fs::write(root.join(".gitignore"), "node_modules\nbuild\n").unwrap();
        git_in(&root, &["init", "-q"]);
        git_in(&root, &["add", "."]);
        git_in(&root, &["add", "-f", "build/keep.txt"]);
        git_in(&root, &["commit", "-q", "-m", "init"]);
        dir
    }

    #[test]
    fn blocks_folders_with_tracked_files() {
        let dir = fixture();
        let root = dir.path().join("my project");
        assert_eq!(check_artifact(&root.join("node_modules")), Ok(()));
        assert_eq!(
            check_artifact(&root.join("build")),
            Err(Blocked::TrackedFiles { count: 1 })
        );
    }

    #[test]
    fn folders_outside_any_repository_have_no_tracked_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("dist")).unwrap();
        assert_eq!(tracked_count(&dir.path().join("dist")), Ok(0));
    }

    #[test]
    fn blocks_env_files_and_nested_repositories() {
        let dir = fixture();
        let root = dir.path().join("my project");
        fs::write(root.join("build/.env.local"), "SECRET=1").unwrap();
        assert!(matches!(
            check_artifact(&root.join("build")),
            Err(Blocked::EnvFile { .. })
        ));

        fs::create_dir_all(root.join("node_modules/.git")).unwrap();
        assert_eq!(
            check_artifact(&root.join("node_modules")),
            Err(Blocked::ContainsRepository)
        );
    }

    #[test]
    fn reports_changes_and_unpushed_commits() {
        let dir = fixture();
        let root = dir.path().join("my project");
        fs::write(root.join("package.json"), "{\"name\":\"x\"}").unwrap();
        let status = repo_status(&root).unwrap();
        assert_eq!(status.changes, vec![" M package.json"]);
        assert!(!status.has_remote);
        assert_eq!(status.unpushed, 1);
        assert!(status.last_commit.is_some());
    }
}

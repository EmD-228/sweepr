//! Last check before anything is deleted. Every deletion goes through
//! [`check_deletable`], whatever rule or provider produced the path.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SafetyError {
    #[error("`{0}` is not an absolute path")]
    NotAbsolute(PathBuf),
    #[error("`{0}` contains `.` or `..`")]
    NotNormalized(PathBuf),
    #[error("`{0}` does not exist")]
    Missing(PathBuf),
    #[error("`{0}` is a protected folder")]
    Protected(PathBuf),
    #[error("`{0}` is outside the folders Sweepr may clean")]
    OutsideRoots(PathBuf),
    #[error("`{0}` is a `.env` file")]
    EnvFile(PathBuf),
}

/// Folders deletions may happen in, and folders that must never be deleted themselves.
/// Both are resolved once when the policy is built, so checking a path costs no extra syscalls.
#[derive(Debug, Clone)]
pub struct SafetyPolicy {
    /// Canonical roots.
    roots: Vec<PathBuf>,
    /// Comparison keys of protected folders, as written and as resolved.
    protected: HashSet<String>,
}

/// Folders of the home directory that hold the user's own files.
const PROTECTED_IN_HOME: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Library",
    "Movies",
    "Music",
    "Pictures",
    "Public",
    "Applications",
    "Developer",
    "Library/Application Support",
    "Library/Caches",
    "Library/Containers",
    "Library/Developer",
    "Library/Mobile Documents",
    "AppData",
    "AppData/Local",
    "AppData/Roaming",
    "Videos",
    ".ssh",
    ".gnupg",
    ".config",
    ".local",
    ".local/share",
];

const PROTECTED_ABSOLUTE: &[&str] = &[
    "/",
    "/Applications",
    "/System",
    "/Library",
    "/Users",
    "/usr",
    "/bin",
    "/sbin",
    "/etc",
    "/var",
    "/private",
    "/opt",
    "/home",
    "/Volumes",
];

impl SafetyPolicy {
    /// Deletions allowed under `home` only.
    pub fn new(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        let mut policy = SafetyPolicy {
            roots: Vec::new(),
            protected: HashSet::new(),
        };
        for path in PROTECTED_ABSOLUTE
            .iter()
            .map(PathBuf::from)
            .chain(PROTECTED_IN_HOME.iter().map(|p| home.join(p)))
        {
            policy.protect(&path);
        }
        policy.allow_root(home)
    }

    fn protect(&mut self, path: &Path) {
        self.protected.insert(key(path));
        self.protected.insert(key(&canonical_or_self(path)));
    }

    /// Also allows deletions under `root` (for example `/Applications` for old macOS installers,
    /// or the temporary folder on Windows). The root itself stays protected.
    pub fn allow_root(mut self, root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        self.protect(&root);
        self.roots.push(canonical_or_self(&root));
        self
    }

    /// Checks that `path` may be deleted. `path` itself may be a symbolic link (the link is
    /// removed, not its target), but its parent folders are resolved so a link cannot lead
    /// a deletion outside the allowed roots.
    pub fn check_deletable(&self, path: &Path) -> Result<PathBuf, SafetyError> {
        if !path.is_absolute() {
            return Err(SafetyError::NotAbsolute(path.to_path_buf()));
        }
        if path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
        {
            return Err(SafetyError::NotNormalized(path.to_path_buf()));
        }
        if path.symlink_metadata().is_err() {
            return Err(SafetyError::Missing(path.to_path_buf()));
        }
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(SafetyError::Protected(path.to_path_buf()));
        };
        let real_parent = parent
            .canonicalize()
            .map_err(|_| SafetyError::Missing(path.to_path_buf()))?;
        let real = real_parent.join(name);

        if Path::new(name).to_string_lossy().starts_with(".env") {
            return Err(SafetyError::EnvFile(path.to_path_buf()));
        }
        if self.protected.contains(&key(path)) || self.protected.contains(&key(&real)) {
            return Err(SafetyError::Protected(path.to_path_buf()));
        }
        let inside_root = self.roots.iter().any(|root| real.starts_with(root) && &real != root);
        if !inside_root {
            return Err(SafetyError::OutsideRoots(path.to_path_buf()));
        }
        Ok(real)
    }
}

fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Comparison key of a path. Default macOS and Windows volumes are case-insensitive.
fn key(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(any(target_os = "macos", windows)) {
        text.to_lowercase()
    } else {
        text.into_owned()
    }
}

/// Deletes a path already accepted by [`SafetyPolicy::check_deletable`]. A symbolic link is
/// removed without touching its target; `remove_dir_all` does not follow links inside folders.
pub fn remove(path: &Path) -> std::io::Result<()> {
    let meta = path.symlink_metadata()?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup() -> (tempfile::TempDir, PathBuf, SafetyPolicy) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap().join("home user");
        fs::create_dir_all(home.join("Library/Caches/app")).unwrap();
        fs::create_dir_all(home.join("Documents/project/node_modules")).unwrap();
        fs::write(home.join("Documents/project/.env"), "SECRET=1").unwrap();
        let policy = SafetyPolicy::new(&home);
        (dir, home, policy)
    }

    #[test]
    fn accepts_paths_inside_home() {
        let (_dir, home, policy) = setup();
        assert!(policy.check_deletable(&home.join("Library/Caches/app")).is_ok());
        assert!(policy
            .check_deletable(&home.join("Documents/project/node_modules"))
            .is_ok());
    }

    #[test]
    fn refuses_protected_and_outside_paths() {
        let (dir, home, policy) = setup();
        assert!(matches!(policy.check_deletable(&home), Err(SafetyError::Protected(_))));
        assert!(matches!(
            policy.check_deletable(&home.join("Documents")),
            Err(SafetyError::Protected(_))
        ));
        assert!(matches!(
            policy.check_deletable(&home.join("Library/Caches")),
            Err(SafetyError::Protected(_))
        ));
        assert!(matches!(
            policy.check_deletable(&home.join("Documents/project/.env")),
            Err(SafetyError::EnvFile(_))
        ));
        assert!(matches!(
            policy.check_deletable(Path::new("relative")),
            Err(SafetyError::NotAbsolute(_))
        ));
        assert!(matches!(
            policy.check_deletable(&home.join("Documents/project/../project")),
            Err(SafetyError::NotNormalized(_))
        ));
        assert!(matches!(
            policy.check_deletable(&home.join("nope")),
            Err(SafetyError::Missing(_))
        ));
        fs::create_dir(dir.path().join("elsewhere")).unwrap();
        assert!(matches!(
            policy.check_deletable(&dir.path().canonicalize().unwrap().join("elsewhere")),
            Err(SafetyError::OutsideRoots(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_parent_cannot_escape_home() {
        let (dir, home, policy) = setup();
        let outside = dir.path().join("outside");
        fs::create_dir_all(outside.join("precious")).unwrap();
        std::os::unix::fs::symlink(&outside, home.join("Library/Caches/link")).unwrap();

        // Through the link, `precious` really lives outside home.
        let through_link = home.join("Library/Caches/link/precious");
        assert!(matches!(
            policy.check_deletable(&through_link),
            Err(SafetyError::OutsideRoots(_))
        ));

        // The link itself may be removed, and removing it leaves the target alone.
        let link = home.join("Library/Caches/link");
        assert!(policy.check_deletable(&link).is_ok());
        remove(&link).unwrap();
        assert!(outside.join("precious").exists());
    }

    #[test]
    fn extra_roots_stay_protected_themselves() {
        let (dir, _home, policy) = setup();
        let apps = dir.path().canonicalize().unwrap().join("Applications");
        fs::create_dir_all(apps.join("Install macOS Sonoma.app")).unwrap();
        let policy = policy.allow_root(&apps);
        assert!(policy.check_deletable(&apps.join("Install macOS Sonoma.app")).is_ok());
        assert!(matches!(policy.check_deletable(&apps), Err(SafetyError::Protected(_))));
    }
}

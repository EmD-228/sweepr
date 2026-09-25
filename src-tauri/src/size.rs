//! Disk usage of files and folders, computed in parallel.
//!
//! Two sizes are kept: the apparent size (sum of file lengths) and the allocated size
//! (blocks really used on disk). Hard links are counted once, including across several
//! calls sharing the same [`Seen`] set, so the pnpm store and its links into projects are
//! not counted twice. APFS clones share blocks too but cannot be detected cheaply: that is
//! why the real gain is always measured afterwards (see `measure`).

use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use rayon::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Usage {
    /// Bytes allocated on disk. The figure shown to the user.
    pub allocated: u64,
    /// Sum of file lengths.
    pub apparent: u64,
    pub files: u64,
    /// Entries that could not be read (permissions, files removed during the scan).
    pub unreadable: u64,
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, other: Usage) {
        self.allocated += other.allocated;
        self.apparent += other.apparent;
        self.files += other.files;
        self.unreadable += other.unreadable;
    }
}

impl std::ops::Add for Usage {
    type Output = Usage;

    fn add(mut self, other: Usage) -> Usage {
        self += other;
        self
    }
}

/// Allocated bytes of several paths, hard links counted once.
pub fn total_allocated(paths: &[PathBuf], seen: &Seen, cancel: &AtomicBool) -> u64 {
    paths.iter().map(|p| usage(p, seen, cancel).allocated).sum()
}

/// Files with several hard links already counted, by (device, inode).
#[derive(Debug, Default)]
pub struct Seen(Mutex<HashSet<(u64, u64)>>);

impl Seen {
    fn first_time(&self, key: (u64, u64)) -> bool {
        self.0.lock().map(|mut set| set.insert(key)).unwrap_or(true)
    }
}

/// Computes the usage of a file or folder. Symbolic links are counted as links, never
/// followed, and other volumes mounted inside the folder are skipped.
pub fn usage(path: &Path, seen: &Seen, cancel: &AtomicBool) -> Usage {
    match path.symlink_metadata() {
        Ok(meta) => {
            let device = device_of(&meta);
            walk(path, &meta, device, seen, cancel)
        }
        Err(_) => Usage { unreadable: 1, ..Usage::default() },
    }
}

fn walk(path: &Path, meta: &Metadata, device: u64, seen: &Seen, cancel: &AtomicBool) -> Usage {
    if !meta.is_dir() {
        return file_usage(meta, seen);
    }
    let mut total = Usage { allocated: allocated_of(meta), ..Usage::default() };
    if cancel.load(Ordering::Relaxed) {
        return total;
    }
    let entries: Vec<_> = match fs::read_dir(path) {
        Ok(entries) => entries.collect(),
        Err(_) => {
            total.unreadable += 1;
            return total;
        }
    };
    let children = entries
        .into_par_iter()
        .map(|entry| {
            let Ok(entry) = entry else {
                return Usage { unreadable: 1, ..Usage::default() };
            };
            let child = entry.path();
            match child.symlink_metadata() {
                Ok(meta) if meta.is_dir() && device_of(&meta) != device => Usage::default(),
                Ok(meta) => walk(&child, &meta, device, seen, cancel),
                Err(_) => Usage { unreadable: 1, ..Usage::default() },
            }
        })
        .reduce(Usage::default, |a, b| a + b);
    total += children;
    total
}

fn file_usage(meta: &Metadata, seen: &Seen) -> Usage {
    if let Some(key) = hard_link_key(meta) {
        if !seen.first_time(key) {
            return Usage::default();
        }
    }
    Usage { allocated: allocated_of(meta), apparent: meta.len(), files: 1, unreadable: 0 }
}

#[cfg(unix)]
fn allocated_of(meta: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks() * 512
}

#[cfg(not(unix))]
fn allocated_of(meta: &Metadata) -> u64 {
    meta.len()
}

#[cfg(unix)]
fn device_of(meta: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.dev()
}

#[cfg(not(unix))]
fn device_of(_meta: &Metadata) -> u64 {
    0
}

#[cfg(unix)]
fn hard_link_key(meta: &Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    (meta.nlink() > 1).then(|| (meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn hard_link_key(_meta: &Metadata) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    #[test]
    fn counts_files_recursively() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/one"), vec![0u8; 10_000]).unwrap();
        fs::write(dir.path().join("a/b/two"), vec![0u8; 5_000]).unwrap();

        let usage = usage(dir.path(), &Seen::default(), &no_cancel());
        assert_eq!(usage.files, 2);
        assert_eq!(usage.apparent, 15_000);
        assert!(usage.allocated >= 15_000);
        assert_eq!(usage.unreadable, 0);
    }

    #[cfg(unix)]
    #[test]
    fn counts_hard_links_once_across_calls() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("store")).unwrap();
        fs::create_dir_all(dir.path().join("project")).unwrap();
        fs::write(dir.path().join("store/pkg"), vec![0u8; 8_000]).unwrap();
        fs::hard_link(dir.path().join("store/pkg"), dir.path().join("project/pkg")).unwrap();

        let seen = Seen::default();
        let store = usage(&dir.path().join("store"), &seen, &no_cancel());
        let project = usage(&dir.path().join("project"), &seen, &no_cancel());
        assert_eq!(store.apparent + project.apparent, 8_000);
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_symbolic_links() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("big")).unwrap();
        fs::create_dir_all(dir.path().join("scanned")).unwrap();
        fs::write(dir.path().join("big/file"), vec![0u8; 50_000]).unwrap();
        std::os::unix::fs::symlink(dir.path().join("big"), dir.path().join("scanned/link")).unwrap();

        let usage = usage(&dir.path().join("scanned"), &Seen::default(), &no_cancel());
        assert!(usage.apparent < 50_000);
    }

    #[test]
    fn missing_path_is_unreadable() {
        let usage = usage(Path::new("/definitely/not/here"), &Seen::default(), &no_cancel());
        assert_eq!(usage, Usage { unreadable: 1, ..Usage::default() });
    }
}

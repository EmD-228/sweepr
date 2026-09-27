//! Large files and duplicates in the user's own folders (SPEC section 5).
//!
//! One walk collects the files and both lists come from it. Nothing here deletes: the items go
//! through `exec` like every other, and a duplicate is compared byte for byte with the copy
//! kept right before it is deleted ([`same_content`]).

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, Metadata};
use std::hash::{DefaultHasher, Hasher};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use rayon::prelude::*;

use crate::catalog::Ecosystem;
use crate::platform;
use crate::projects::{is_hidden, NEVER_ENTER};
use crate::size;

/// Folders of the home directory searched for duplicates.
pub const FOLDERS: &[&str] = &["Desktop", "Documents", "Downloads", "Movies", "Music", "Pictures"];

/// Folders where large files are listed (SPEC section 5). Elsewhere a large file is usually
/// kept on purpose.
pub const LARGE_FILE_FOLDERS: &[&str] = &["Desktop", "Downloads", "Movies"];

/// A large file frees at least this much.
pub const LARGE_FILE_MIN: u64 = 200_000_000;

/// Smaller duplicates are not worth a decision.
pub const DUPLICATE_MIN: u64 = 1_000_000;

/// Folders that are one document for the user, managed by an application (Photos library,
/// Final Cut, Logic, apps): never cleaned file by file.
const PACKAGES: &[&str] = &[
    "app",
    "aplibrary",
    "band",
    "bundle",
    "fcpbundle",
    "framework",
    "imovielibrary",
    "logicx",
    "lrdata",
    "migratedphotolibrary",
    "musiclibrary",
    "photolibrary",
    "photoslibrary",
    "plugin",
    "sparsebundle",
    "tvlibrary",
    "xcarchive",
    "xcodeproj",
    "xcworkspace",
];

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    /// Folder of the home directory the file is in, from [`FOLDERS`].
    pub folder: &'static str,
    pub len: u64,
    allocated: u64,
    modified: SystemTime,
    /// (device, inode): hard links to the same file share it.
    key: Option<(u64, u64)>,
}

impl FileInfo {
    /// Bytes deleting the file would free: the blocks it does not share with a clone.
    pub fn freed(&self) -> u64 {
        platform::private_size(&self.path).unwrap_or(self.allocated)
    }
}

/// Files of at least [`DUPLICATE_MIN`] bytes in [`FOLDERS`], stored on this Mac. Skipped:
/// hidden entries, symbolic links, other volumes, application packages, git repositories,
/// dependency and build folders of the catalog's ecosystems, and files whose content is only
/// in iCloud (reading them would download them).
pub fn collect(home: &Path, ecosystems: &[Ecosystem], cancel: &AtomicBool) -> Vec<FileInfo> {
    // `android/app/build` and `**/__pycache__` are skipped by their last name.
    let project_folders: HashSet<&str> = NEVER_ENTER
        .iter()
        .copied()
        .chain(
            ecosystems
                .iter()
                .flat_map(|e| &e.artifacts)
                .filter_map(|a| a.rsplit('/').next()),
        )
        .collect();
    FOLDERS
        .par_iter()
        .flat_map_iter(|folder| {
            let root = home.join(folder);
            match root.symlink_metadata() {
                Ok(meta) if meta.is_dir() => walk(&root, folder, size::device_of(&meta), &project_folders, cancel),
                _ => Vec::new(),
            }
        })
        .collect()
}

fn walk(
    dir: &Path,
    folder: &'static str,
    device: u64,
    project_folders: &HashSet<&str>,
    cancel: &AtomicBool,
) -> Vec<FileInfo> {
    if cancel.load(Ordering::Relaxed) {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let entries: Vec<_> = entries.filter_map(Result::ok).collect();
    // A git repository: its files belong to the code.
    if entries.iter().any(|e| e.file_name() == ".git") {
        return Vec::new();
    }
    entries
        .into_par_iter()
        .flat_map_iter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if is_hidden(&name) {
                return Vec::new();
            }
            // `DirEntry::metadata` does not follow symbolic links.
            match entry.metadata() {
                Ok(meta)
                    if meta.is_dir()
                        && size::device_of(&meta) == device
                        && !is_package(&name)
                        && !project_folders.contains(name.as_ref()) =>
                {
                    walk(&entry.path(), folder, device, project_folders, cancel)
                }
                Ok(meta) if meta.is_file() && meta.len() >= DUPLICATE_MIN && !platform::is_dataless(&meta) => {
                    vec![file_info(entry.path(), folder, &meta)]
                }
                _ => Vec::new(),
            }
        })
        .collect()
}

fn is_package(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PACKAGES.contains(&e.to_ascii_lowercase().as_str()))
}

fn file_info(path: PathBuf, folder: &'static str, meta: &Metadata) -> FileInfo {
    FileInfo {
        path,
        folder,
        len: meta.len(),
        allocated: size::allocated_of(meta),
        modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        key: size::inode_of(meta),
    }
}

/// Files in [`LARGE_FILE_FOLDERS`] that free at least [`LARGE_FILE_MIN`], largest first,
/// with the bytes each would free.
pub fn large_files<'a>(files: &'a [FileInfo], skip: &HashSet<&Path>) -> Vec<(&'a FileInfo, u64)> {
    let mut large: Vec<(&FileInfo, u64)> = files
        .par_iter()
        .filter(|f| f.len >= LARGE_FILE_MIN && LARGE_FILE_FOLDERS.contains(&f.folder))
        .filter(|f| !skip.contains(f.path.as_path()))
        .map(|f| (f, f.freed()))
        .filter(|(_, freed)| *freed >= LARGE_FILE_MIN)
        .collect();
    large.sort_by_key(|(_, freed)| std::cmp::Reverse(*freed));
    large
}

/// Files with the same content. One copy is kept: the one outside Downloads, then the
/// oldest, then the one with the shortest path.
#[derive(Debug, Clone)]
pub struct Duplicates<'a> {
    pub original: &'a FileInfo,
    pub copies: Vec<&'a FileInfo>,
    /// Bytes deleting the copies would free.
    pub freed: u64,
}

/// Groups files by size, then by a sample of their content, then by their whole content.
/// Hard links are one file, not duplicates. Copies made in the Finder share their blocks with
/// the original: deleting them frees nothing, so groups freeing less than [`DUPLICATE_MIN`]
/// are dropped, before their content is read in full when possible.
pub fn duplicates<'a>(files: &'a [FileInfo], cancel: &AtomicBool) -> Vec<Duplicates<'a>> {
    let mut inodes = HashSet::new();
    let mut by_len: HashMap<u64, Vec<&FileInfo>> = HashMap::new();
    for file in files {
        if file.key.is_none_or(|key| inodes.insert(key)) {
            by_len.entry(file.len).or_default().push(file);
        }
    }
    let frees_enough = |group: &[&FileInfo]| group.iter().map(|f| f.freed()).sum::<u64>() >= DUPLICATE_MIN;
    by_len
        .into_par_iter()
        .filter(|(_, group)| group.len() > 1)
        .flat_map_iter(|(_, group)| {
            split(group, sample_hash)
                .into_iter()
                .filter(|group| frees_enough(group))
                .flat_map(|group| split(group, |path| full_hash(path, cancel)))
        })
        .filter_map(|mut group| {
            group.sort_by(|a, b| original_order(a).cmp(&original_order(b)));
            let copies = group.split_off(1);
            let freed = copies.iter().map(|c| c.freed()).sum();
            (freed >= DUPLICATE_MIN).then(|| Duplicates {
                original: group[0],
                copies,
                freed,
            })
        })
        .collect()
}

fn original_order(file: &FileInfo) -> (bool, SystemTime, usize, &Path) {
    (
        file.folder == "Downloads",
        file.modified,
        file.path.as_os_str().len(),
        &file.path,
    )
}

/// Sub-groups of files with the same hash, of two files or more. Unreadable files drop out.
fn split(group: Vec<&FileInfo>, hash: impl Fn(&Path) -> io::Result<u64> + Sync) -> Vec<Vec<&FileInfo>> {
    let hashed: Vec<(u64, &FileInfo)> = group
        .into_par_iter()
        .filter_map(|file| hash(&file.path).ok().map(|h| (h, file)))
        .collect();
    let mut buckets: HashMap<u64, Vec<&FileInfo>> = HashMap::new();
    for (h, file) in hashed {
        buckets.entry(h).or_default().push(file);
    }
    buckets.into_values().filter(|b| b.len() > 1).collect()
}

const SAMPLE: usize = 64 * 1024;
const CHUNK: usize = 1024 * 1024;

/// Hash of the first and last 64 KiB: tells most files of the same size apart without
/// reading them whole.
fn sample_hash(path: &Path) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; SAMPLE];
    let mut hasher = DefaultHasher::new();
    let n = read_full(&mut file, &mut buffer)?;
    hasher.write(&buffer[..n]);
    if file.seek(SeekFrom::End(-(SAMPLE as i64))).is_ok() {
        let n = read_full(&mut file, &mut buffer)?;
        hasher.write(&buffer[..n]);
    }
    Ok(hasher.finish())
}

fn full_hash(path: &Path, cancel: &AtomicBool) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; CHUNK];
    let mut hasher = DefaultHasher::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let n = read_full(&mut file, &mut buffer)?;
        hasher.write(&buffer[..n]);
        if n < buffer.len() {
            return Ok(hasher.finish());
        }
    }
}

/// Whether `copy` is a separate file with exactly the content of `original`, compared byte
/// for byte. Run right before a duplicate is deleted: the original must still be there.
pub fn same_content(original: &Path, copy: &Path) -> io::Result<bool> {
    let (mut a, mut b) = (File::open(original)?, File::open(copy)?);
    let (meta_a, meta_b) = (a.metadata()?, b.metadata()?);
    if !meta_a.is_file() || !meta_b.is_file() || meta_a.len() != meta_b.len() {
        return Ok(false);
    }
    let inode = size::inode_of(&meta_a);
    if inode.is_some() && inode == size::inode_of(&meta_b) {
        // The same file under two names: deleting one would not leave a copy.
        return Ok(false);
    }
    let (mut buf_a, mut buf_b) = (vec![0u8; CHUNK], vec![0u8; CHUNK]);
    loop {
        let n = read_full(&mut a, &mut buf_a)?;
        let m = read_full(&mut b, &mut buf_b)?;
        if n != m || buf_a[..n] != buf_b[..m] {
            return Ok(false);
        }
        if n < buf_a.len() {
            return Ok(true);
        }
    }
}

/// Reads until `buffer` is full or the file ends.
fn read_full(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn content(byte: u8, len: usize) -> Vec<u8> {
        (0..len).map(|i| byte.wrapping_add((i % 251) as u8)).collect()
    }

    fn names(files: &[&FileInfo]) -> Vec<String> {
        let mut names: Vec<String> = files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn finds_duplicates_and_keeps_the_copy_outside_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let video = content(1, 2_000_000);
        write(&home.join("Downloads/clip.mov"), &video);
        write(&home.join("Documents/Montage/clip.mov"), &video);
        write(&home.join("Desktop/clip copie.mov"), &video);
        // Same size, same start and end, different middle.
        let mut other = video.clone();
        other[1_000_000] ^= 0xff;
        write(&home.join("Documents/autre.mov"), &other);
        // Small files are ignored.
        write(&home.join("Documents/a.txt"), b"same");
        write(&home.join("Desktop/a.txt"), b"same");

        let files = collect(home, &[], &AtomicBool::new(false));
        let groups = duplicates(&files, &AtomicBool::new(false));
        assert_eq!(groups.len(), 1);
        let group = &groups[0];
        assert_eq!(group.original.folder, "Documents");
        assert_eq!(names(&group.copies), vec!["clip copie.mov", "clip.mov"]);
        assert!(group.copies.iter().any(|c| c.folder == "Downloads"));
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_are_not_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        write(&home.join("Documents/a.bin"), &content(3, 1_500_000));
        fs::create_dir_all(home.join("Desktop")).unwrap();
        fs::hard_link(home.join("Documents/a.bin"), home.join("Desktop/a.bin")).unwrap();

        let files = collect(home, &[], &AtomicBool::new(false));
        assert_eq!(files.len(), 2);
        assert!(duplicates(&files, &AtomicBool::new(false)).is_empty());
        assert!(!same_content(&home.join("Documents/a.bin"), &home.join("Desktop/a.bin")).unwrap());
    }

    #[test]
    fn skips_repositories_packages_and_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let data = content(5, 1_200_000);
        write(&home.join("Documents/app/.git/HEAD"), b"ref");
        write(&home.join("Documents/app/assets/big.bin"), &data);
        write(&home.join("Documents/site/node_modules/pkg/big.bin"), &data);
        write(
            &home.join("Pictures/Photos Library.photoslibrary/originals/big.bin"),
            &data,
        );
        write(&home.join("Documents/.hidden/big.bin"), &data);
        write(&home.join("Documents/kept.bin"), &data);

        let files = collect(home, &[], &AtomicBool::new(false));
        let paths: Vec<_> = files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(paths, vec![home.join("Documents/kept.bin")]);
    }

    #[test]
    fn large_files_come_from_their_folders_only() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        // Sparse files: large without using disk space, so they free nothing.
        for path in ["Downloads/sparse.iso", "Documents/sparse.iso"] {
            fs::create_dir_all(home.join(path).parent().unwrap()).unwrap();
            File::create(home.join(path))
                .unwrap()
                .set_len(LARGE_FILE_MIN + 1)
                .unwrap();
        }
        let files = collect(home, &[], &AtomicBool::new(false));
        assert_eq!(files.len(), 2);
        assert!(
            large_files(&files, &HashSet::new()).is_empty(),
            "a sparse file frees nothing"
        );
    }

    #[test]
    fn compares_content_byte_for_byte() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, c) = (dir.path().join("a"), dir.path().join("b"), dir.path().join("c"));
        let data = content(9, CHUNK + 10);
        fs::write(&a, &data).unwrap();
        fs::write(&b, &data).unwrap();
        let mut changed = data.clone();
        changed[CHUNK + 5] ^= 1;
        fs::write(&c, &changed).unwrap();

        assert!(same_content(&a, &b).unwrap());
        assert!(!same_content(&a, &c).unwrap());
        assert!(same_content(&dir.path().join("gone"), &b).is_err());
    }
}

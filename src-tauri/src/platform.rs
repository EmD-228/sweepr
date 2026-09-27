//! System calls that differ between operating systems.

use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskSpace {
    /// Capacity of the volume.
    pub total: u64,
    /// Space available to the user (what `df` reports).
    pub free: u64,
}

/// Capacity and free space of the volume holding `path`, in bytes.
#[cfg(unix)]
pub fn disk_space(path: &Path) -> io::Result<DiskSpace> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path =
        CString::new(path.as_os_str().as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c_path` is a valid NUL-terminated string and `stat` a valid out pointer.
    let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    #[allow(clippy::unnecessary_cast)]
    let block = stat.f_frsize as u64;
    #[allow(clippy::unnecessary_cast)]
    Ok(DiskSpace {
        total: stat.f_blocks as u64 * block,
        free: stat.f_bavail as u64 * block,
    })
}

/// Capacity and free space of the volume holding `path`, in bytes.
#[cfg(windows)]
pub fn disk_space(path: &Path) -> io::Result<DiskSpace> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut available: u64 = 0;
    let mut total: u64 = 0;
    // SAFETY: `wide` is NUL-terminated; the unused out parameter may be null.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, std::ptr::null_mut()) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(DiskSpace { total, free: available })
}

/// Space available to the user on the volume holding `path`, in bytes.
pub fn free_space(path: &Path) -> io::Result<u64> {
    disk_space(path).map(|d| d.free)
}

/// macOS: whether Sweepr has Full Disk Access, needed to read the Trash, Mail and Messages.
/// Tested on `~/Library/Safari`, which exists on every Mac and is always protected.
/// `None` on other systems or when the answer cannot be known.
pub fn full_disk_access(home: &Path) -> Option<bool> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    match std::fs::read_dir(home.join("Library/Safari")) {
        Ok(_) => Some(true),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => Some(false),
        Err(_) => None,
    }
}

/// System settings page where the user grants Full Disk Access.
pub const FULL_DISK_ACCESS_SETTINGS: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// macOS: whether the file's content is only in iCloud. Reading it would download it.
#[cfg(target_os = "macos")]
pub fn is_dataless(meta: &std::fs::Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    /// `SF_DATALESS` in `<sys/stat.h>`, not exported by the `libc` crate.
    const SF_DATALESS: u32 = 0x4000_0000;
    meta.st_flags() & SF_DATALESS != 0
}

#[cfg(not(target_os = "macos"))]
pub fn is_dataless(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Bytes deleting the file would free. On APFS a copy made in the Finder is a clone that
/// shares its blocks with the original, so deleting it frees only the blocks it does not
/// share (`ATTR_CMNEXT_PRIVATESIZE`). `None` when the file system cannot tell.
#[cfg(target_os = "macos")]
pub fn private_size(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    #[repr(C, packed(4))]
    struct Buffer {
        length: u32,
        private_size: libc::off_t,
    }

    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: `attrlist` is plain data; all-zero is a valid empty request.
    let mut request: libc::attrlist = unsafe { std::mem::zeroed() };
    request.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    request.forkattr = libc::ATTR_CMNEXT_PRIVATESIZE;
    let mut buffer = Buffer {
        length: 0,
        private_size: 0,
    };
    // SAFETY: valid NUL-terminated path, request and buffer pointers, and the buffer size.
    let result = unsafe {
        libc::getattrlist(
            c_path.as_ptr(),
            (&mut request as *mut libc::attrlist).cast(),
            (&mut buffer as *mut Buffer).cast(),
            std::mem::size_of::<Buffer>(),
            libc::FSOPT_ATTR_CMN_EXTENDED | libc::FSOPT_NOFOLLOW,
        )
    };
    // A file system without the attribute returns only the length field.
    let (length, size) = (buffer.length, buffer.private_size);
    if result != 0 || (length as usize) < std::mem::size_of::<Buffer>() {
        return None;
    }
    u64::try_from(size).ok()
}

#[cfg(not(target_os = "macos"))]
pub fn private_size(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_free_space_of_temp_dir() {
        let free = free_space(&std::env::temp_dir()).unwrap();
        assert!(free > 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_clone_frees_nothing_but_a_copy_does() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original");
        std::fs::write(&original, vec![7u8; 1_000_000]).unwrap();
        let clone = dir.path().join("clone");
        let copy = dir.path().join("copy");
        let status = std::process::Command::new("cp")
            .arg("-c")
            .args([&original, &clone])
            .status()
            .unwrap();
        assert!(status.success());
        // Written again rather than `fs::copy`, which clones on APFS.
        std::fs::write(&copy, vec![7u8; 1_000_000]).unwrap();

        // Temporary folders are on APFS on every supported Mac. The original and its
        // clone share their blocks: deleting either one alone frees almost nothing.
        assert!(private_size(&clone).unwrap() < 100_000);
        assert!(private_size(&original).unwrap() < 100_000);
        assert!(private_size(&copy).unwrap() >= 1_000_000);
    }
}

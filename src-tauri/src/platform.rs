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

    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c_path` is a valid NUL-terminated string and `stat` a valid out pointer.
    let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    #[allow(clippy::unnecessary_cast)]
    let block = stat.f_frsize as u64;
    #[allow(clippy::unnecessary_cast)]
    Ok(DiskSpace { total: stat.f_blocks as u64 * block, free: stat.f_bavail as u64 * block })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_free_space_of_temp_dir() {
        let free = free_space(&std::env::temp_dir()).unwrap();
        assert!(free > 0);
    }
}

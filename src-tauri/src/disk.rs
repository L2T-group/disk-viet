use serde::Serialize;
use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub path: String,
    pub name: String,
    pub total: u64,
    pub free: u64,
    pub is_boot: bool,
}

fn statfs(path: &Path) -> io::Result<libc::statfs> {
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let mut s = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: c is a valid NUL-terminated string; s is only read after statfs returns 0.
    if unsafe { libc::statfs(c.as_ptr(), s.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { s.assume_init() })
}

/// Bytes available to unprivileged users (`f_bavail * f_bsize`).
pub fn free_bytes(path: &Path) -> io::Result<u64> {
    let s = statfs(path)?;
    Ok(s.f_bavail * u64::from(s.f_bsize))
}

/// Mount point of the volume holding `path`.
pub fn mount_point(path: &Path) -> io::Result<std::path::PathBuf> {
    let s = statfs(path)?;
    let c = unsafe { CStr::from_ptr(s.f_mntonname.as_ptr()) };
    Ok(std::ffi::OsStr::from_bytes(c.to_bytes()).into())
}

/// Bytes in use (`(f_blocks - f_bfree) * f_bsize`).
pub fn used_bytes(path: &Path) -> io::Result<u64> {
    let s = statfs(path)?;
    Ok(s.f_blocks.saturating_sub(s.f_bfree) * u64::from(s.f_bsize))
}

/// Name of the `/Volumes` entry that is a symlink to `/`, else "/".
fn boot_name() -> String {
    std::fs::read_dir("/Volumes")
        .into_iter()
        .flatten()
        .flatten()
        .find(|e| std::fs::read_link(e.path()).is_ok_and(|l| l == Path::new("/")))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".into())
}

/// Mounted volumes: `/` plus direct children of `/Volumes/`.
pub fn volumes() -> Vec<Volume> {
    let mut buf: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo points buf at a libc-owned array of n entries (not to be freed).
    let n = unsafe { libc::getmntinfo(&mut buf, libc::MNT_NOWAIT) };
    if n <= 0 || buf.is_null() {
        return Vec::new();
    }
    let entries = unsafe { std::slice::from_raw_parts(buf, n as usize) };
    let mut out = Vec::new();
    for s in entries {
        let path = unsafe { CStr::from_ptr(s.f_mntonname.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let is_boot = path == "/";
        let name = if is_boot {
            boot_name()
        } else {
            match path.strip_prefix("/Volumes/") {
                Some(x) if !x.is_empty() && !x.contains('/') => x.to_string(),
                _ => continue,
            }
        };
        let bsize = u64::from(s.f_bsize);
        out.push(Volume {
            path,
            name,
            total: s.f_blocks * bsize,
            free: s.f_bavail * bsize,
            is_boot,
        });
    }
    out
}

#[tauri::command]
pub fn volumes_list() -> Vec<Volume> {
    volumes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn free_bytes_matches_df() {
        let out = std::process::Command::new("df")
            .args(["-k", "/"])
            .output()
            .unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        let avail: f64 = text
            .lines()
            .nth(1)
            .unwrap()
            .split_whitespace()
            .nth(3)
            .unwrap()
            .parse()
            .unwrap();
        let df = avail * 1024.0;
        let got = free_bytes(Path::new("/")).unwrap() as f64;
        assert!((got - df).abs() <= df * 0.01, "got {got}, df {df}");
    }

    #[test]
    fn volumes_has_boot() {
        let boot: Vec<_> = volumes().into_iter().filter(|v| v.is_boot).collect();
        assert_eq!(boot.len(), 1);
        assert_eq!(boot[0].path, "/");
    }
}

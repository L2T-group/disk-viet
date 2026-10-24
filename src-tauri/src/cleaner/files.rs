//! Filesystem side of deleting: Trash via Finder, permanent delete that never leaves the
//! volume or follows symlinks, and the running-process probe.
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use trash::TrashContext;
use trash::macos::{DeleteMethod, TrashContextExtMacos};

/// Moves `path` (itself, never a symlink's target) to the Trash through Finder, so "Put Back"
/// works. No fallback to permanent delete.
pub fn trash(path: &Path) -> Result<(), String> {
    // The crate %-encodes non-UTF-8 names for Finder, which could name a different file.
    if path.to_str().is_none() {
        return Err("trash_failed: non-UTF-8 path".into());
    }
    let mut ctx = TrashContext::default();
    // Finder hangs (AppleEvent timeout) on a symlink to a folder; a link has nothing to Put Back.
    let link = fs::symlink_metadata(path).is_ok_and(|m| m.is_symlink());
    ctx.set_delete_method(if link {
        DeleteMethod::NsFileManager
    } else {
        DeleteMethod::Finder
    });
    // Finder can finish the move yet answer "AppleEvent timed out" (-1712): gone counts as done.
    ctx.delete(path)
        .or_else(|e| match fs::symlink_metadata(path) {
            Err(g) if g.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(trash_reason(&e.to_string())),
        })
}

/// -1743 (errAEEventNotPermitted): the user (or macOS) denied DiskViet control of Finder.
fn trash_reason(msg: &str) -> String {
    if msg.contains("-1743") {
        "trash_denied".into()
    } else {
        format!("trash_failed: {msg}")
    }
}

/// Deletes `path` for good. A symlink or file is unlinked; a directory is emptied entry by
/// entry. Returns the first error; other entries are still attempted.
pub fn remove(path: &Path) -> Result<(), String> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() {
        return fs::remove_file(path).map_err(|e| e.to_string());
    }
    let mut first = None;
    remove_tree(path, m.dev(), &mut first);
    first.map_or(Ok(()), Err)
}

/// Like `remove_dir_all`, but never descends into another device (`mount_inside`) and keeps
/// going after errors. Entries are `lstat`ed, so symlinks are unlinked, not followed.
/// shortcut: an entry swapped for a symlink between lstat and read_dir is followed; use
/// openat(O_NOFOLLOW) if a hostile local writer ever matters.
/// shortcut: unlinking while iterating read_dir can skip entries, so remove_dir may fail with
/// ENOTEMPTY (nothing extra is lost); add one retry pass if users report it.
fn remove_tree(dir: &Path, dev: u64, first: &mut Option<String>) {
    match fs::read_dir(dir) {
        Err(e) => note(first, e),
        Ok(entries) => {
            for e in entries {
                let p = match e {
                    Ok(e) => e.path(),
                    Err(e) => {
                        note(first, e);
                        continue;
                    }
                };
                match fs::symlink_metadata(&p) {
                    Err(e) => note(first, e),
                    Ok(m) if m.is_dir() && m.dev() != dev => note(first, "mount_inside"),
                    Ok(m) if m.is_dir() => remove_tree(&p, dev, first),
                    Ok(_) => {
                        if let Err(e) = fs::remove_file(&p) {
                            note(first, e);
                        }
                    }
                }
            }
        }
    }
    if let Err(e) = fs::remove_dir(dir) {
        note(first, e);
    }
}

fn note(first: &mut Option<String>, e: impl ToString) {
    first.get_or_insert_with(|| e.to_string());
}

/// `pgrep -x <name>`. Only exit code 1 ("no match") counts as not running: any failure to
/// ask errs on the side of "running".
pub fn is_running(name: &str) -> bool {
    !matches!(
        Command::new("pgrep")
            .args(["-x", name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.code()),
        Ok(Some(1))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("dv-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p.canonicalize().unwrap()
    }

    #[test]
    fn finder_denial_has_its_own_reason() {
        let denied = "osascript: Not authorized to send Apple events to Finder. (-1743)";
        assert_eq!(trash_reason(denied), "trash_denied");
        assert_eq!(trash_reason("timeout (-1712)"), "trash_failed: timeout (-1712)");
    }

    #[test]
    fn remove_dir_keeps_symlink_targets() {
        let d = tmp("links");
        let outside = tmp("links-out");
        fs::write(outside.join("keep"), "x").unwrap();
        fs::create_dir_all(d.join("top/sub")).unwrap();
        fs::write(d.join("top/sub/f"), "x").unwrap();
        symlink(&outside, d.join("top/dirlink")).unwrap();
        symlink(outside.join("keep"), d.join("top/sub/filelink")).unwrap();
        assert_eq!(remove(&d.join("top")), Ok(()));
        assert!(!d.join("top").exists());
        assert!(outside.join("keep").exists());
        // A symlink as the item itself: the link goes, the target stays.
        symlink(&outside, d.join("ln")).unwrap();
        assert_eq!(remove(&d.join("ln")), Ok(()));
        assert!(fs::symlink_metadata(d.join("ln")).is_err());
        assert!(outside.join("keep").exists());
    }

    #[test]
    fn remove_tree_skips_other_devices() {
        // Pretend `top` sits on another device: its subdirectories are then "mounts".
        let d = tmp("mount");
        fs::create_dir_all(d.join("top/sub")).unwrap();
        fs::write(d.join("top/sub/f"), "x").unwrap();
        fs::write(d.join("top/f"), "x").unwrap();
        let dev = fs::metadata(&d).unwrap().dev();
        let mut first = None;
        remove_tree(&d.join("top"), dev + 1, &mut first);
        assert_eq!(first.as_deref(), Some("mount_inside"));
        assert!(d.join("top/sub/f").exists());
        assert!(!d.join("top/f").exists());
    }

    #[test]
    fn remove_missing_is_error() {
        assert!(remove(&tmp("missing").join("nope")).is_err());
    }

    #[test]
    fn pgrep_probe() {
        assert!(!is_running("dv-no-such-process-xyz"));
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let seen = is_running("sleep");
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(seen);
    }

    /// Needs Finder + Automation permission; run by hand:
    /// `cargo test --lib trash_via_finder -- --ignored`
    #[test]
    #[ignore]
    fn trash_via_finder() {
        let d = tmp("trash");
        let outside = tmp("trash-out");
        fs::write(outside.join("keep"), "x").unwrap();
        fs::write(d.join("dv-trash-test.txt"), "x").unwrap();
        symlink(&outside, d.join("dv-trash-link")).unwrap();
        assert_eq!(trash(&d.join("dv-trash-test.txt")), Ok(()));
        assert_eq!(trash(&d.join("dv-trash-link")), Ok(()));
        assert!(!d.join("dv-trash-test.txt").exists());
        assert!(fs::symlink_metadata(d.join("dv-trash-link")).is_err());
        assert!(outside.join("keep").exists());
    }
}

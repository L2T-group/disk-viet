//! Filesystem side of deleting: Trash via Finder, permanent delete that never leaves the
//! volume or follows symlinks, and the running-process probe.
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
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

/// Every file some other process has open, as (command, path), from one `lsof` for the batch.
/// No answer is an error, never "nothing open": callers refuse to delete then.
/// shortcut: sees only the user's own processes (no root), enough for apps a user runs.
pub fn open_files() -> Result<Vec<(String, PathBuf)>, String> {
    // -b: never block on a stale network mount; +c 0: full command names.
    // A Finder-launched app has no LANG; in the C locale lsof escapes non-ASCII names (`\xe1`),
    // so Vietnamese paths would never match. Same env whatever launched us.
    let out = Command::new("/usr/sbin/lsof")
        .env_clear()
        .env("LC_ALL", "en_US.UTF-8")
        .args(["-nP", "-w", "-b", "+c", "0", "-F", "pcn"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| "in_use_unknown".to_string())?;
    // lsof exits 1 when some process could not be read; the rest of the list is still good.
    if out.stdout.is_empty() {
        return Err("in_use_unknown".into());
    }
    Ok(parse_lsof(
        &String::from_utf8_lossy(&out.stdout),
        std::process::id(),
    ))
}

/// `-F pcn` output: `p<pid>`, `c<command>`, then `f<fd>`/`n<path>` per open file.
/// shortcut: lsof prints a newline in a name as `\n`, so such paths never match (a missed
/// block, never a wrong delete); switch to libproc if that matters.
fn parse_lsof(out: &str, own_pid: u32) -> Vec<(String, PathBuf)> {
    let (mut cmd, mut own, mut v) = ("", false, Vec::new());
    for line in out.lines() {
        match line.split_at_checked(1) {
            Some(("p", pid)) => own = pid.parse() == Ok(own_pid),
            Some(("c", c)) => cmd = c.trim_end(), // `+c 0` still cuts long names mid-word
            Some(("n", n)) if !own && n.starts_with('/') => v.push((cmd.to_string(), n.into())),
            _ => {}
        }
    }
    v
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

    #[test]
    fn parse_lsof_skips_own_process_and_non_paths() {
        let out = "p1\ncCursor Helper\nfcwd\nn/Users/u\nf12\nn/Users/u/Library/Caches/x/db\n\
                   f13\nn127.0.0.1:80\np42\ncDiskViet\nf3\nn/Users/u/Library/Caches/y\n";
        assert_eq!(
            parse_lsof(out, 42),
            [
                ("Cursor Helper".to_string(), PathBuf::from("/Users/u")),
                (
                    "Cursor Helper".to_string(),
                    PathBuf::from("/Users/u/Library/Caches/x/db")
                ),
            ]
        );
    }

    #[test]
    fn open_files_sees_a_file_held_open() {
        // Non-ASCII on purpose: lsof escapes it unless the locale is UTF-8.
        let p = std::env::temp_dir().join(format!("dv-mở tệp-{}", std::process::id()));
        fs::write(&p, "x").unwrap();
        let p = p.canonicalize().unwrap();
        let mut child = Command::new("/usr/bin/tail")
            .arg("-f")
            .arg(&p)
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let open = open_files().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        assert!(open.iter().any(|(c, f)| c == "tail" && f == &p), "{p:?}");
        let _ = fs::remove_file(&p);
    }
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
        assert_eq!(
            trash_reason("timeout (-1712)"),
            "trash_failed: timeout (-1712)"
        );
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

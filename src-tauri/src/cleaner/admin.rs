//! One-shot privilege elevation for 🔑 rules (spec §4.3, §7). The app writes the guarded list
//! to a private file and asks macOS (one password prompt for the whole batch) to run this same
//! binary as root in helper mode. The helper trusts nothing: it re-runs the guard on every item.
use super::{DeleteItem, ItemResult, files, reason};
use crate::error::AppError;
use crate::guard::{self, Checked, DeleteMode};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What the app hands the helper. `home` is the user's canonical home: root's `$HOME` is not it.
#[derive(Debug, Serialize, Deserialize)]
pub struct HelperList {
    pub home: PathBuf,
    pub items: Vec<DeleteItem>,
}

type GuardFn<'a> = dyn Fn(&Path, DeleteMode, bool, &Path) -> Result<Checked, AppError> + 'a;

/// Helper-mode entry (`diskviet --helper-delete <list>`), running as root. Returns the exit
/// code: 0 once `<list>.out` is written.
pub fn helper_main(list_file: &str) -> i32 {
    match helper_io(Path::new(list_file), &guard::check) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("diskviet helper: {e}");
            1
        }
    }
}

fn helper_io(list: &Path, guard_fn: &GuardFn<'_>) -> Result<(), String> {
    // O_NOFOLLOW + fstat: the checks below are about the file actually read.
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(list)
        .map_err(|e| format!("list: {e}"))?;
    let m = f.metadata().map_err(|e| e.to_string())?;
    // Root acts on the list only if nobody but its owner could have written it.
    if !m.is_file() || m.mode() & 0o022 != 0 {
        return Err("unsafe_list_file".into());
    }
    let mut data = vec![];
    f.read_to_end(&mut data).map_err(|e| e.to_string())?;
    let l: HelperList = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    let json = serde_json::to_vec(&helper_run(&l, guard_fn)).map_err(|e| e.to_string())?;
    let out = write_private(&out_path(list), &json).map_err(|e| format!("out: {e}"))?;
    // Root created it: hand it to the list's owner so the app can read it.
    std::os::unix::fs::fchown(&out, Some(m.uid()), Some(m.gid())).map_err(|e| e.to_string())
}

/// Re-checks each item with the guard (always as a permanent delete, home from the list) and
/// deletes it only when it belongs to a `needs_admin` rule.
pub fn helper_run(list: &HelperList, guard_fn: &GuardFn<'_>) -> Vec<ItemResult> {
    list.items
        .iter()
        .map(|it| {
            let res = guard_fn(
                Path::new(&it.path),
                DeleteMode::Permanent,
                it.ack_irreversible,
                &list.home,
            )
            .map_err(reason)
            .and_then(|c| {
                if c.rule.is_some_and(|r| r.needs_admin) {
                    files::remove(&c.path)
                } else {
                    Err("not_admin_item".into())
                }
            });
            ItemResult {
                path: it.path.clone(),
                ok: res.is_ok(),
                error: res.err(),
            }
        })
        .collect()
}

/// Runs the helper as root for all `items` at once. The user cancelling the password prompt is
/// `AppError::Cancelled`; any other failure is `AppError::Invalid("admin_failed: …")`.
pub fn run_elevated(items: &[DeleteItem], home: &Path) -> Result<Vec<ItemResult>, AppError> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let dir = std::env::temp_dir().join(format!("diskviet-admin-{}-{nanos}", std::process::id()));
    // create (not create_all): fails if the name exists, so the dir is fresh, ours and 0700.
    fs::DirBuilder::new().mode(0o700).create(&dir)?;
    let res = elevate_in(&dir, items, home);
    let _ = fs::remove_dir_all(&dir);
    res
}

fn elevate_in(dir: &Path, items: &[DeleteItem], home: &Path) -> Result<Vec<ItemResult>, AppError> {
    let failed = |e: &dyn std::fmt::Display| AppError::Invalid(format!("admin_failed: {e}"));
    let list = dir.join("list.json");
    let body = HelperList {
        home: fs::canonicalize(home)?,
        items: items
            .iter()
            .map(|it| DeleteItem {
                path: it.path.clone(),
                mode: it.mode,
                ack_irreversible: it.ack_irreversible,
            })
            .collect(),
    };
    let json = serde_json::to_vec(&body).map_err(|e| failed(&e))?;
    write_private(&list, &json)?;
    let out = Command::new("osascript")
        .args(osascript_args(&std::env::current_exe()?, &list))
        .output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        if err.contains("(-128)") || err.contains("User canceled") {
            return Err(AppError::Cancelled);
        }
        return Err(failed(&err.trim()));
    }
    let data = fs::read(out_path(&list)).map_err(|e| failed(&e))?;
    let res: Vec<ItemResult> = serde_json::from_slice(&data).map_err(|e| failed(&e))?;
    if res.len() != items.len() {
        return Err(failed(&"result count"));
    }
    Ok(res)
}

/// Paths travel only as argv items; AppleScript's `quoted form` quotes them for the shell.
fn osascript_args(exe: &Path, list: &Path) -> Vec<OsString> {
    let script = [
        "on run argv",
        "do shell script (quoted form of item 1 of argv) & \" --helper-delete \" & (quoted form of item 2 of argv) with administrator privileges",
        "end run",
    ];
    let mut a: Vec<OsString> = script
        .iter()
        .flat_map(|l| ["-e".into(), l.into()])
        .collect();
    a.extend([exe.into(), list.into()]);
    a
}

/// New file, 0600, never through an existing file or symlink (O_EXCL).
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<File> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(data)?;
    Ok(f)
}

fn out_path(list: &Path) -> PathBuf {
    let mut s = list.as_os_str().to_owned();
    s.push(".out");
    s.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RULES;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("dv-admin-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p.canonicalize().unwrap()
    }

    fn item(p: &Path) -> DeleteItem {
        DeleteItem {
            path: p.to_string_lossy().into_owned(),
            mode: DeleteMode::Permanent,
            ack_irreversible: true,
        }
    }

    /// Writes `list` as the app would (0600) and returns its path.
    fn write_list(dir: &Path, list: &HelperList) -> PathBuf {
        let p = dir.join("list.json");
        let _ = fs::remove_file(&p);
        let _ = fs::remove_file(out_path(&p));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&p)
            .unwrap()
            .write_all(&serde_json::to_vec(list).unwrap())
            .unwrap();
        p
    }

    fn read_out(list: &Path) -> Vec<ItemResult> {
        serde_json::from_slice(&fs::read(out_path(list)).unwrap()).unwrap()
    }

    #[test]
    fn helper_reruns_guard() {
        let d = tmp("guard");
        let home = d.join("home");
        fs::create_dir_all(home.join(".ssh")).unwrap();
        fs::write(home.join(".ssh/id"), "k").unwrap();
        fs::create_dir_all(home.join("junk")).unwrap();
        fs::write(home.join("junk/f"), "x").unwrap();
        let list = write_list(
            &d,
            &HelperList {
                home: home.clone(),
                items: vec![item(&home.join(".ssh")), item(&home.join("junk/f"))],
            },
        );
        assert_eq!(helper_main(list.to_str().unwrap()), 0);
        let r = read_out(&list);
        let errs: Vec<_> = r.iter().map(|x| (x.ok, x.error.as_deref())).collect();
        // Guard says no to ~/.ssh; a deletable non-🔑 item is still refused by the helper.
        assert_eq!(
            errs,
            [
                (false, Some("system_safety")),
                (false, Some("not_admin_item"))
            ]
        );
        assert!(home.join(".ssh/id").exists() && home.join("junk/f").exists());
        assert_eq!(fs::metadata(out_path(&list)).unwrap().mode() & 0o777, 0o600);
    }

    #[test]
    fn helper_deletes_allowed() {
        // Every 🔑 rule is an absolute system path, so pretend the guard matched one.
        let sim = RULES.iter().find(|r| r.id == "simulator_caches").unwrap();
        let as_admin = |p: &Path, m, a, h: &Path| {
            let mut c = guard::check(p, m, a, h)?;
            c.rule = Some(sim);
            Ok(c)
        };
        let d = tmp("ok");
        let home = d.join("home");
        fs::create_dir_all(home.join("cache/sub")).unwrap();
        fs::write(home.join("cache/sub/f"), "x").unwrap();
        fs::create_dir_all(home.join(".ssh")).unwrap();
        let list = HelperList {
            home: home.clone(),
            items: vec![item(&home.join("cache")), item(&home.join(".ssh"))],
        };
        let r = helper_run(&list, &as_admin);
        let errs: Vec<_> = r.iter().map(|x| x.error.as_deref()).collect();
        assert_eq!(errs, [None, Some("system_safety")]); // the real guard still runs
        assert!(r[0].ok && !home.join("cache").exists());
        assert!(home.join(".ssh").exists());
    }

    #[test]
    fn helper_rejects_writable_list_file() {
        let d = tmp("writable");
        let home = d.join("home");
        fs::create_dir_all(home.join("junk")).unwrap();
        let list = write_list(
            &d,
            &HelperList {
                home,
                items: vec![],
            },
        );
        fs::set_permissions(&list, std::os::unix::fs::PermissionsExt::from_mode(0o622)).unwrap();
        assert_ne!(helper_main(list.to_str().unwrap()), 0);
        assert!(!out_path(&list).exists());
        // A symlink to a good list is refused too.
        fs::set_permissions(&list, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
        let ln = d.join("ln.json");
        std::os::unix::fs::symlink(&list, &ln).unwrap();
        assert_ne!(helper_main(ln.to_str().unwrap()), 0);
        assert!(!out_path(&ln).exists());
    }

    #[test]
    fn helper_refuses_existing_out_file() {
        // A planted `.out` (or a symlink there) is never written through.
        let d = tmp("out");
        let list = write_list(
            &d,
            &HelperList {
                home: d.clone(),
                items: vec![],
            },
        );
        let target = d.join("victim");
        fs::write(&target, "keep").unwrap();
        std::os::unix::fs::symlink(&target, out_path(&list)).unwrap();
        assert_ne!(helper_main(list.to_str().unwrap()), 0);
        assert_eq!(fs::read_to_string(&target).unwrap(), "keep");
    }

    #[test]
    fn osascript_argv_keeps_paths_out_of_the_script() {
        let exe = Path::new("/Apps/Disk Viet.app/Contents/MacOS/diskviet");
        let list = Path::new("/tmp/x'; rm -rf ~ #/list.json");
        let a = osascript_args(exe, list);
        let n = a.len();
        assert_eq!(a[n - 2], exe.as_os_str());
        assert_eq!(a[n - 1], list.as_os_str());
        let script: Vec<_> = a[..n - 2].iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(
            script,
            [
                "-e",
                "on run argv",
                "-e",
                "do shell script (quoted form of item 1 of argv) & \" --helper-delete \" & (quoted form of item 2 of argv) with administrator privileges",
                "-e",
                "end run",
            ]
        );
    }
}

//! The single deletion gate (spec §5). Every delete, from any caller, goes through `check`
//! (file deletes) or `check_tool` (tool cleaners). Independent of the UI.
use crate::error::AppError;
use crate::rules::{Cleaner, Rule, Safety, hit_path};
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirEntryExt, MetadataExt};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeleteMode {
    Trash,
    Permanent,
}

#[derive(Debug)]
pub struct Checked {
    pub path: PathBuf,
    pub rule: Option<&'static Rule>,
    pub safety: Safety,
    /// The rule comes from an ancestor, not a direct match on this path.
    pub inherited: bool,
}

/// Everything inside these is off limits (except `/usr/local`), whatever a rule says.
const SYSTEM_PREFIXES: &[&str] = &[
    "/System",
    "/usr",
    "/bin",
    "/sbin",
    "/private/var/db",
    "/Library/Apple",
];

fn deny(reason: &str) -> AppError {
    AppError::Guard(reason.into())
}

/// Gate for deleting files (Trash or Permanent). Errors are `AppError::Guard(reason)`.
pub fn check(
    path: &Path,
    mode: DeleteMode,
    ack_irreversible: bool,
    home: &Path,
) -> Result<Checked, AppError> {
    let c = gate(path, home, false)?;
    let rule = c.rule;
    if rule.is_some_and(|r| r.cleaner != Cleaner::Files) {
        return Err(deny("use_tool"));
    }
    if rule.is_some_and(|r| r.id == "trash") && mode != DeleteMode::Permanent {
        return Err(deny("trash_permanent_only"));
    }
    if matches!(c.safety, Safety::Important | Safety::Unknown)
        && mode == DeleteMode::Permanent
        && !ack_irreversible
    {
        return Err(deny("needs_ack"));
    }
    if mode == DeleteMode::Trash && rule.is_some_and(|r| r.needs_admin) {
        return Err(deny("admin_trash"));
    }
    Ok(c)
}

/// Gate for rules cleaned by an external tool (brew, simctl, docker): no file is deleted by
/// us, so the system-prefix ban is waived for those rules only.
pub fn check_tool(path: &Path, home: &Path) -> Result<Checked, AppError> {
    let c = gate(path, home, true)?;
    if !c.rule.is_some_and(|r| r.cleaner != Cleaner::Files) {
        return Err(deny("not_tool"));
    }
    if c.inherited {
        return Err(deny("tool_root_only"));
    }
    Ok(c)
}

/// Checks shared by both gates: home, canonical path, exact roots, system prefixes, System safety.
fn gate(path: &Path, home: &Path, tool: bool) -> Result<Checked, AppError> {
    if !home.is_absolute() {
        return Err(deny("no_home"));
    }
    let home = fs::canonicalize(home).map_err(|_| deny("no_home"))?;
    let path = resolve(path)?;
    let roots = [
        home.clone(),
        home.join("Library"),
        home.join("Documents"),
        home.join("Desktop"),
        home.join("Pictures"),
        canon("/System/Volumes/Data"),
        canon("/usr/local"),
    ];
    // Beyond spec: top-level dirs and links (/private/var, /etc, /opt, ...) break the OS whole.
    let top = ["/", "/private", "/Volumes"].map(canon);
    if roots.contains(&path)
        || home.starts_with(&path) // any ancestor of home, wherever home lives
        || path.parent().is_some_and(|p| top.iter().any(|t| t == p))
    {
        return Err(deny("forbidden_root"));
    }
    let hit = hit_path(&path, &home);
    let (rule, inherited) = (hit.map(|h| h.rule), hit.is_some_and(|h| h.inherited));
    let safety = rule.map_or(Safety::Unknown, |r| r.safety);
    let is_tool = rule.is_some_and(|r| r.cleaner != Cleaner::Files);
    if !(tool && is_tool) && in_system(&path) {
        return Err(deny("system_path"));
    }
    if safety == Safety::System {
        return Err(deny("system_safety"));
    }
    Ok(Checked {
        path,
        rule,
        safety,
        inherited,
    })
}

/// Canonical parent + the on-disk name of the last component, which is never followed
/// (a symlink is deleted, not its target).
fn resolve(path: &Path) -> Result<PathBuf, AppError> {
    if !path.is_absolute() {
        return Err(deny("relative"));
    }
    let raw = path.as_os_str().as_bytes();
    let end = raw.iter().rposition(|&b| b != b'/').map_or(0, |i| i + 1);
    if end == 0 {
        return Err(deny("forbidden_root"));
    }
    // `Path` silently drops a trailing `.`; refuse it and `..` instead of guessing.
    let last = raw[..end].rsplit(|&b| b == b'/').next();
    if matches!(last, Some(b".") | Some(b"..")) {
        return Err(deny("invalid_path"));
    }
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(deny("invalid_path"));
    };
    let parent = fs::canonicalize(parent).map_err(|_| deny("not_found"))?;
    let meta = fs::symlink_metadata(parent.join(name)).map_err(|_| deny("not_found"))?;
    let pdev = fs::metadata(&parent).map_err(|_| deny("not_found"))?.dev();
    if meta.dev() != pdev {
        return Err(deny("mount_root"));
    }
    Ok(parent.join(true_name(&parent, name, meta.ino())?))
}

/// APFS is case- and normalization-insensitive: `~/DOCUMENTS` opens `~/Documents`. Use the
/// name stored in the directory so path comparisons and rule patterns see the real path.
fn true_name(parent: &Path, name: &OsStr, ino: u64) -> Result<OsString, AppError> {
    let mut found = None;
    let entries = fs::read_dir(parent).map_err(|_| deny("not_found"))?;
    for e in entries.flatten() {
        if e.ino() != ino {
            continue;
        }
        if e.file_name() == name {
            return Ok(name.to_owned());
        }
        found.get_or_insert(e.file_name()); // hard links: prefer the exact name if present
    }
    // readdir shows the covered inode when a volume or firmlink sits on top: a mount root.
    found.ok_or_else(|| deny("mount_root"))
}

fn canon(p: &str) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.into())
}

fn in_system(path: &Path) -> bool {
    let usr_local = path.starts_with(canon("/usr/local"));
    SYSTEM_PREFIXES
        .iter()
        .any(|p| path.starts_with(canon(p)) && !(*p == "/usr" && usr_local))
}

#[cfg(test)]
mod tests {
    use super::DeleteMode::{Permanent, Trash};
    use super::*;
    use std::fs;

    fn tmp_home(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("dv-guard-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        for d in [
            "Library/Caches/Homebrew",
            "Documents",
            "Desktop",
            "Pictures",
            ".ssh",
            ".Trash",
        ] {
            fs::create_dir_all(p.join(d)).unwrap();
        }
        fs::write(p.join("Documents/a.txt"), "x").unwrap();
        fs::write(p.join(".Trash/x"), "x").unwrap();
        p.canonicalize().unwrap()
    }

    fn reason(r: Result<Checked, AppError>) -> String {
        match r {
            Ok(c) => format!("ok:{}", c.path.display()),
            Err(AppError::Guard(s)) => s,
            Err(e) => format!("other:{e}"),
        }
    }

    /// Most permissive call: anything it rejects is rejected in every mode.
    fn perm(p: &Path, home: &Path) -> String {
        reason(check(p, Permanent, true, home))
    }

    #[test]
    fn rejects_exact_roots() {
        let h = tmp_home("roots");
        for p in [
            h.clone(),
            h.join("Library"),
            h.join("Documents"),
            h.join("Desktop"),
            h.join("Pictures"),
            h.join("Documents/"),
            h.join("DOCUMENTS"), // APFS is case-insensitive
            PathBuf::from("/"),
            PathBuf::from("//"),
        ] {
            assert_eq!(perm(&p, &h), "forbidden_root", "{p:?}");
        }
        for e in fs::read_dir("/Volumes").unwrap().flatten() {
            assert_ne!(&perm(&e.path(), &h)[..3], "ok:", "{:?}", e.path());
        }
        let a = h.join("Documents/a.txt");
        assert_eq!(perm(&a, &h), format!("ok:{}", a.display()));
    }

    #[test]
    fn rejects_system_prefixes() {
        let h = tmp_home("sys");
        for p in [
            "/System/Library",
            "/usr/bin/true",
            "/SYSTEM/Library",
            "/usr/BIN",
            "/bin/ls",
        ] {
            assert_eq!(perm(Path::new(p), &h), "system_path", "{p}");
        }
        assert_eq!(perm(Path::new("/var/db"), &h), "system_path"); // /var -> /private/var
        assert!(in_system(Path::new("/usr/bin")));
        assert!(in_system(Path::new("/usr/localx")));
        assert!(in_system(Path::new("/private/var/db/x")));
        assert!(in_system(Path::new("/Library/Apple/x")));
        assert!(!in_system(Path::new("/usr/local/x")));
        assert!(!in_system(Path::new("/Systemx")));
        assert!(!in_system(Path::new("/Library/Applex")));
    }

    #[test]
    fn dotdot_cannot_escape() {
        let h = tmp_home("dotdot");
        let p = h.join("Library/Caches/../../Documents");
        assert_eq!(perm(&p, &h), "forbidden_root");
        assert_eq!(perm(&h.join("Documents/."), &h), "invalid_path");
        assert_eq!(perm(&h.join("Documents/a.txt/.."), &h), "invalid_path");
        assert_eq!(perm(Path::new("/.."), &h), "invalid_path");
    }

    #[test]
    fn symlink_deletes_link_not_target() {
        let h = tmp_home("link");
        std::os::unix::fs::symlink("/System", h.join("link")).unwrap();
        std::os::unix::fs::symlink(h.join("Documents"), h.join("doclink")).unwrap();
        for n in ["link", "doclink"] {
            let c = check(&h.join(n), Trash, false, &h).unwrap();
            assert_eq!(c.path, h.join(n));
        }
        // Going *through* a link still lands on the target's rules.
        std::os::unix::fs::symlink(&h, h.join("homelink")).unwrap();
        assert_eq!(perm(&h.join("homelink/Documents"), &h), "forbidden_root");
    }

    #[test]
    fn system_safety_rejected() {
        let h = tmp_home("ssh");
        assert_eq!(perm(&h.join(".ssh"), &h), "system_safety");
        assert_eq!(perm(&h.join(".SSH"), &h), "system_safety");
    }

    #[test]
    fn important_permanent_needs_ack() {
        let h = tmp_home("ack");
        let a = h.join("Documents/a.txt");
        assert_eq!(reason(check(&a, Permanent, false, &h)), "needs_ack");
        assert!(check(&a, Permanent, true, &h).is_ok());
        assert!(check(&a, Trash, false, &h).is_ok());
        // Unknown is treated like Important.
        fs::write(h.join("loose.bin"), "x").unwrap();
        let u = h.join("loose.bin");
        assert_eq!(check(&u, Trash, false, &h).unwrap().safety, Safety::Unknown);
        assert_eq!(reason(check(&u, Permanent, false, &h)), "needs_ack");
    }

    #[test]
    fn admin_rule_trash_rejected() {
        let h = tmp_home("admin");
        let p = Path::new("/Library/Developer/CoreSimulator/Caches"); // simulator_caches: admin
        if !p.exists() {
            return;
        }
        assert_eq!(reason(check(p, Trash, true, &h)), "admin_trash");
        assert!(check(p, Permanent, false, &h).is_ok());
    }

    #[test]
    fn tool_rules_only_via_check_tool() {
        let h = tmp_home("tool");
        let brew = h.join("Library/Caches/Homebrew");
        assert_eq!(perm(&brew, &h), "use_tool");
        assert_eq!(check_tool(&brew, &h).unwrap().path, brew);
        // Only the rule-matched folder runs the tool, never a child of it.
        fs::create_dir_all(brew.join("downloads")).unwrap();
        assert_eq!(
            reason(check_tool(&brew.join("downloads"), &h)),
            "tool_root_only"
        );
        assert_eq!(perm(&brew.join("downloads"), &h), "use_tool");
        assert_eq!(
            reason(check_tool(&h.join("Documents/a.txt"), &h)),
            "not_tool"
        );
        assert_eq!(
            reason(check_tool(&h.join("Documents"), &h)),
            "forbidden_root"
        );
        assert_eq!(
            reason(check_tool(Path::new("/System/Library"), &h)),
            "system_path"
        );
    }

    #[test]
    fn trash_permanent_only() {
        let h = tmp_home("trash");
        let x = h.join(".Trash/x");
        assert_eq!(reason(check(&x, Trash, true, &h)), "trash_permanent_only");
        assert!(check(&x, Permanent, false, &h).is_ok());
    }

    #[test]
    fn bad_home_refuses_everything() {
        let h = tmp_home("nohome");
        let a = h.join("Documents/a.txt");
        for home in ["", "rel/home", "/no/such/home/dv"] {
            assert_eq!(perm(&a, Path::new(home)), "no_home", "{home:?}");
            assert_eq!(reason(check_tool(&a, Path::new(home))), "no_home");
        }
    }

    #[test]
    fn mount_roots_rejected() {
        let h = tmp_home("mount");
        assert_eq!(perm(Path::new("/System/Volumes/Data"), &h), "mount_root");
        assert_eq!(perm(Path::new("/"), &h), "forbidden_root");
        assert_eq!(perm(Path::new("/dev"), &h), "mount_root"); // devfs
        for p in [
            "/Users",
            "/Applications",
            "/Library",
            "/private/var",
            "/etc",
            "/tmp",
            "/opt",
            "/usr/local",
        ] {
            // Firmlinked dirs (/Users, /Applications, ...) stop at mount_root first.
            let r = perm(Path::new(p), &h);
            assert!(r == "forbidden_root" || r == "mount_root", "{p}: {r}");
        }
    }

    #[test]
    fn home_ancestors_rejected() {
        // Relocated home (e.g. /Volumes/Data/Users/tu): its parents classify Unknown.
        let base = tmp_home("anc");
        fs::create_dir_all(base.join("a/b/home/Documents")).unwrap();
        let h = base.join("a/b/home");
        for p in [base.join("a/b"), base.join("a")] {
            assert_eq!(perm(&p, &h), "forbidden_root", "{p:?}");
            assert_eq!(reason(check(&p, Trash, false, &h)), "forbidden_root");
            assert_eq!(reason(check_tool(&p, &h)), "forbidden_root");
        }
    }

    #[test]
    fn bad_input() {
        let h = tmp_home("input");
        assert_eq!(perm(Path::new("Documents/a.txt"), &h), "relative");
        assert_eq!(perm(&h.join("nope"), &h), "not_found");
        assert_eq!(perm(&h.join("nope/deeper"), &h), "not_found");
    }
}

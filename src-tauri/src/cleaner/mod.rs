//! Deleting files: guard → process check → delete → tree update → history (spec §4.3, §5).
//! Tool rules (brew, simctl, docker) run their tool instead, as the user (`tools`).
pub mod admin;
pub mod files;
pub mod tools;

use crate::disk;
use crate::error::AppError;
use crate::guard::{self, Checked, DeleteMode};
use crate::history::{self, HistoryEntry};
use crate::rules::{self, Cleaner, Rule};
use crate::scanner::Tree;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::{PoisonError, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteItem {
    pub path: String,
    pub mode: DeleteMode,
    pub ack_irreversible: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub path: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteReport {
    pub free_before: u64,
    pub free_after: u64,
    pub expected: u64,
    pub results: Vec<ItemResult>,
}

/// Runs a batch of 🔑 items as root (`admin::run_elevated`; tests inject a fake).
pub type Elevate<'a> = dyn Fn(&[DeleteItem], &Path) -> Result<Vec<ItemResult>, AppError> + 'a;

/// Runs a tool cleaner on its rule-matched folder (`tools::run_tool`; tests inject a fake).
pub type ToolRun<'a> = dyn Fn(Cleaner, &Path) -> Result<(), String> + 'a;

/// Guard rejections are reported by their reason code.
fn reason(e: AppError) -> String {
    match e {
        AppError::Guard(r) => r,
        e => e.to_string(),
    }
}

/// Deletes `items` in order. `tree` may hold no scan; it is locked only around lookups and
/// removals, never across filesystem work. `running` answers "is this process running?".
/// All 🔑 items go to `elevate` in one call, so macOS asks for the password once.
pub fn run(
    items: &[DeleteItem],
    tree: &RwLock<Option<Tree>>,
    home: &Path,
    history_dir: &Path,
    running: &dyn Fn(&str) -> bool,
    elevate: &Elevate<'_>,
    tool: &ToolRun<'_>,
) -> DeleteReport {
    let expanded = empty_containers(items, home);
    let items: Vec<DeleteItem> = expanded.iter().map(|(it, _)| it.clone()).collect();
    let items = &items[..];
    let checked: Vec<Result<Checked, String>> = expanded
        .into_iter()
        .map(|(it, pre)| {
            if let Some(e) = pre {
                return Err(e);
            }
            let p = Path::new(&it.path);
            match guard::check_tool(p, home) {
                Err(AppError::Guard(r)) if r == "not_tool" => {
                    guard::check(p, it.mode, it.ack_irreversible, home)
                }
                c => c,
            }
            .map_err(reason)
        })
        .collect();
    let tool_of = |i: usize| {
        let c = checked[i].as_ref().ok()?.rule?.cleaner;
        (c != Cleaner::Files).then_some(c)
    };
    // Rule prefixes are compared with canonical item paths.
    let canon_home = fs::canonicalize(home).unwrap_or_else(|_| home.into());
    // Drop items inside (or equal to an earlier) other item: deleting the outer one covers them.
    // shortcut: the outer item's mode wins, so a Trash child under a Permanent parent is deleted
    // for good; the confirm dialog must show the nesting.
    let covered = |i: usize, c: &Checked| {
        checked.iter().enumerate().any(|(j, o)| {
            o.as_ref()
                .is_ok_and(|o| j != i && c.path.starts_with(&o.path) && (c.path != o.path || j < i))
        })
    };
    let (root, sizes): (_, Vec<u64>) = {
        let g = tree.read().unwrap_or_else(PoisonError::into_inner);
        let size = |c: &Result<Checked, String>| {
            let (Some(t), Ok(c)) = (g.as_ref(), c) else {
                return 0;
            };
            t.find(&c.path).map_or(0, |id| t.nodes[id as usize].size)
        };
        let root = g
            .as_ref()
            .map_or_else(|| "/".into(), |t| t.root_path.clone());
        (root, checked.iter().map(size).collect())
    };
    let free = || disk::free_bytes(&root).unwrap_or(0);
    let free_before = free();
    let mut expected = 0;
    let mut outcome: Vec<Option<Result<(), String>>> = vec![None; items.len()];
    // Records an attempted item: expected bytes, tree, history, outcome.
    let mut finish = |i: usize, path: String, res: Result<(), String>| {
        let it = &items[i];
        let tool = tool_of(i);
        // A tool decides what goes (prune never shrinks Docker.raw): no expected bytes, no tree
        // change until rescan; the result dialog's statfs before/after is the real number.
        if res.is_ok() && tool.is_none() {
            // Trash frees nothing until it is emptied.
            if it.mode == DeleteMode::Permanent {
                expected += sizes[i];
            }
            let mut g = tree.write().unwrap_or_else(PoisonError::into_inner);
            if let (Some(t), Ok(c)) = (g.as_mut(), &checked[i])
                && let Some(id) = t.find(&c.path)
            {
                t.remove(id);
            }
        }
        let entry = HistoryEntry {
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64),
            path,
            size: if tool.is_some() { 0 } else { sizes[i] },
            mode: it.mode,
            ok: res.is_ok(),
            error: res.as_ref().err().cloned(),
            tool: tool.and_then(tools::tool_name).map(String::from),
        };
        if let Err(e) = history::append_to(history_dir, &entry) {
            eprintln!("history: {e}");
        }
        outcome[i] = Some(res);
    };
    let mut admin = Vec::new();
    for (i, (it, c)) in items.iter().zip(&checked).enumerate() {
        match c {
            Err(e) => finish(i, it.path.clone(), Err(e.clone())),
            Ok(c) if covered(i, c) => {}
            // Tools always run as the user, never through the admin helper.
            Ok(c) if let Some(cl) = tool_of(i) => {
                finish(i, c.path.to_string_lossy().into_owned(), tool(cl, &c.path));
            }
            Ok(c) => {
                let res = precheck(c, it.mode, &canon_home, running);
                if res.is_ok() && c.rule.is_some_and(|r| r.needs_admin) {
                    admin.push(i); // guard already refused Trash for these
                    continue;
                }
                let res = res.and_then(|()| match it.mode {
                    DeleteMode::Trash => files::trash(&c.path),
                    DeleteMode::Permanent => files::remove(&c.path),
                });
                finish(i, c.path.to_string_lossy().into_owned(), res);
            }
        }
    }
    if !admin.is_empty() {
        let batch: Vec<DeleteItem> = admin
            .iter()
            .filter_map(|&i| checked[i].as_ref().ok().map(|c| (i, c)))
            .map(|(i, c)| DeleteItem {
                path: c.path.to_string_lossy().into_owned(),
                mode: DeleteMode::Permanent,
                ack_irreversible: items[i].ack_irreversible,
            })
            .collect();
        let done = elevate(&batch, &canon_home);
        for (k, &i) in admin.iter().enumerate() {
            let res = match &done {
                Ok(v) => v
                    .get(k)
                    .map_or(Err("admin_failed".into()), |r| match &r.error {
                        _ if r.ok => Ok(()),
                        Some(e) => Err(e.clone()),
                        None => Err("admin_failed".into()),
                    }),
                Err(AppError::Cancelled) => Err("admin_cancelled".into()),
                Err(e) => {
                    eprintln!("elevate: {e}");
                    Err("admin_failed".into())
                }
            };
            finish(i, batch[k].path.clone(), res);
        }
    }
    let results = items
        .iter()
        .zip(&checked)
        .enumerate()
        .map(|(i, (it, c))| {
            let res = match (&outcome[i], c) {
                (Some(r), _) => r.clone(),
                // Covered: the outermost item holding it decides.
                (None, Ok(c)) => {
                    let outer = (0..items.len()).find(|&j| {
                        outcome[j].is_some()
                            && checked[j]
                                .as_ref()
                                .is_ok_and(|o| c.path.starts_with(&o.path))
                    });
                    match outer {
                        Some(j) if outcome[j] == Some(Ok(())) => Ok(()),
                        Some(j) => Err(format!(
                            "covered_by:{}",
                            checked[j].as_ref().unwrap().path.display()
                        )),
                        None => Err("covered_by:?".into()), // unreachable: coverage is transitive
                    }
                }
                (None, Err(e)) => Err(e.clone()), // unreachable: errors always get an outcome
            };
            let path = match c {
                Ok(c) => c.path.to_string_lossy().into_owned(),
                Err(_) => it.path.clone(),
            };
            ItemResult {
                path,
                ok: res.is_ok(),
                error: res.err(),
            }
        })
        .collect();
    DeleteReport {
        free_before,
        free_after: free(),
        expected,
        results,
    }
}

/// Rules whose root the user means to empty, not remove: ~/.Trash and ~/Downloads.
const EMPTY_ONLY: &[&str] = &["trash", "downloads"];

/// Replaces an item that is the root of an `EMPTY_ONLY` rule by its children (same mode and
/// consent), each of which then goes through the guard like any item. The folder stays.
/// An unreadable folder becomes one item with that error, so the folder itself is never deleted.
fn empty_containers(items: &[DeleteItem], home: &Path) -> Vec<(DeleteItem, Option<String>)> {
    let mut out = Vec::new();
    for it in items {
        let c = guard::check(Path::new(&it.path), it.mode, it.ack_irreversible, home).ok();
        let Some(c) =
            c.filter(|c| !c.inherited && c.rule.is_some_and(|r| EMPTY_ONLY.contains(&r.id)))
        else {
            out.push((it.clone(), None));
            continue;
        };
        let entries = match fs::read_dir(&c.path) {
            Ok(rd) => rd.collect::<Result<Vec<_>, _>>(),
            Err(e) => Err(e),
        };
        match entries {
            Err(e) => out.push((it.clone(), Some(e.to_string()))),
            Ok(es) => out.extend(
                es.iter()
                    // Finder's localized folder name lives in `.localized`.
                    .filter(|e| e.file_name() != ".localized")
                    .map(|e| {
                        let path = e.path().to_string_lossy().into_owned();
                        (DeleteItem { path, ..it.clone() }, None)
                    }),
            ),
        }
    }
    out
}

/// Checks left after the guard: a Trash on the item's volume, no related app running.
fn precheck(
    c: &Checked,
    mode: DeleteMode,
    home: &Path,
    running: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    if mode == DeleteMode::Trash && !trash_on_volume(&c.path, home) {
        return Err("no_trash_on_volume".into());
    }
    // The item's own rule, plus every rule that could match inside the item (a parent folder
    // such as ~/Library/Caches holds the Telegram cache).
    let inside = |r: &Rule| r.patterns.iter().any(|p| could_hold(p, &c.path, home));
    let procs = rules::RULES
        .iter()
        .filter(|r| inside(r))
        .chain(c.rule)
        .flat_map(|r| r.processes);
    if let Some(p) = procs.into_iter().find(|p| running(p)) {
        return Err(format!("quit:{p}"));
    }
    Ok(())
}

/// Could `pattern` match `item` or something inside it? True when `item` matches the
/// pattern cut after any segment (`~/a/*.tg` for `~/a/*.tg/**/media`); a trailing `**`
/// covers everything below.
fn could_hold(pattern: &str, item: &Path, home: &Path) -> bool {
    let segs: Vec<&str> = pattern.split('/').collect();
    (1..=segs.len()).any(|k| rules::matches(&segs[..k].join("/"), item, home))
}

/// Trash only on the home volume or a volume with `/.Trashes`; elsewhere (SMB, exFAT) Finder
/// may delete for good. Unknown counts as no.
fn trash_on_volume(path: &Path, home: &Path) -> bool {
    let dev = |p: &Path| fs::metadata(p).map(|m| m.dev()).ok();
    let (Some(item), Some(h)) = (fs::symlink_metadata(path).ok().map(|m| m.dev()), dev(home))
    else {
        return false;
    };
    // The parent: statfs follows a symlink item to its target's volume.
    let trashes = path
        .parent()
        .and_then(|p| disk::mount_point(p).ok())
        .is_some_and(|m| fs::symlink_metadata(m.join(".Trashes")).is_ok_and(|t| t.is_dir()));
    trash_allowed(item, h, trashes)
}

fn trash_allowed(item_dev: u64, home_dev: u64, volume_has_trashes: bool) -> bool {
    item_dev == home_dev || volume_has_trashes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules;
    use crate::scanner::{Progress, scan};
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    const MB: usize = 1 << 20;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("dv-clean-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p.canonicalize().unwrap()
    }

    fn write(path: &Path, len: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::File::create(path)
            .unwrap()
            .write_all(&vec![7u8; len])
            .unwrap();
    }

    /// Fake home, scanned and classified. Returns (home, history dir, tree).
    fn setup(name: &str) -> (PathBuf, PathBuf, RwLock<Option<Tree>>) {
        let h = tmp(name);
        write(&h.join("Library/Caches/com.fake.app/big"), 2 * MB);
        write(
            &h.join("Library/Caches/ru.keepcoder.Telegram/media"),
            2 * MB,
        );
        write(&h.join("junk/a/b/f"), 2 * MB);
        write(&h.join("junk/a/g"), 2 * MB);
        write(&h.join("junk/one"), 2 * MB);
        write(&h.join("junk/two"), 2 * MB);
        write(&h.join(".ssh/id"), 10);
        write(&h.join("Library/Caches/Homebrew/downloads/x"), 2 * MB);
        let mut t = scan(&h, &AtomicBool::new(false), &Progress::default()).unwrap();
        rules::classify(&mut t, &h);
        (h, tmp(&format!("{name}-hist")), RwLock::new(Some(t)))
    }

    fn item(p: &Path, mode: DeleteMode) -> DeleteItem {
        DeleteItem {
            path: p.to_string_lossy().into_owned(),
            mode,
            ack_irreversible: true,
        }
    }

    fn never(_: &str) -> bool {
        false
    }

    fn no_admin(_: &[DeleteItem], _: &Path) -> Result<Vec<ItemResult>, AppError> {
        panic!("unexpected elevation")
    }

    fn no_tool(_: rules::Cleaner, _: &Path) -> Result<(), String> {
        panic!("unexpected tool run")
    }

    fn size_of(t: &RwLock<Option<Tree>>, p: &Path) -> Option<u64> {
        let g = t.read().unwrap();
        let t = g.as_ref().unwrap();
        t.find(p).map(|id| t.nodes[id as usize].size)
    }

    fn errors(r: &DeleteReport) -> Vec<Option<&str>> {
        r.results.iter().map(|x| x.error.as_deref()).collect()
    }

    #[test]
    fn permanent_removes_dir_and_updates_tree() {
        let (h, hist, t) = setup("perm");
        let p = h.join("Library/Caches/com.fake.app");
        let size = size_of(&t, &p).unwrap();
        let root = size_of(&t, &h).unwrap();
        assert!(size >= 2 * MB as u64);
        let r = run(
            &[item(&p, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [None]);
        assert!(r.results[0].ok);
        assert_eq!(r.results[0].path, p.to_string_lossy());
        assert_eq!(r.expected, size);
        assert!(!p.exists());
        assert_eq!(size_of(&t, &p), None);
        assert_eq!(size_of(&t, &h), Some(root - size));
        assert!(r.free_before > 0 && r.free_after > 0);
    }

    #[test]
    fn missing_item_reports_error_others_continue() {
        let (h, hist, t) = setup("missing");
        let (one, two) = (h.join("junk/one"), h.join("junk/two"));
        fs::remove_file(&one).unwrap();
        let items = [
            item(&one, DeleteMode::Permanent),
            item(&two, DeleteMode::Permanent),
        ];
        let r = run(&items, &t, &h, &hist, &never, &no_admin, &no_tool);
        assert_eq!(errors(&r), [Some("not_found"), None]);
        assert!(!r.results[0].ok && r.results[1].ok);
        assert!(!two.exists());
    }

    #[test]
    fn nested_items_deduped() {
        let (h, hist, t) = setup("nested");
        let (a, b) = (h.join("junk/a"), h.join("junk/a/b"));
        let size = size_of(&t, &a).unwrap();
        // Child first, parent later, plus an exact duplicate: all reported, only the parent run.
        let items = [
            item(&b, DeleteMode::Permanent),
            item(&a, DeleteMode::Permanent),
            item(&a, DeleteMode::Permanent),
        ];
        let r = run(&items, &t, &h, &hist, &never, &no_admin, &no_tool);
        let paths: Vec<_> = r.results.iter().map(|x| x.path.clone()).collect();
        let (a_s, b_s) = (a.to_string_lossy(), b.to_string_lossy());
        assert_eq!(paths, [b_s.clone(), a_s.clone(), a_s]);
        assert!(r.results.iter().all(|x| x.ok && x.error.is_none()));
        assert_eq!(r.expected, size);
        assert!(!a.exists());
        // History only holds what was actually attempted: the parent, once.
        assert_eq!(history::list_from(&hist).unwrap().len(), 1);
    }

    #[test]
    fn parent_dir_checks_processes_of_rules_inside() {
        let (h, hist, t) = setup("parent");
        let caches = h.join("Library/Caches");
        let tg = h.join("Library/Caches/ru.keepcoder.Telegram");
        let running = |n: &str| n == "Telegram";
        // The Telegram child is dropped by dedupe; it reports the parent's failure.
        let items = [
            item(&caches, DeleteMode::Permanent),
            item(&tg, DeleteMode::Permanent),
        ];
        let r = run(&items, &t, &h, &hist, &running, &no_admin, &no_tool);
        assert_eq!(
            errors(&r),
            [
                Some("quit:Telegram"),
                Some(format!("covered_by:{}", caches.display()).as_str())
            ]
        );
        assert!(!r.results[1].ok);
        assert!(tg.join("media").exists() && caches.join("com.fake.app/big").exists());
        assert_eq!(r.expected, 0);
        // Group Containers holds a Telegram pattern below a glob segment.
        let gc = h.join("Library/Group Containers");
        fs::create_dir_all(&gc).unwrap();
        let r = run(
            &[item(&gc, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &running,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [Some("quit:Telegram")]);
        assert!(gc.exists());
    }

    #[test]
    fn dir_where_a_rule_could_match_checks_processes() {
        let (h, hist, t) = setup("glob");
        // Pattern: ~/Library/Group Containers/*.ru.keepcoder.Telegram/**/postbox/media
        let gc = h.join("Library/Group Containers/6N38VWS5BX.ru.keepcoder.Telegram");
        write(&gc.join("account/x"), 10);
        let running = |n: &str| n == "Telegram";
        for p in [gc.clone(), gc.join("account")] {
            let r = run(
                &[item(&p, DeleteMode::Permanent)],
                &t,
                &h,
                &hist,
                &running,
                &no_admin,
                &no_tool,
            );
            assert_eq!(errors(&r), [Some("quit:Telegram")], "{p:?}");
        }
        assert!(gc.join("account/x").exists());
        // A sibling container that no Telegram pattern can reach is not blocked.
        let other = h.join("Library/Group Containers/group.com.other");
        write(&other.join("x"), 10);
        let r = run(
            &[item(&other, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &running,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [None]);
    }

    #[test]
    fn could_hold_follows_globs() {
        let h = Path::new("/h");
        let p = "~/a/*.tg/**/postbox/media";
        for (item, want) in [
            ("/h/a", true),              // literal ancestor
            ("/h/a/x.tg", true),         // matches a glob segment
            ("/h/a/x.tg/deep/er", true), // under `**`
            ("/h/a/x.tg/postbox/media", true),
            ("/h/a/other", false),
            ("/h/b", false),
        ] {
            assert_eq!(could_hold(p, Path::new(item), h), want, "{item}");
        }
        assert!(could_hold("/Library/Updates", Path::new("/Library"), h));
        assert!(!could_hold("/Library/Updates", Path::new("/Users"), h));
    }

    #[test]
    fn container_rules_empty_the_folder_not_remove_it() {
        let (h, hist, t) = setup("container");
        let (tr, dl) = (h.join(".Trash"), h.join("Downloads"));
        write(&tr.join("old/x"), 10);
        write(&tr.join("y"), 10);
        write(&dl.join("setup.dmg"), 10);
        write(&dl.join(".localized"), 0);
        let items = [
            item(&tr, DeleteMode::Permanent),
            item(&dl, DeleteMode::Permanent),
        ];
        let r = run(&items, &t, &h, &hist, &never, &no_admin, &no_tool);
        assert_eq!(errors(&r), [None, None, None]);
        let mut paths: Vec<_> = r.results.iter().map(|x| x.path.clone()).collect();
        paths.sort();
        let want = [tr.join("old"), tr.join("y"), dl.join("setup.dmg")];
        assert_eq!(paths, want.map(|p| p.to_string_lossy().into_owned()));
        assert!(tr.is_dir() && dl.is_dir());
        assert_eq!(fs::read_dir(&tr).unwrap().count(), 0);
        assert!(dl.join(".localized").exists() && !dl.join("setup.dmg").exists());
        // ~/.Trash refuses Trash mode: the folder is reported, not expanded.
        write(&tr.join("z"), 10);
        let r = run(
            &[item(&tr, DeleteMode::Trash)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [Some("trash_permanent_only")]);
        assert!(tr.join("z").exists());
    }

    #[test]
    fn trash_needs_a_trash_on_the_volume() {
        assert!(trash_allowed(1, 1, false));
        assert!(trash_allowed(2, 1, true));
        assert!(!trash_allowed(2, 1, false));
    }

    #[test]
    fn process_names_fit_pgrep() {
        // macOS `pgrep -x` matches p_comm, which keeps only the first 16 bytes of the name.
        for r in rules::RULES {
            for p in r.processes {
                assert!(p.len() <= 15, "{}: {p}", r.id);
            }
        }
    }

    #[test]
    fn guard_rejection_is_item_error() {
        let (h, hist, t) = setup("guard");
        let ssh = h.join(".ssh");
        let r = run(
            &[item(&ssh, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [Some("system_safety")]);
        assert!(!r.results[0].ok);
        assert!(ssh.join("id").exists());
        assert_eq!(r.expected, 0);
    }

    #[test]
    fn history_appended() {
        let (h, hist, t) = setup("hist");
        let (one, ssh) = (h.join("junk/one"), h.join(".ssh"));
        let items = [
            item(&one, DeleteMode::Permanent),
            item(&ssh, DeleteMode::Permanent),
        ];
        run(&items, &t, &h, &hist, &never, &no_admin, &no_tool);
        let l = history::list_from(&hist).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].path, ssh.to_string_lossy()); // newest first
        assert!(!l[0].ok && l[0].error.as_deref() == Some("system_safety"));
        assert!(l[1].ok && l[1].error.is_none() && l[1].size >= 2 * MB as u64);
        assert_eq!(l[1].mode, DeleteMode::Permanent);
    }

    #[test]
    fn running_process_blocks_item() {
        let (h, hist, t) = setup("proc");
        let p = h.join("Library/Caches/ru.keepcoder.Telegram");
        let tg = |n: &str| n == "Telegram";
        let r = run(
            &[item(&p, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &tg,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [Some("quit:Telegram")]);
        assert_eq!(r.expected, 0); // nothing freed
        assert!(p.join("media").exists());
        assert!(size_of(&t, &p).is_some());
        let r = run(
            &[item(&p, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [None]);
        assert!(!p.exists());
    }

    #[test]
    fn deletes_canonical_path_and_link_not_target() {
        let (h, hist, t) = setup("canon");
        let outside = tmp("canon-out");
        write(&outside.join("keep"), 10);
        std::os::unix::fs::symlink(&outside, h.join("junk/ln")).unwrap();
        // Different case + a `..` detour: the real on-disk path is what gets reported/deleted.
        let raw = h.join("junk/a/../ONE");
        let items = [
            item(&raw, DeleteMode::Permanent),
            item(&h.join("junk/ln"), DeleteMode::Permanent),
        ];
        let r = run(&items, &t, &h, &hist, &never, &no_admin, &no_tool);
        assert_eq!(errors(&r), [None, None]);
        assert_eq!(r.results[0].path, h.join("junk/one").to_string_lossy());
        assert!(!h.join("junk/one").exists());
        assert!(fs::symlink_metadata(h.join("junk/ln")).is_err());
        assert!(outside.join("keep").exists());
    }

    #[test]
    fn works_without_scan() {
        let (h, hist, _) = setup("noscan");
        let t = RwLock::new(None);
        let r = run(
            &[item(&h.join("junk/one"), DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [None]);
        assert_eq!(r.expected, 0);
        assert!(!h.join("junk/one").exists());
    }

    /// Two sibling paths inside the real 🔑 simulator cache (root-owned on a dev Mac); the
    /// elevator is faked, nothing is deleted. None (test skipped) when there are no siblings.
    fn admin_paths() -> Option<[PathBuf; 2]> {
        let mut dir = PathBuf::from("/Library/Developer/CoreSimulator/Caches");
        loop {
            let mut kids: Vec<PathBuf> = fs::read_dir(&dir)
                .ok()?
                .flatten()
                .map(|e| e.path())
                .collect();
            kids.sort();
            match kids.len() {
                0 => return None,
                1 => dir = kids.pop()?,
                _ => return Some([kids.swap_remove(0), kids.swap_remove(0)]),
            }
        }
    }

    #[test]
    fn run_batches_admin_items_once() {
        let Some([sim, upd]) = admin_paths() else {
            eprintln!("skipped: no admin rule paths on this machine");
            return;
        };
        let (h, hist, t) = setup("admin");
        let one = h.join("junk/one");
        let calls = std::cell::RefCell::new(vec![]);
        let fake = |b: &[DeleteItem], home: &Path| {
            calls.borrow_mut().push((
                b.iter().map(|x| x.path.clone()).collect::<Vec<_>>(),
                home.to_owned(),
            ));
            Ok(b.iter()
                .map(|x| ItemResult {
                    path: x.path.clone(),
                    ok: x.path == b[0].path,
                    error: (x.path != b[0].path).then(|| "boom".into()),
                })
                .collect())
        };
        let items = [
            item(&sim, DeleteMode::Permanent),
            item(&one, DeleteMode::Permanent),
            item(&upd, DeleteMode::Permanent),
        ];
        let r = run(&items, &t, &h, &hist, &never, &fake, &no_tool);
        assert_eq!(errors(&r), [None, None, Some("boom")]);
        let s = |p: &PathBuf| p.to_string_lossy().into_owned();
        assert_eq!(*calls.borrow(), [(vec![s(&sim), s(&upd)], h.clone())]);
        assert!(!one.exists()); // the normal item went the normal way
        assert_eq!(history::list_from(&hist).unwrap().len(), 3);
        // A Trash request for a 🔑 item never reaches the elevator.
        let r = run(
            &[item(&sim, DeleteMode::Trash)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [Some("admin_trash")]);
        // No 🔑 item, no elevation (no_admin panics).
        run(
            &[item(&h.join("junk/two"), DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &no_tool,
        );
    }

    #[test]
    fn run_elevation_cancelled_marks_items() {
        let Some([sim, upd]) = admin_paths() else {
            eprintln!("skipped: no admin rule paths on this machine");
            return;
        };
        let (h, hist, t) = setup("cancel");
        let cancel = |_: &[DeleteItem], _: &Path| Err(AppError::Cancelled);
        let items = [
            item(&sim, DeleteMode::Permanent),
            item(&upd, DeleteMode::Permanent),
        ];
        let r = run(&items, &t, &h, &hist, &never, &cancel, &no_tool);
        assert_eq!(
            errors(&r),
            [Some("admin_cancelled"), Some("admin_cancelled")]
        );
        assert_eq!(r.expected, 0);
        let l = history::list_from(&hist).unwrap();
        assert!(
            l.iter()
                .all(|e| !e.ok && e.error.as_deref() == Some("admin_cancelled"))
        );
        let fail = |_: &[DeleteItem], _: &Path| Err(AppError::Invalid("admin_failed: x".into()));
        let r = run(&items[..1], &t, &h, &hist, &never, &fail, &no_tool);
        assert_eq!(errors(&r), [Some("admin_failed")]);
        // A short result list fails the missing items instead of guessing.
        let short = |_: &[DeleteItem], _: &Path| Ok(vec![]);
        let r = run(&items, &t, &h, &hist, &never, &short, &no_tool);
        assert_eq!(errors(&r), [Some("admin_failed"), Some("admin_failed")]);
    }

    #[test]
    fn tool_rule_runs_its_tool_not_a_delete() {
        let (h, hist, t) = setup("tool");
        let brew = h.join("Library/Caches/Homebrew");
        let calls = std::cell::RefCell::new(vec![]);
        let fake = |c: rules::Cleaner, p: &Path| {
            calls.borrow_mut().push((c, p.to_owned()));
            Ok(())
        };
        // Mode is irrelevant for a tool row; Trash must not be refused.
        let r = run(
            &[item(&brew, DeleteMode::Trash)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &fake,
        );
        assert_eq!(errors(&r), [None]);
        assert_eq!(
            *calls.borrow(),
            [(rules::Cleaner::BrewCleanup, brew.clone())]
        );
        // The tool decides what goes (Docker.raw never shrinks): only statfs before/after counts.
        assert_eq!(r.expected, 0);
        assert!(brew.join("downloads/x").exists()); // the fake tool deleted nothing
        let l = history::list_from(&hist).unwrap();
        assert_eq!((l[0].size, l[0].tool.as_deref()), (0, Some("brew")));
        assert!(size_of(&t, &brew).is_some()); // kept in the tree until rescan
        let failing = |_: rules::Cleaner, _: &Path| {
            Err("tool_failed:Cannot connect to the Docker daemon".into())
        };
        let r = run(
            &[item(&brew, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &failing,
        );
        assert_eq!(
            errors(&r),
            [Some("tool_failed:Cannot connect to the Docker daemon")]
        );
        assert_eq!(r.expected, 0);
        let l = history::list_from(&hist).unwrap();
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn inherited_tool_child_rejected() {
        let (h, hist, t) = setup("toolchild");
        let child = h.join("Library/Caches/Homebrew/downloads");
        let r = run(
            &[item(&child, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &no_tool,
        );
        assert_eq!(errors(&r), [Some("tool_root_only")]);
        assert!(child.join("x").exists());
    }

    #[test]
    fn missing_tool_reported() {
        let (h, hist, t) = setup("toolmissing");
        let brew = h.join("Library/Caches/Homebrew");
        let missing = |_: rules::Cleaner, _: &Path| Err("tool_missing:brew".into());
        let r = run(
            &[item(&brew, DeleteMode::Permanent)],
            &t,
            &h,
            &hist,
            &never,
            &no_admin,
            &missing,
        );
        assert_eq!(errors(&r), [Some("tool_missing:brew")]);
        assert!(!r.results[0].ok);
    }
}

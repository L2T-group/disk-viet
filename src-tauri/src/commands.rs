use crate::cleaner::{self, DeleteItem, DeleteReport, tools};
use crate::disk;
use crate::error::{AppError, AppResult};
use crate::history::{self, HistoryEntry};
use crate::rules::{self, Cleaner, Safety};
use crate::scanner::{self, Node, NodeId, Progress, Tree};
use crate::state::AppState;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeView {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub rule_id: Option<String>,
    pub safety: Safety,
    pub needs_admin: bool,
    /// Tool that cleans this node (`brew`, `xcrun`, `docker`); only on the rule-matched node.
    pub tool: Option<&'static str>,
    /// The exact command shown to the user, with `tool`.
    pub tool_command: Option<String>,
    /// `tool` is not installed: the node cannot be cleaned.
    pub tool_missing: bool,
    /// Inside a tool-cleaned folder: only the folder itself can be cleaned.
    pub tool_child: bool,
    pub unreadable: bool,
    pub small_bytes: u64,
    pub small_count: u32,
    pub has_children: bool,
    /// Overview-only: what is left of a root child after its rule tiles are cut out.
    pub remainder: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelView {
    pub node: NodeView,
    pub children: Vec<NodeView>,
    pub crumbs: Vec<NodeView>,
    pub unexplained: Option<u64>,
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    files: u64,
    bytes: u64,
}

#[derive(Serialize, Clone)]
struct DoneEvent {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn node_view(tree: &Tree, id: NodeId) -> NodeView {
    let n = &tree.nodes[id as usize];
    let cleaner = n.rule.map_or(Cleaner::Files, |h| h.rule.cleaner);
    let direct_tool = n.rule.is_some_and(|h| !h.inherited) && cleaner != Cleaner::Files;
    let tool = direct_tool.then(|| tools::tool_name(cleaner)).flatten();
    NodeView {
        id,
        name: n.name.to_string(),
        path: tree.path_of(id).to_string_lossy().into_owned(),
        size: n.size,
        is_dir: n.is_dir,
        rule_id: n.rule.map(|h| h.rule.id.to_string()),
        safety: n.rule.map_or(Safety::Unknown, |h| h.rule.safety),
        needs_admin: n.rule.is_some_and(|h| h.rule.needs_admin),
        tool,
        tool_command: tool.and_then(|_| tools::command_text(cleaner)),
        tool_missing: tool.is_some() && tools::tool_path(cleaner).is_none(),
        tool_child: !direct_tool && cleaner != Cleaner::Files,
        unreadable: n.unreadable,
        small_bytes: n.small_bytes,
        small_count: n.small_count,
        has_children: !n.children.is_empty(),
        remainder: false,
    }
}

/// Collects outermost direct-rule tiles under `id` and returns the bytes they cover.
/// Only Safe/Caution are pulled out; Important/System/Unknown stay in the remainder tiles:
/// pulling them out would explode into one tile per user file (e.g. every ~/Documents/* child)
/// and the overview's job is to surface deletable space.
fn rule_tiles(tree: &Tree, id: NodeId, out: &mut Vec<NodeId>) -> u64 {
    let n = &tree.nodes[id as usize];
    if is_rule_tile(n) {
        out.push(id);
        return n.size;
    }
    n.children.iter().map(|&c| rule_tiles(tree, c, out)).sum()
}

fn is_rule_tile(n: &Node) -> bool {
    n.rule
        .is_some_and(|h| !h.inherited && matches!(h.rule.safety, Safety::Safe | Safety::Caution))
}

fn overview(tree: &Tree, unexplained: Option<u64>) -> LevelView {
    let mut tiles = Vec::new();
    let mut children: Vec<NodeView> = Vec::new();
    for &c in &tree.nodes[0].children {
        let covered = rule_tiles(tree, c, &mut tiles);
        let left = tree.nodes[c as usize].size.saturating_sub(covered);
        if left > 0 {
            let mut v = node_view(tree, c);
            v.size = left;
            v.remainder = true;
            children.push(v);
        }
    }
    children.extend(tiles.into_iter().map(|t| node_view(tree, t)));
    children.sort_by_key(|c| std::cmp::Reverse(c.size));
    LevelView {
        node: node_view(tree, 0),
        children,
        crumbs: Vec::new(),
        unexplained,
    }
}

fn attached(tree: &Tree, id: NodeId) -> AppResult<&Node> {
    let node = tree
        .nodes
        .get(id as usize)
        .ok_or_else(|| AppError::Invalid(format!("unknown node {id}")))?;
    // A deleted node is only detached (`Tree::remove`): refuse it and anything under it.
    let mut cur = id;
    while let Some(p) = tree.nodes[cur as usize].parent {
        if !tree.nodes[p as usize].children.contains(&cur) {
            return Err(AppError::Invalid(format!("removed node {id}")));
        }
        cur = p;
    }
    Ok(node)
}

/// Children of each tile, sorted desc, to draw inside it. On the overview a remainder tile's
/// children lose the rule tiles inside them (those are tiles of their own), so each list plus
/// the tile's `small_bytes` still sums to the tile's displayed size.
pub fn children_view(tree: &Tree, ids: &[NodeId], overview: bool) -> Vec<(NodeId, Vec<NodeView>)> {
    // Ids gone since the layout (deleted meanwhile) are skipped, not fatal for the batch.
    ids.iter()
        .filter_map(|&id| {
            let node = attached(tree, id).ok()?;
            let cut = overview && !is_rule_tile(node);
            let mut kids: Vec<NodeView> = node
                .children
                .iter()
                .filter_map(|&k| {
                    let mut v = node_view(tree, k);
                    if cut {
                        let covered = rule_tiles(tree, k, &mut Vec::new());
                        v.size -= covered; // a rule tile itself drops to 0
                        v.remainder = covered > 0;
                    }
                    (v.size > 0).then_some(v)
                })
                .collect();
            kids.sort_by_key(|c| std::cmp::Reverse(c.size));
            Some((id, kids))
        })
        .collect()
}

/// Children sorted by size descending; crumbs run from the root down to the parent.
/// Id 0 is the overview level (rule tiles + remainder tiles).
pub fn level_view(tree: &Tree, id: NodeId, unexplained: Option<u64>) -> AppResult<LevelView> {
    if id == 0 && !tree.nodes.is_empty() {
        return Ok(overview(tree, unexplained));
    }
    let node = attached(tree, id)?;
    let mut kids: Vec<NodeId> = node.children.clone();
    kids.sort_by_key(|&k| std::cmp::Reverse(tree.nodes[k as usize].size));
    let mut crumbs = Vec::new();
    let mut cur = node.parent;
    while let Some(p) = cur {
        crumbs.push(node_view(tree, p));
        cur = tree.nodes[p as usize].parent;
    }
    crumbs.reverse();
    Ok(LevelView {
        node: node_view(tree, id),
        children: kids.into_iter().map(|k| node_view(tree, k)).collect(),
        crumbs,
        unexplained,
    })
}

fn is_volume_root(root: &Path) -> bool {
    root == Path::new("/") || disk::volumes().iter().any(|v| Path::new(&v.path) == root)
}

#[tauri::command]
pub fn scan_start(app: AppHandle, state: State<'_, Arc<AppState>>, root: String) -> AppResult<()> {
    let root = std::fs::canonicalize(&root)?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut cur = state.cancel.lock().unwrap();
        cur.store(true, Ordering::Relaxed);
        *cur = cancel.clone();
    }
    let state = Arc::clone(&state);
    std::thread::spawn(move || {
        let progress = Arc::new(Progress::default());
        let finished = Arc::new(AtomicBool::new(false));
        let ticker = {
            let (app, progress, finished) = (app.clone(), progress.clone(), finished.clone());
            std::thread::spawn(move || {
                while !finished.load(Ordering::Relaxed) {
                    let _ = app.emit(
                        "scan://progress",
                        ProgressEvent {
                            files: progress.files.load(Ordering::Relaxed),
                            bytes: progress.bytes.load(Ordering::Relaxed),
                        },
                    );
                    std::thread::sleep(Duration::from_millis(100));
                }
            })
        };
        let home = std::env::home_dir()
            .and_then(|h| h.canonicalize().ok())
            .unwrap_or_default();
        let result = scanner::scan(&root, &cancel, &progress).map(|mut tree| {
            rules::classify(&mut tree, &home);
            tree
        });
        finished.store(true, Ordering::Relaxed);
        let _ = ticker.join();
        let superseded = !Arc::ptr_eq(&cancel, &state.cancel.lock().unwrap());
        let done = match result {
            Ok(tree) if !superseded => {
                *state.tree.write().unwrap() = Some(tree);
                DoneEvent {
                    ok: true,
                    error: None,
                }
            }
            Ok(_) => return,
            Err(_) if superseded => return,
            Err(e) => DoneEvent {
                ok: false,
                error: Some(e.to_string()),
            },
        };
        let _ = app.emit("scan://done", done);
    });
    Ok(())
}

#[tauri::command]
pub fn scan_cancel(state: State<'_, Arc<AppState>>) {
    state.cancel.lock().unwrap().store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn level_get(state: State<'_, Arc<AppState>>, id: u32) -> AppResult<LevelView> {
    let guard = state.tree.read().unwrap();
    let tree = guard.as_ref().ok_or(AppError::NotScanned)?;
    let unexplained = if id == 0 && is_volume_root(&tree.root_path) {
        disk::used_bytes(&tree.root_path)
            .ok()
            .map(|u| u.saturating_sub(tree.nodes[0].size))
    } else {
        None
    };
    level_view(tree, id, unexplained)
}

#[tauri::command]
pub fn children_get(
    state: State<'_, Arc<AppState>>,
    ids: Vec<u32>,
    overview: bool,
) -> AppResult<Vec<(u32, Vec<NodeView>)>> {
    let guard = state.tree.read().unwrap();
    let tree = guard.as_ref().ok_or(AppError::NotScanned)?;
    Ok(children_view(tree, &ids, overview))
}

/// Runs `open` with separate args (never through a shell).
fn run_open(args: &[&str]) -> AppResult<()> {
    let st = std::process::Command::new("open").args(args).status()?;
    if st.success() {
        Ok(())
    } else {
        Err(AppError::Invalid(format!("open failed: {st}")))
    }
}

/// Select `path` in Finder. Must be absolute, so it can never be read as an `open` flag.
#[tauri::command]
pub fn reveal_path(path: String) -> AppResult<()> {
    if !Path::new(&path).is_absolute() {
        return Err(AppError::Invalid(format!("not an absolute path: {path}")));
    }
    run_open(&["-R", &path])
}

/// System Settings › Privacy & Security › Full Disk Access.
#[tauri::command]
pub fn open_fda_settings() -> AppResult<()> {
    run_open(&["x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"])
}

fn home() -> AppResult<std::path::PathBuf> {
    std::env::home_dir().ok_or_else(|| AppError::Guard("no_home".into()))
}

/// Deletes the items (each through `guard::check`) off the UI thread.
#[tauri::command]
pub async fn delete_run(
    state: State<'_, Arc<AppState>>,
    items: Vec<DeleteItem>,
) -> AppResult<DeleteReport> {
    let home = home()?;
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let dir = history::dir(&home);
        cleaner::run(
            &items,
            &state.tree,
            &home,
            &dir,
            &cleaner::files::is_running,
            &cleaner::files::open_files,
            &cleaner::admin::run_elevated,
            &cleaner::tools::run_tool,
        )
    })
    .await
    .map_err(|e| AppError::Invalid(e.to_string()))
}

#[tauri::command]
pub fn history_list() -> AppResult<Vec<HistoryEntry>> {
    Ok(history::list_from(&history::dir(&home()?))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reveal_rejects_relative_and_flag_like_paths() {
        assert!(reveal_path("-a Calculator".into()).is_err());
        assert!(reveal_path("Documents".into()).is_err());
    }

    fn node(name: &str, parent: Option<NodeId>, children: Vec<NodeId>, size: u64) -> Node {
        Node {
            name: name.into(),
            parent,
            children,
            size,
            is_dir: true,
            ..Default::default()
        }
    }

    #[test]
    fn level_view_sorted_desc() {
        // / -> a -> {x 10, y 30, z 20}
        let tree = Tree {
            root_path: "/r".into(),
            nodes: vec![
                node("r", None, vec![1], 60),
                node("a", Some(0), vec![2, 3, 4], 60),
                node("x", Some(1), vec![], 10),
                node("y", Some(1), vec![], 30),
                node("z", Some(1), vec![], 20),
            ],
        };
        let v = level_view(&tree, 1, None).unwrap();
        let sizes: Vec<u64> = v.children.iter().map(|c| c.size).collect();
        assert_eq!(sizes, [30, 20, 10]);
        assert_eq!(sizes.iter().sum::<u64>() + v.node.small_bytes, v.node.size);
        assert_eq!(v.crumbs.iter().map(|c| c.id).collect::<Vec<_>>(), [0]);
        assert_eq!(v.node.path, "/r/a");
        let deep = level_view(&tree, 3, None).unwrap();
        assert_eq!(deep.crumbs.iter().map(|c| c.id).collect::<Vec<_>>(), [0, 1]);
        assert!(level_view(&tree, 99, None).is_err());
    }

    #[test]
    fn level_view_refuses_removed_nodes() {
        let mut tree = Tree {
            root_path: "/r".into(),
            nodes: vec![
                node("r", None, vec![1], 10),
                node("a", Some(0), vec![2], 10),
                node("b", Some(1), vec![], 10),
            ],
        };
        tree.remove(1);
        assert!(level_view(&tree, 1, None).is_err());
        assert!(level_view(&tree, 2, None).is_err()); // under the removed node
        assert!(level_view(&tree, 0, None).is_ok());
    }

    use crate::rules::{Cleaner, Rule, RuleHit};

    const fn rule(id: &'static str, safety: Safety) -> Rule {
        Rule {
            id,
            patterns: &[],
            safety,
            needs_admin: false,
            processes: &[],
            cleaner: Cleaner::Files,
            in_repo: false,
        }
    }
    static SAFE: Rule = rule("safe", Safety::Safe);
    static CAUTION: Rule = rule("caution", Safety::Caution);
    static IMPORTANT: Rule = rule("important", Safety::Important);

    fn hit(n: &mut Node, r: &'static Rule, inherited: bool) {
        n.rule = Some(RuleHit { rule: r, inherited });
    }

    // r 17 -> Library 10 -> Caches 6 -> pip 3 (Safe); Documents 7 -> proj 7 (Important)
    fn sample() -> Tree {
        let mut nodes = vec![
            node("r", None, vec![1, 5], 17),
            node("Library", Some(0), vec![2], 10),
            node("Caches", Some(1), vec![3], 6),
            node("pip", Some(2), vec![4], 3),
            node("wheel", Some(3), vec![], 3),
            node("Documents", Some(0), vec![6], 7),
            node("proj", Some(5), vec![], 7),
        ];
        hit(&mut nodes[3], &SAFE, false);
        hit(&mut nodes[4], &SAFE, true);
        hit(&mut nodes[6], &IMPORTANT, false);
        Tree {
            root_path: "/r".into(),
            nodes,
        }
    }

    #[test]
    fn tool_only_on_the_rule_matched_node() {
        static BREW: Rule = Rule {
            cleaner: Cleaner::BrewCleanup,
            ..rule("homebrew_cache", Safety::Safe)
        };
        let mut t = sample();
        hit(&mut t.nodes[3], &BREW, false);
        hit(&mut t.nodes[4], &BREW, true);
        let (top, child, plain) = (node_view(&t, 3), node_view(&t, 4), node_view(&t, 6));
        assert_eq!(top.tool, Some("brew"));
        assert_eq!(top.tool_command.as_deref(), Some("brew cleanup -s"));
        assert!(!top.tool_child);
        assert_eq!(
            (child.tool, child.tool_child, child.tool_missing),
            (None, true, false)
        );
        assert_eq!(
            (plain.tool, plain.tool_child, plain.tool_missing),
            (None, false, false)
        );
    }

    #[test]
    fn overview_no_double_count() {
        let v = level_view(&sample(), 0, None).unwrap();
        assert_eq!(v.children.iter().map(|c| c.size).sum::<u64>(), 17);
        let pip = v.children.iter().find(|c| c.id == 3).unwrap();
        assert!(!pip.remainder);
        let lib = v.children.iter().find(|c| c.id == 1).unwrap();
        assert!(lib.remainder);
        assert_eq!(lib.size, 7);
        assert!(v.crumbs.is_empty());
        assert!(v.children.windows(2).all(|w| w[0].size >= w[1].size));
    }

    #[test]
    fn overview_excludes_important_and_nested() {
        let mut t = sample();
        hit(&mut t.nodes[2], &CAUTION, false); // Caches (Caution) contains pip (Safe)
        let v = level_view(&t, 0, None).unwrap();
        let ids: Vec<_> = v.children.iter().map(|c| c.id).collect();
        assert!(ids.contains(&2) && !ids.contains(&3), "{ids:?}");
        assert!(!ids.contains(&6), "{ids:?}"); // Important never its own tile
        assert!(ids.contains(&5)); // Documents stays a remainder tile
        assert_eq!(v.children.iter().map(|c| c.size).sum::<u64>(), 17);
    }

    #[test]
    fn overview_sums_with_small_bytes() {
        // r 18 = Library 10 (pip 3 inside) + Documents 7 + 1 small byte at root
        let mut t = sample();
        t.nodes[0].size = 18;
        t.nodes[0].small_bytes = 1;
        let v = level_view(&t, 0, None).unwrap();
        assert!(v.children.iter().any(|c| c.id == 3 && !c.remainder));
        assert_eq!(
            v.children.iter().map(|c| c.size).sum::<u64>() + v.node.small_bytes,
            v.node.size
        );
    }

    #[test]
    fn children_get_real_node() {
        let t = sample();
        let v = children_view(&t, &[5, 1], false);
        assert_eq!(v.iter().map(|x| x.0).collect::<Vec<_>>(), [5, 1]);
        assert_eq!(v[0].1.iter().map(|c| c.id).collect::<Vec<_>>(), [6]);
        assert_eq!(
            v[1].1.iter().map(|c| (c.id, c.size)).collect::<Vec<_>>(),
            [(2, 6)]
        );
        assert!(!v[1].1[0].remainder);
    }

    #[test]
    fn children_get_remainder_subtracts_rule_tiles() {
        // r 20 -> Library 13 (1 small) -> {Caches 6 -> pip 3 (Safe), Logs 4 (Caution), Prefs 2}
        let mut t = sample();
        t.nodes[0].size = 20;
        t.nodes[1].size = 13;
        t.nodes[1].small_bytes = 1;
        t.nodes[1].children = vec![2, 7, 8];
        t.nodes[0].children = vec![1, 5];
        t.nodes.push(node("Logs", Some(1), vec![], 4));
        t.nodes.push(node("Prefs", Some(1), vec![], 2));
        hit(&mut t.nodes[7], &CAUTION, false);
        let ov = level_view(&t, 0, None).unwrap();
        let shown = |id| ov.children.iter().find(|c| c.id == id).unwrap().size;
        let v = children_view(&t, &[1, 5, 3], true);
        let lib = &v[0].1;
        assert_eq!(
            lib.iter()
                .map(|c| (c.id, c.size, c.remainder))
                .collect::<Vec<_>>(),
            [(2, 3, true), (8, 2, false)] // Logs is its own overview tile
        );
        let sum = |i: usize| v[i].1.iter().map(|c| c.size).sum::<u64>();
        assert_eq!(sum(0) + t.nodes[1].small_bytes, shown(1));
        assert_eq!(sum(1) + t.nodes[5].small_bytes, shown(5));
        // A rule tile keeps its whole contents.
        assert_eq!(sum(2) + t.nodes[3].small_bytes, shown(3));
    }

    #[test]
    fn children_get_refuses_removed() {
        let mut t = sample();
        t.remove(5);
        // Removed (5), under a removed node (6) and unknown (99) ids are left out.
        let v = children_view(&t, &[5, 1, 6, 99], true);
        assert_eq!(v.iter().map(|x| x.0).collect::<Vec<_>>(), [1]);
    }
}

pub mod pattern;

pub use pattern::matches;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Safety {
    Safe,
    Caution,
    Important,
    System,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cleaner {
    Files,
    BrewCleanup,
    SimctlRuntime,
    SimctlDeleteUnavailable,
    DockerPrune,
}

#[derive(Debug)]
pub struct Rule {
    pub id: &'static str,
    pub patterns: &'static [&'static str],
    pub safety: Safety,
    pub needs_admin: bool,
    /// Processes that must quit before cleaning.
    pub processes: &'static [&'static str],
    pub cleaner: Cleaner,
    /// Only matches under an ancestor with `has_git`.
    pub in_repo: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RuleHit {
    pub rule: &'static Rule,
    pub inherited: bool,
}

pub mod macos;
pub use macos::RULES;

use crate::scanner::{NodeId, Tree};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Attach a `RuleHit` to every node. Preorder DFS that carries the path down instead of
/// rebuilding it per node.
pub fn classify(tree: &mut Tree, home: &Path) {
    // Ancestors above the scan root still count (rule inheritance, `.git` for in_repo).
    let (parent, in_repo) = match tree.root_path.parent() {
        Some(p) => walk_up(p, home),
        None => (None, false),
    };
    let mut path = tree.root_path.clone();
    visit(tree, 0, &mut path, home, parent, in_repo);
}

fn visit(
    tree: &mut Tree,
    id: NodeId,
    path: &mut PathBuf,
    home: &Path,
    parent: Option<RuleHit>,
    in_repo: bool,
) {
    let n = &mut tree.nodes[id as usize];
    let hit = step(path, home, n.has_git, in_repo, parent);
    n.rule = hit;
    let in_repo = in_repo || n.has_git;
    for i in 0..tree.nodes[id as usize].children.len() {
        let k = tree.nodes[id as usize].children[i];
        path.push(&*tree.nodes[k as usize].name);
        visit(tree, k, path, home, hit, in_repo);
        path.pop();
    }
}

/// Same answer as `classify` for that node, using only the path and the disk.
#[cfg(test)]
pub fn classify_path(path: &Path, home: &Path) -> (Option<&'static Rule>, Safety) {
    let rule = hit_path(path, home).map(|h| h.rule);
    (rule, rule.map_or(Safety::Unknown, |r| r.safety))
}

/// The rule hit for `path` (with `inherited`), same as `classify` would attach.
pub fn hit_path(path: &Path, home: &Path) -> Option<RuleHit> {
    walk_up(path, home).0
}

/// Classify `path` by applying `step` to each ancestor from `/` down, checking `.git` on disk.
/// Returns the hit for `path` and whether `path` or an ancestor is a git repo.
fn walk_up(path: &Path, home: &Path) -> (Option<RuleHit>, bool) {
    let mut chain: Vec<&Path> = path.ancestors().collect();
    chain.reverse();
    let (mut hit, mut in_repo) = (None, false);
    for p in chain {
        let has_git = std::fs::symlink_metadata(p.join(".git")).is_ok();
        hit = step(p, home, has_git, in_repo, hit);
        in_repo |= has_git;
    }
    (hit, in_repo)
}

/// Precedence: a direct match beats an inherited one; among direct matches the most specific
/// pattern wins; `git_projects` (by `.git`, no pattern) only when no pattern matches.
/// Repo rules never apply under a System ancestor (a git checkout inside an app stays System).
fn step(
    path: &Path,
    home: &Path,
    has_git: bool,
    in_repo: bool,
    parent: Option<RuleHit>,
) -> Option<RuleHit> {
    let last2 = [path.file_name(), path.parent().and_then(Path::file_name)];
    let under_system = parent.is_some_and(|h| h.rule.safety == Safety::System);
    let direct = RULES
        .iter()
        .filter(|r| !r.in_repo || (in_repo && !under_system))
        // System is sticky: nothing under it becomes file-deletable. Tool cleaners are exempt
        // (they never delete files; simulator runtimes live under /System).
        .filter(|r| !under_system || r.cleaner != Cleaner::Files)
        .filter_map(|r| {
            if r.in_repo && !has_manifest(path) {
                return None;
            }
            let best = r
                .patterns
                .iter()
                .filter(|p| maybe(p, last2) && matches(p, path, home))
                .map(|p| specificity(p))
                .max()?;
            Some((best, r))
        })
        .max_by_key(|&(s, _)| s)
        .map(|(_, r)| r)
        .or_else(|| (has_git && !under_system).then(git_rule));
    match direct {
        Some(rule) => Some(RuleHit {
            rule,
            inherited: false,
        }),
        None => parent.map(|h| RuleHit {
            inherited: true,
            ..h
        }),
    }
}

/// More literal segments first, then more literal chars; `**`, `*` and `{..}` groups don't count.
fn specificity(pattern: &str) -> (usize, usize) {
    let segs = pattern
        .split('/')
        .filter(|s| !s.is_empty() && *s != "**")
        .count();
    let (mut chars, mut depth) = (0, 0);
    for c in pattern.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            '*' | '/' => {}
            _ if depth == 0 => chars += 1,
            _ => {}
        }
    }
    (segs, chars)
}

/// Cheap reject before `matches`: the pattern's last two literal segments must equal the
/// node's name and its parent's name (`Path::components` per pattern is too slow at 400k nodes).
fn maybe(pattern: &str, last2: [Option<&OsStr>; 2]) -> bool {
    // A segment ending in a literal byte only matches names ending in that byte.
    let end = pattern.as_bytes().last().copied();
    if !matches!(end, Some(b'*' | b'}'))
        && end != last2[0].and_then(|n| n.as_bytes().last().copied())
    {
        return false;
    }
    let mut segs = pattern.rsplit('/');
    for want in last2 {
        let Some(seg) = segs.next() else { return true };
        if seg.is_empty() || seg == "**" || seg.starts_with('~') {
            return true;
        }
        if !seg.contains(['*', '{']) && want != Some(OsStr::new(seg)) {
            return false;
        }
    }
    true
}

/// A build-output folder counts only next to its project manifest; tool installs that are
/// git clones (Homebrew's lib/node_modules, pip's own `build` package) have none.
fn has_manifest(path: &Path) -> bool {
    let manifests: &[&str] = match path.file_name().and_then(OsStr::to_str) {
        Some("node_modules") => &["package.json"],
        Some("target") => &["Cargo.toml"],
        Some("build") => &[
            "pubspec.yaml",
            "build.gradle",
            "build.gradle.kts",
            "CMakeLists.txt",
            "package.json",
        ],
        Some(".dart_tool") => &["pubspec.yaml"],
        Some("Pods") => &["Podfile"],
        _ => return false,
    };
    let Some(dir) = path.parent() else {
        return false;
    };
    manifests.iter().any(|m| dir.join(m).exists())
}

fn git_rule() -> &'static Rule {
    RULES.iter().find(|r| r.id == "git_projects").unwrap()
}

#[cfg(test)]
mod tests;

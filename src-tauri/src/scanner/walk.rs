use super::tree::{Node, NodeId, Tree};
use crate::error::AppError;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

/// Files below this are aggregated into the parent instead of becoming nodes.
const SMALL: u64 = 1 << 20;

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub bytes: AtomicU64,
}

/// Owned subtree built by one `walk_dir`; flattened into the arena once at the end.
struct Entry {
    node: Node,
    kids: Vec<Entry>,
}

struct Ctx<'a> {
    devs: Vec<u64>,
    seen: Mutex<HashSet<(u64, u64)>>,
    cancel: &'a AtomicBool,
    progress: &'a Progress,
}

impl<'a> Ctx<'a> {
    fn new(root: &Path, cancel: &'a AtomicBool, progress: &'a Progress) -> std::io::Result<Self> {
        let dev = fs::symlink_metadata(root)?.dev();
        let mut devs = vec![dev];
        // Boot volume: `/` (System) and its firmlinked Data volume count as one disk.
        if let (Ok(sys), Ok(data)) = (
            fs::symlink_metadata("/"),
            fs::symlink_metadata("/System/Volumes/Data"),
        ) && (dev == sys.dev() || dev == data.dev())
        {
            devs = vec![sys.dev(), data.dev()];
        }
        Ok(Ctx {
            devs,
            seen: Mutex::default(),
            cancel,
            progress,
        })
    }
}

pub fn scan(root: &Path, cancel: &AtomicBool, progress: &Progress) -> Result<Tree, AppError> {
    let md = fs::symlink_metadata(root)?;
    if !md.is_dir() {
        return Err(AppError::Invalid(format!(
            "not a directory: {}",
            root.display()
        )));
    }
    let ctx = Ctx::new(root, cancel, progress)?;
    let name = root.to_string_lossy().into();
    let entry = walk_dir(root, name, md.blocks() * 512, &ctx);
    if cancel.load(Relaxed) {
        return Err(AppError::Cancelled);
    }
    let entry = entry.ok_or_else(|| std::io::Error::from(ErrorKind::NotFound))?;
    let mut nodes = Vec::new();
    flatten(entry, None, &mut nodes);
    Ok(Tree {
        nodes,
        root_path: root.to_path_buf(),
    })
}

fn flatten(e: Entry, parent: Option<NodeId>, nodes: &mut Vec<Node>) -> NodeId {
    let id = nodes.len() as NodeId;
    nodes.push(Node { parent, ..e.node });
    let kids: Vec<NodeId> = e
        .kids
        .into_iter()
        .map(|k| flatten(k, Some(id), nodes))
        .collect();
    nodes[id as usize].children = kids;
    id
}

/// Walk one directory. `None` = vanished (or cancelled); unreadable dirs come back flagged.
fn walk_dir(path: &Path, name: Box<str>, own: u64, ctx: &Ctx) -> Option<Entry> {
    if ctx.cancel.load(Relaxed) {
        return None;
    }
    let mut node = Node {
        name,
        size: own,
        is_dir: true,
        ..Node::default()
    };
    let rd = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return None;
        }
        Err(_) => {
            node.unreadable = true;
            return Some(Entry {
                node,
                kids: Vec::new(),
            });
        }
    };
    let mut kids = Vec::new();
    let mut subdirs: Vec<(PathBuf, Box<str>, u64)> = Vec::new();
    let (mut files, mut file_bytes) = (0u64, 0u64);
    for de in rd.flatten() {
        let fname = de.file_name();
        if fname == ".git" {
            node.has_git = true;
        }
        // DirEntry::metadata is lstat on unix: symlinks are never followed.
        let Ok(md) = de.metadata() else { continue };
        if !ctx.devs.contains(&md.dev()) {
            continue; // other mount point
        }
        let bytes = md.blocks() * 512;
        let name: Box<str> = fname.to_string_lossy().into();
        if md.is_dir() {
            let p = de.path();
            if p == Path::new("/System/Volumes") {
                continue; // firmlink mirror of the Data volume; would double count
            }
            subdirs.push((p, name, bytes));
            continue;
        }
        if md.nlink() > 1 && !ctx.seen.lock().unwrap().insert((md.dev(), md.ino())) {
            continue;
        }
        files += 1;
        file_bytes += bytes;
        if bytes >= SMALL {
            kids.push(Entry {
                node: Node {
                    name,
                    size: bytes,
                    ..Node::default()
                },
                kids: Vec::new(),
            });
        } else {
            node.small_bytes += bytes;
            node.small_count += 1;
        }
    }
    ctx.progress.files.fetch_add(files, Relaxed);
    ctx.progress.bytes.fetch_add(file_bytes, Relaxed);

    let sub: Vec<Entry> = subdirs
        .into_par_iter()
        .filter_map(|(p, n, b)| walk_dir(&p, n, b, ctx))
        .collect();
    kids.extend(sub);
    node.size += file_bytes
        + kids
            .iter()
            .filter(|k| k.node.is_dir)
            .map(|k| k.node.size)
            .sum::<u64>();
    Some(Entry { node, kids })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    const MB: u64 = 1 << 20;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("dv-scan-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(path: &Path, len: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::File::create(path)
            .unwrap()
            .write_all(&vec![7u8; len])
            .unwrap();
    }

    fn run(root: &Path) -> Tree {
        scan(root, &AtomicBool::new(false), &Progress::default()).unwrap()
    }

    #[test]
    fn counts_allocated_blocks_not_len() {
        let d = tmp("sparse");
        fs::File::create(d.join("sparse"))
            .unwrap()
            .set_len(1 << 30)
            .unwrap();
        assert!(run(&d).nodes[0].size < MB);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn hardlink_counted_once() {
        let d = tmp("hardlink");
        write(&d.join("a/big"), 2 * MB as usize);
        fs::create_dir(d.join("b")).unwrap();
        fs::hard_link(d.join("a/big"), d.join("b/link")).unwrap();
        let size = run(&d).nodes[0].size;
        assert!(size.abs_diff(2 * MB) <= 64 * 1024, "size {size}");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn symlink_not_followed() {
        let d = tmp("symlink");
        let outside = tmp("symlink-target");
        write(&outside.join("big"), 5 * MB as usize);
        std::os::unix::fs::symlink(&outside, d.join("link")).unwrap();
        assert!(run(&d).nodes[0].size < MB);
        fs::remove_dir_all(&d).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn small_files_aggregated() {
        let d = tmp("small");
        for i in 0..10 {
            write(&d.join(format!("f{i}")), 10 * 1024);
        }
        let t = run(&d);
        assert!(t.nodes[0].children.is_empty());
        assert_eq!(t.nodes[0].small_count, 10);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn unreadable_dir_flagged() {
        let d = tmp("unreadable");
        let locked = d.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let res = scan(&d, &AtomicBool::new(false), &Progress::default());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        let t = res.unwrap();
        let id = t.find(&locked).expect("locked dir is a node");
        assert!(t.nodes[id as usize].unreadable);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn vanished_entry_is_skipped() {
        let d = tmp("vanished");
        let gone = d.join("gone");
        let (cancel, progress) = (AtomicBool::new(false), Progress::default());
        let ctx = Ctx::new(&d, &cancel, &progress).unwrap();
        assert!(walk_dir(&gone, "gone".into(), 0, &ctx).is_none());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn cancel_returns_cancelled() {
        let d = tmp("cancel");
        let res = scan(&d, &AtomicBool::new(true), &Progress::default());
        assert!(matches!(res, Err(AppError::Cancelled)));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn has_git_flag() {
        let d = tmp("git");
        fs::create_dir_all(d.join("repo/.git")).unwrap();
        let t = run(&d);
        let id = t.find(&d.join("repo")).unwrap();
        assert!(t.nodes[id as usize].has_git);
        assert!(!t.nodes[0].has_git);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn remove_subtracts_from_ancestors() {
        let d = tmp("remove");
        write(&d.join("a/b/big"), 2 * MB as usize);
        let mut t = run(&d);
        let big = t.find(&d.join("a/b/big")).unwrap();
        let a = t.find(&d.join("a")).unwrap();
        assert_eq!(t.path_of(big), d.join("a/b/big"));
        let (big_size, a_before, root_before) = (
            t.nodes[big as usize].size,
            t.nodes[a as usize].size,
            t.nodes[0].size,
        );
        assert!(big_size >= 2 * MB);
        t.remove(big);
        assert_eq!(t.nodes[a as usize].size, a_before - big_size);
        assert_eq!(t.nodes[0].size, root_before - big_size);
        assert_eq!(t.find(&d.join("a/b/big")), None);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "scans the whole boot volume; run by hand"]
    fn boot_scan_skips_system_volumes() {
        let start = std::time::Instant::now();
        let t = run(Path::new("/"));
        println!(
            "/: {} nodes, {} files, {} bytes, {:?}",
            t.nodes.len(),
            t.nodes.iter().map(|n| n.small_count as u64 + !n.is_dir as u64).sum::<u64>(),
            t.nodes[0].size,
            start.elapsed()
        );
        for id in 0..t.nodes.len() as u32 {
            assert!(
                !t.path_of(id).starts_with("/System/Volumes"),
                "{:?}",
                t.path_of(id)
            );
        }
    }

    #[test]
    #[ignore = "timing sample of $HOME; run by hand"]
    fn home_scan_timing() {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let start = std::time::Instant::now();
        let t = run(&home);
        println!(
            "$HOME: {} nodes, {} bytes, {:?}",
            t.nodes.len(),
            t.nodes[0].size,
            start.elapsed()
        );
    }
}

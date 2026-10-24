use crate::rules::RuleHit;
use std::path::{Path, PathBuf};

pub type NodeId = u32;

#[derive(Debug, Default)]
pub struct Node {
    pub name: Box<str>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// Allocated bytes of the whole subtree (includes `small_bytes`).
    pub size: u64,
    /// Files < 1 MB are not nodes; they are summed into their parent here.
    pub small_bytes: u64,
    pub small_count: u32,
    pub is_dir: bool,
    pub has_git: bool,
    pub unreadable: bool,
    pub rule: Option<RuleHit>,
}

/// Arena tree; node 0 is the root (`root_path`).
pub struct Tree {
    pub nodes: Vec<Node>,
    pub root_path: PathBuf,
}

impl Tree {
    pub fn path_of(&self, id: NodeId) -> PathBuf {
        let mut names = Vec::new();
        let mut cur = id;
        while let Some(p) = self.nodes[cur as usize].parent {
            names.push(&*self.nodes[cur as usize].name);
            cur = p;
        }
        let mut path = self.root_path.clone();
        path.extend(names.iter().rev());
        path
    }

    pub fn find(&self, path: &Path) -> Option<NodeId> {
        let rel = path.strip_prefix(&self.root_path).ok()?;
        let mut cur: NodeId = 0;
        for c in rel.components() {
            let name = c.as_os_str().to_str()?;
            cur = *self.nodes[cur as usize]
                .children
                .iter()
                .find(|&&k| &*self.nodes[k as usize].name == name)?;
        }
        Some(cur)
    }

    /// Detach from parent and subtract its size from every ancestor. The node keeps its
    /// `parent` so `path_of` stays right; removing it twice is a no-op.
    pub fn remove(&mut self, id: NodeId) {
        let size = self.nodes[id as usize].size;
        let Some(parent) = self.nodes[id as usize].parent else {
            return;
        };
        let kids = &mut self.nodes[parent as usize].children;
        let Some(i) = kids.iter().position(|&k| k == id) else {
            return;
        };
        kids.remove(i);
        let mut cur = Some(parent);
        while let Some(p) = cur {
            let n = &mut self.nodes[p as usize];
            n.size = n.size.saturating_sub(size);
            cur = n.parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn remove_keeps_path_and_is_idempotent() {
        // r 10 -> a 7 -> b 4
        let mut t = Tree {
            root_path: "/r".into(),
            nodes: vec![
                node("r", None, vec![1], 10),
                node("a", Some(0), vec![2], 7),
                node("b", Some(1), vec![], 4),
            ],
        };
        t.remove(2);
        assert_eq!(t.path_of(2), Path::new("/r/a/b"));
        assert_eq!((t.nodes[0].size, t.nodes[1].size), (6, 3));
        assert_eq!(t.find(Path::new("/r/a/b")), None);
        t.remove(2);
        assert_eq!((t.nodes[0].size, t.nodes[1].size), (6, 3));
        t.remove(0); // root: no-op
        assert_eq!(t.nodes[0].size, 6);
    }
}

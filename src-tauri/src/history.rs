//! Deletion log: one JSON object per line in `<dir>/history.jsonl`.
use crate::guard::DeleteMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Unix seconds.
    pub time: i64,
    pub path: String,
    pub size: u64,
    pub mode: DeleteMode,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Cleaned with this tool (`brew`, `xcrun`, `docker`): `size` is 0, the tool decides what goes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

const FILE: &str = "history.jsonl";

/// `~/Library/Application Support/DiskViet`.
pub fn dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/DiskViet")
}

pub fn append_to(dir: &Path, e: &HistoryEntry) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut line = serde_json::to_string(e)?;
    line.push('\n');
    // One write per line in append mode: concurrent appends never interleave mid-line.
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(FILE))?
        .write_all(line.as_bytes())
}

/// Newest first; malformed lines are skipped.
pub fn list_from(dir: &Path) -> io::Result<Vec<HistoryEntry>> {
    let text = match fs::read(dir.join(FILE)) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(text
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(time: i64, ok: bool) -> HistoryEntry {
        HistoryEntry {
            time,
            path: format!("/x/{time}"),
            size: 5,
            mode: DeleteMode::Trash,
            ok,
            error: (!ok).then(|| "boom".into()),
            tool: None,
        }
    }

    #[test]
    fn newest_first_skips_malformed() {
        let d = std::env::temp_dir().join(format!("dv-hist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        assert!(list_from(&d).unwrap().is_empty());
        append_to(&d, &entry(1, true)).unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(d.join(FILE))
            .unwrap()
            .write_all(b"{not json\n\n")
            .unwrap();
        append_to(&d, &entry(2, false)).unwrap();
        assert_eq!(list_from(&d).unwrap(), [entry(2, false), entry(1, true)]);
    }
}

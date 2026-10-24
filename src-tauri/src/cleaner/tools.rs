//! Rules cleaned by the vendor's own tool (spec §6, §7): brew, xcrun simctl, docker. Each tool
//! is run directly (no shell) with separate arguments, only when it is installed.
use crate::rules::Cleaner;
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(600);

/// Apps opened from Finder get a minimal PATH, so the usual install dirs are tried first.
const DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"];

/// The executable name for a tool cleaner; `None` for `Files`.
pub fn tool_name(c: Cleaner) -> Option<&'static str> {
    match c {
        Cleaner::Files => None,
        Cleaner::BrewCleanup => Some("brew"),
        Cleaner::SimctlRuntime | Cleaner::SimctlDeleteUnavailable => Some("xcrun"),
        Cleaner::DockerPrune => Some("docker"),
    }
}

pub fn tool_path(c: Cleaner) -> Option<PathBuf> {
    find(tool_name(c)?, std::env::var_os("PATH"))
}

fn find(name: &str, path_env: Option<OsString>) -> Option<PathBuf> {
    let from_env = path_env
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default();
    DIRS.iter()
        .map(PathBuf::from)
        .chain(from_env.into_iter().filter(|d| d.is_absolute()))
        .map(|d| d.join(name))
        .find(|p| {
            std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// Exact arguments after the executable. `runtime` is the simctl runtime identifier.
pub fn tool_argv(c: Cleaner, runtime: &str) -> Vec<OsString> {
    let a: &[&str] = match c {
        Cleaner::Files => &[],
        Cleaner::BrewCleanup => &["cleanup", "-s"],
        Cleaner::SimctlDeleteUnavailable => &["simctl", "delete", "unavailable"],
        Cleaner::SimctlRuntime => &["simctl", "runtime", "delete", runtime],
        // No --volumes: volume data (databases) is never ours to delete.
        Cleaner::DockerPrune => &["system", "prune", "-f"],
    };
    a.iter().map(OsString::from).collect()
}

/// The command as shown to the user; the runtime id is left out (it is looked up at run time).
pub fn command_text(c: Cleaner) -> Option<String> {
    let mut parts = vec![tool_name(c)?.to_string()];
    let argv = tool_argv(c, "");
    parts.extend(
        argv.iter()
            .filter(|a| !a.is_empty())
            .map(|a| a.to_string_lossy().into_owned()),
    );
    Some(parts.join(" "))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Runtime {
    identifier: String,
    #[serde(default)]
    deletable: bool,
    path: Option<PathBuf>,
    parent_image_path: Option<PathBuf>,
    parent_mount_path: Option<PathBuf>,
}

/// `xcrun simctl runtime list -j` → (disk-image-related paths, identifier) for deletable runtimes.
pub fn parse_runtime_list(json: &str) -> Result<Vec<(Vec<PathBuf>, String)>, String> {
    let m: HashMap<String, Runtime> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(m.into_values()
        .filter(|r| r.deletable)
        .map(|r| {
            let paths = [r.path, r.parent_image_path, r.parent_mount_path];
            (paths.into_iter().flatten().collect(), r.identifier)
        })
        .collect())
}

/// The one runtime whose image lives at or under `target` (the `.asset` dir or the `.dmg`).
pub fn runtime_for(list: &[(Vec<PathBuf>, String)], target: &Path) -> Option<String> {
    let mut hits = list
        .iter()
        .filter(|(paths, _)| paths.iter().any(|p| p.starts_with(target)));
    let (_, id) = hits.next()?;
    hits.next().is_none().then(|| id.clone())
}

fn failed(msg: &str) -> String {
    let m = msg.trim();
    let cut = m.char_indices().nth(300).map_or(m.len(), |(i, _)| i);
    format!("tool_failed:{}", &m[..cut])
}

/// Runs the tool for `c` on `target` (the rule-matched node) as the current user.
pub fn run_tool(c: Cleaner, target: &Path) -> Result<(), String> {
    run_with(c, target, tool_path(c), &exec)
}

type Exec<'a> = dyn Fn(&Path, &[OsString]) -> Result<Vec<u8>, String> + 'a;

fn run_with(
    c: Cleaner,
    target: &Path,
    exe: Option<PathBuf>,
    exec: &Exec<'_>,
) -> Result<(), String> {
    let name = tool_name(c).ok_or("not_tool")?;
    let exe = exe.ok_or_else(|| format!("tool_missing:{name}"))?;
    let mut runtime = String::new();
    if c == Cleaner::SimctlRuntime {
        let list: Vec<OsString> = ["simctl", "runtime", "list", "-j"]
            .map(OsString::from)
            .into();
        let out = exec(&exe, &list)?;
        let list = parse_runtime_list(&String::from_utf8_lossy(&out)).map_err(|e| failed(&e))?;
        runtime = runtime_for(&list, target).ok_or_else(|| failed("no deletable runtime here"))?;
    }
    exec(&exe, &tool_argv(c, &runtime)).map(drop)
}

/// Runs `exe args`, returns stdout. Non-zero exit → `tool_failed:<stderr>`; 10 min timeout.
fn exec(exe: &Path, args: &[OsString]) -> Result<Vec<u8>, String> {
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| failed(&e.to_string()))?;
    // Drain both pipes on threads so a chatty tool never blocks on a full pipe.
    let drain = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut v = vec![];
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut v);
            }
            v
        })
    };
    let out = drain(child.stdout.take().map(|r| Box::new(r) as _));
    let err = drain(child.stderr.take().map(|r| Box::new(r) as _));
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(200)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(failed("timeout"));
            }
            Err(e) => return Err(failed(&e.to_string())),
        }
    };
    let (out, err) = (
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default(),
    );
    if status.success() {
        Ok(out)
    } else {
        Err(failed(&String::from_utf8_lossy(&err)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const FIXTURE: &str = include_str!("../../tests/fixtures/simctl_runtimes.json");
    const ASSET: &str = "/System/Library/AssetsV2/com_apple_MobileAsset_iOSSimulatorRuntime/e4478b4b9014ff28fe3c265daca488f55a284f4c.asset";

    #[test]
    fn parse_runtime_list_maps_paths_and_skips_undeletable() {
        let l = parse_runtime_list(FIXTURE).unwrap();
        assert_eq!(l.len(), 2);
        let dmg = Path::new(ASSET).join("AssetData/Restore/043-87504-067.dmg");
        assert_eq!(
            runtime_for(&l, &dmg).as_deref(),
            Some("C62A6A94-CFD4-4C23-BED4-4001207F1CFD")
        );
        let off = FIXTURE.replacen("\"deletable\" : true", "\"deletable\" : false", 1);
        assert_eq!(parse_runtime_list(&off).unwrap().len(), 1);
        assert!(parse_runtime_list("nope").is_err());
    }

    #[test]
    fn runtime_target_matching() {
        let l = parse_runtime_list(FIXTURE).unwrap();
        let id = |p: &str| runtime_for(&l, Path::new(p));
        assert_eq!(
            id(ASSET).as_deref(),
            Some("C62A6A94-CFD4-4C23-BED4-4001207F1CFD")
        );
        // 18.6: its base image (parentImagePath) sits in another .asset dir.
        assert_eq!(
            id("/System/Library/AssetsV2/com_apple_MobileAsset_iOSSimulatorRuntime/7fd8477b73d7b67a321fff7d87723eed3c628424.asset").as_deref(),
            Some("F6E8CD05-A388-4271-B1DE-5C44A607F27B")
        );
        assert_eq!(
            id("/Library/Developer/CoreSimulator/Images/other.dmg"),
            None
        );
        assert_eq!(id(&format!("{ASSET}x")), None); // prefix by component, not by string
        // Too broad: holds both runtimes, so neither is picked.
        assert_eq!(
            id("/System/Library/AssetsV2/com_apple_MobileAsset_iOSSimulatorRuntime"),
            None
        );
        assert_eq!(id("/System"), None);
    }

    #[test]
    fn tool_argv_exact() {
        let s = |c, r| {
            tool_argv(c, r)
                .into_iter()
                .map(|a| a.into_string().unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(s(Cleaner::BrewCleanup, "x"), ["cleanup", "-s"]);
        assert_eq!(
            s(Cleaner::SimctlDeleteUnavailable, "x"),
            ["simctl", "delete", "unavailable"]
        );
        assert_eq!(
            s(Cleaner::SimctlRuntime, "ID-1"),
            ["simctl", "runtime", "delete", "ID-1"]
        );
        assert_eq!(s(Cleaner::DockerPrune, "x"), ["system", "prune", "-f"]);
        assert!(s(Cleaner::Files, "x").is_empty());
        assert_eq!(
            command_text(Cleaner::BrewCleanup).as_deref(),
            Some("brew cleanup -s")
        );
        assert_eq!(
            command_text(Cleaner::SimctlRuntime).as_deref(),
            Some("xcrun simctl runtime delete")
        );
        assert_eq!(command_text(Cleaner::Files), None);
    }

    #[test]
    fn missing_tool_reported() {
        let never = |_: &Path, _: &[OsString]| -> Result<Vec<u8>, String> { panic!("ran") };
        let t = Path::new("/x");
        assert_eq!(
            run_with(Cleaner::BrewCleanup, t, None, &never),
            Err("tool_missing:brew".into())
        );
        assert_eq!(
            run_with(Cleaner::SimctlRuntime, t, None, &never),
            Err("tool_missing:xcrun".into())
        );
        assert_eq!(find("diskviet-no-such-tool", Some("/bin:rel".into())), None);
        assert_eq!(
            find("sh", Some("/bin".into())),
            Some(PathBuf::from("/bin/sh"))
        );
    }

    #[test]
    fn runtime_delete_uses_listed_id_and_stderr_is_reported() {
        let calls = RefCell::new(vec![]);
        let fake = |exe: &Path, a: &[OsString]| {
            calls.borrow_mut().push((exe.to_owned(), a.to_vec()));
            if a.iter().any(|x| x == "list") {
                Ok(FIXTURE.as_bytes().to_vec())
            } else {
                Err(failed(&format!("  {}\n", "e".repeat(400))))
            }
        };
        let xcrun = Some(PathBuf::from("/usr/bin/xcrun"));
        let r = run_with(
            Cleaner::SimctlRuntime,
            Path::new(ASSET),
            xcrun.clone(),
            &fake,
        );
        assert_eq!(r, Err(format!("tool_failed:{}", "e".repeat(300))));
        let c = calls.borrow();
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[1].1,
            tool_argv(
                Cleaner::SimctlRuntime,
                "C62A6A94-CFD4-4C23-BED4-4001207F1CFD"
            )
        );
        drop(c);
        let r = run_with(
            Cleaner::SimctlRuntime,
            Path::new("/elsewhere"),
            xcrun,
            &fake,
        );
        assert_eq!(r, Err("tool_failed:no deletable runtime here".into()));
    }
}

use super::*;
use crate::scanner::{Progress, scan};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

fn tmp_home(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dv-rules-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p.canonicalize().unwrap()
}

#[test]
fn table() {
    let home = tmp_home("table");
    fs::create_dir_all(home.join("proj/.git")).unwrap();
    fs::create_dir_all(home.join("proj/node_modules")).unwrap();
    fs::create_dir_all(home.join("x/node_modules")).unwrap();
    fs::write(home.join("proj/package.json"), "{}").unwrap();
    fs::create_dir_all(home.join("proj/src/build")).unwrap();
    // Homebrew-like tool install: a git clone with no manifest next to node_modules / build.
    fs::create_dir_all(home.join("brew/.git")).unwrap();
    fs::create_dir_all(home.join("brew/lib/node_modules/npm")).unwrap();
    fs::create_dir_all(home.join("brew/lib/python/pip/_internal/operations/build")).unwrap();
    let cases: &[(&str, Option<&str>, Safety)] = &[
        (
            "~/Library/Developer/Xcode/DerivedData",
            Some("xcode_derived_data"),
            Safety::Safe,
        ),
        (
            "~/Library/Developer/Xcode/DerivedData/Foo-abc/Build",
            Some("xcode_derived_data"),
            Safety::Safe,
        ),
        (
            "~/Library/Application Support/Cursor/User/globalStorage",
            Some("cursor_global_storage"),
            Safety::Caution,
        ),
        (
            "~/Documents/a.pdf",
            Some("user_documents"),
            Safety::Important,
        ),
        ("~/.ssh/id_ed25519", Some("ssh_gpg"), Safety::System),
        (
            "/Applications/Xcode.app",
            Some("applications"),
            Safety::System,
        ),
        (
            "~/proj/node_modules",
            Some("repo_node_modules"),
            Safety::Caution,
        ),
        (
            "~/proj/node_modules/a/node_modules",
            Some("repo_node_modules"),
            Safety::Caution,
        ),
        ("~/proj/src", Some("git_projects"), Safety::Important),
        ("~/x/node_modules", None, Safety::Unknown),
        ("~/proj/src/build", Some("git_projects"), Safety::Important),
        (
            "~/brew/lib/node_modules",
            Some("git_projects"),
            Safety::Important,
        ),
        (
            "~/brew/lib/node_modules/npm",
            Some("git_projects"),
            Safety::Important,
        ),
        (
            "~/brew/lib/python/pip/_internal/operations/build",
            Some("git_projects"),
            Safety::Important,
        ),
        ("~/Documents", None, Safety::Unknown),
        (
            "~/Pictures/Photos Library.photoslibrary",
            Some("photos_library"),
            Safety::Important,
        ),
        (
            "~/Library/Group Containers/6N38VWS5BX.ru.keepcoder.Telegram/stable/account-1/postbox/media",
            Some("telegram_cache"),
            Safety::Safe,
        ),
        (
            "~/Library/Group Containers/6N38VWS5BX.ru.keepcoder.Telegram/stable/account-1/postbox/db",
            Some("telegram_data"),
            Safety::Caution,
        ),
        ("/usr/local/bin/brew", None, Safety::Unknown),
        // SIP `restricted`: even root gets EPERM, so view-only.
        ("/Library/Updates", Some("macos_updates"), Safety::System),
        ("/usr/bin/ls", Some("system_files"), Safety::System),
        (
            "/Library/Developer/CoreSimulator/Caches",
            Some("simulator_caches"),
            Safety::Safe,
        ),
    ];
    for (p, id, safety) in cases {
        let path = match p.strip_prefix('~') {
            Some(rest) => PathBuf::from(format!("{}{rest}", home.display())),
            None => PathBuf::from(p),
        };
        let (rule, s) = classify_path(&path, &home);
        assert_eq!((rule.map(|r| r.id), s), (*id, *safety), "{p}");
    }
    let (rule, _) = classify_path(Path::new("/Library/Developer/CoreSimulator/Caches"), &home);
    assert!(rule.unwrap().needs_admin);
    let telegram = RULES.iter().find(|r| r.id == "telegram_cache").unwrap();
    assert_eq!(telegram.processes, ["Telegram"]);
}

#[test]
fn classify_tree_matches_classify_path() {
    let home = tmp_home("tree");
    for d in [
        "Library/Developer/Xcode/DerivedData/Foo-abc/Build",
        "proj/.git",
        "proj/node_modules/a",
        "proj/src/build",
        "brew/.git",
        "brew/lib/node_modules/npm",
        "proj/src",
        "x/node_modules",
        "Documents/notes",
        ".ssh",
    ] {
        fs::create_dir_all(home.join(d)).unwrap();
    }
    fs::write(home.join("proj/package.json"), "{}").unwrap();
    let mut tree = scan(&home, &AtomicBool::new(false), &Progress::default()).unwrap();
    classify(&mut tree, &home);
    let hit = |rel: &str| tree.nodes[tree.find(&home.join(rel)).unwrap() as usize].rule;
    let dd = hit("Library/Developer/Xcode/DerivedData").unwrap();
    assert_eq!((dd.rule.id, dd.inherited), ("xcode_derived_data", false));
    let build = hit("Library/Developer/Xcode/DerivedData/Foo-abc/Build").unwrap();
    assert_eq!(
        (build.rule.id, build.inherited),
        ("xcode_derived_data", true)
    );
    assert!(hit("x/node_modules").is_none());
    for id in 0..tree.nodes.len() as u32 {
        let path = tree.path_of(id);
        let want = classify_path(&path, &home);
        let got = tree.nodes[id as usize].rule;
        assert_eq!(
            got.map(|h| h.rule.id),
            want.0.map(|r| r.id),
            "{}",
            path.display()
        );
        assert_eq!(got.map_or(Safety::Unknown, |h| h.rule.safety), want.1);
    }
}

#[test]
fn every_rule_has_i18n() {
    let src = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/features/rules/i18n.ts"),
    )
    .unwrap();
    let (vi, en) = src.split_once("\n  en: {").expect("vi then en sections");
    assert!(!RULES.is_empty());
    for r in RULES {
        for part in ["name", "why", "effect"] {
            let key = format!("'rule.{}.{part}':", r.id);
            assert!(vi.contains(&key), "vi missing {key}");
            assert!(en.contains(&key), "en missing {key}");
        }
    }
}

/// Every pattern is well-formed: it doesn't panic and matches a path built from itself.
#[test]
fn every_pattern_parses_and_matches_itself() {
    let home = Path::new("/Users/u");
    let mut ids = std::collections::HashSet::new();
    for r in RULES {
        assert!(ids.insert(r.id), "duplicate id {}", r.id);
        for p in r.patterns {
            let mut s = p.replace("**/", "").replace('*', "x");
            while let (Some(o), Some(c)) = (s.find('{'), s.find('}')) {
                let first = s[o + 1..c].split(',').next().unwrap().to_string();
                s.replace_range(o..=c, &first);
            }
            let s = s.replacen('~', "/Users/u", 1);
            let s = if s.starts_with('/') {
                s
            } else {
                format!("/Users/u/p/{s}")
            };
            assert!(
                matches(p, Path::new(&s), home),
                "{} pattern {p} vs {s}",
                r.id
            );
        }
    }
}

/// `cargo test --release -- --ignored --nocapture classify_home_timing`
#[test]
#[ignore]
fn classify_home_timing() {
    let home = std::env::home_dir().unwrap().canonicalize().unwrap();
    let t = std::time::Instant::now();
    let mut tree = scan(&home, &AtomicBool::new(false), &Progress::default()).unwrap();
    let scanned = t.elapsed();
    let t = std::time::Instant::now();
    classify(&mut tree, &home);
    let classified = t.elapsed();
    let mut by: std::collections::BTreeMap<&str, u64> = Default::default();
    for n in &tree.nodes {
        if let Some(h) = n.rule.filter(|h| !h.inherited) {
            *by.entry(h.rule.id).or_default() += n.size;
        }
    }
    println!(
        "{} rules, {} nodes, scan {scanned:?}, classify {classified:?}",
        RULES.len(),
        tree.nodes.len()
    );
    for (id, b) in by {
        println!("{id:24} {:>8.2} GB", b as f64 / 1e9);
    }
}

#[test]
fn system_is_sticky() {
    // The catalog has no Files rule under a System one yet; fake the parent hit.
    static SYS: Rule = Rule {
        id: "fake_system",
        patterns: &[],
        safety: Safety::System,
        needs_admin: false,
        processes: &[],
        cleaner: Cleaner::Files,
        in_repo: false,
    };
    let home = tmp_home("sticky");
    let parent = Some(RuleHit {
        rule: &SYS,
        inherited: false,
    });
    let pip = home.join("Library/Caches/pip"); // direct match: python_pkg_cache (Safe)
    let hit = step(&pip, &home, false, false, parent).unwrap();
    assert_eq!((hit.rule.id, hit.inherited), ("fake_system", true));
    // Tool cleaners never delete files themselves, so they may still sit under System
    // (simulator runtimes live in /System/Library/AssetsV2).
    let brew = home.join("Library/Caches/Homebrew");
    assert_eq!(
        step(&brew, &home, false, false, parent).unwrap().rule.id,
        "homebrew_cache"
    );
    // Without the fake parent the direct match wins as before.
    assert_eq!(
        step(&pip, &home, false, false, None).unwrap().rule.id,
        "python_pkg_cache"
    );
}

#[test]
fn tool_rules_never_need_admin() {
    // Tools run as the user; the admin helper only deletes files.
    for r in RULES.iter().filter(|r| r.cleaner != Cleaner::Files) {
        assert!(!r.needs_admin, "{}", r.id);
    }
}

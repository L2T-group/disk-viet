use std::path::{Component, Path};

/// Full-path match. `~` = home, `*` = one segment (in-segment glob), `**` = zero or more
/// segments, `{a,b}` = alternatives. Pure: no disk access.
pub fn matches(pattern: &str, path: &Path, home: &Path) -> bool {
    let (pat, path) = match pattern.strip_prefix('~') {
        Some(rest) => match path.strip_prefix(home) {
            Ok(p) => (rest, p),
            Err(_) => return false,
        },
        None => (pattern, path),
    };
    let segs = path.components().filter_map(|c| match c {
        Component::Normal(s) => Some(s),
        _ => None,
    });
    let pats = pat.split('/').filter(|s| !s.is_empty());
    walk(pats, segs)
}

fn walk<'a, P, S>(mut pats: P, mut segs: S) -> bool
where
    P: Iterator<Item = &'a str> + Clone,
    S: Iterator<Item = &'a std::ffi::OsStr> + Clone,
{
    loop {
        match pats.next() {
            None => return segs.next().is_none(),
            Some("**") => {
                // zero or more segments: try the rest of the pattern at every suffix
                loop {
                    if walk(pats.clone(), segs.clone()) {
                        return true;
                    }
                    if segs.next().is_none() {
                        return false;
                    }
                }
            }
            Some(p) => match segs.next() {
                Some(s) if glob(p, &s.to_string_lossy()) => {}
                _ => return false,
            },
        }
    }
}

/// In-segment glob: `*` = any run of chars, `{a,b}` = alternatives.
fn glob(p: &str, s: &str) -> bool {
    if let (Some(open), Some(close)) = (p.find('{'), p.find('}')) {
        // shortcut: no nested braces, one group expanded per call (allocates only for brace patterns)
        let (head, alts, tail) = (&p[..open], &p[open + 1..close], &p[close + 1..]);
        return alts
            .split(',')
            .any(|a| glob(&format!("{head}{a}{tail}"), s));
    }
    let (p, s) = (p.as_bytes(), s.as_bytes());
    let (mut pi, mut si, mut star) = (0, 0, None);
    while si < s.len() {
        if pi < p.len() && p[pi] == b'*' {
            star = Some((pi, si));
            pi += 1;
        } else if pi < p.len() && p[pi] == s[si] {
            pi += 1;
            si += 1;
        } else if let Some((sp, ss)) = star {
            pi = sp + 1;
            si = ss + 1;
            star = Some((sp, ss + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == b'*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pat: &str, path: &str) -> bool {
        matches(pat, Path::new(path), Path::new("/Users/u"))
    }

    #[test]
    fn exact_is_full_path_not_prefix() {
        assert!(m("~/Library/Caches/pip", "/Users/u/Library/Caches/pip"));
        assert!(!m("~/Library/Caches/pip", "/Users/u/Library/Caches/pip/x"));
        assert!(!m(
            "~/Library/Caches/pip",
            "/Users/other/Library/Caches/pip"
        ));
        assert!(m("/usr/local/x", "/usr/local/x"));
    }

    #[test]
    fn star_is_one_segment_with_glob() {
        assert!(m("~/.android/avd/*.avd", "/Users/u/.android/avd/Pixel.avd"));
        assert!(!m(
            "~/.android/avd/*.avd",
            "/Users/u/.android/avd/Pixel.ini"
        ));
        assert!(!m("~/.android/avd/*", "/Users/u/.android/avd/a/b"));
    }

    #[test]
    fn double_star_spans_segments() {
        let p = "~/Library/Group Containers/*.ru.keepcoder.Telegram/**/postbox/media";
        assert!(m(
            p,
            "/Users/u/Library/Group Containers/6N38VWS5BX.ru.keepcoder.Telegram/stable/account-1/postbox/media"
        ));
        assert!(m(
            p,
            "/Users/u/Library/Group Containers/6N38VWS5BX.ru.keepcoder.Telegram/postbox/media"
        ));
        assert!(!m(
            p,
            "/Users/u/Library/Group Containers/6N38VWS5BX.ru.keepcoder.Telegram/postbox"
        ));
    }

    #[test]
    fn braces_are_alternatives() {
        let p = "~/Library/Application Support/{Cursor,Code}/Cache";
        assert!(m(p, "/Users/u/Library/Application Support/Cursor/Cache"));
        assert!(m(p, "/Users/u/Library/Application Support/Code/Cache"));
        assert!(!m(p, "/Users/u/Library/Application Support/Vim/Cache"));
    }
}

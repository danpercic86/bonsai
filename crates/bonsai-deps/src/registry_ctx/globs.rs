//! Pattern matching for Go `GOPRIVATE`-style globs and NuGet
//! `<packageSourceMapping>` id patterns. Iterative, no recursion. Every
//! ambiguity resolves toward "matches", i.e. toward Private (fail closed).

use super::NugetMapping;

/// Go `module.MatchPrefixPatterns` semantics for ONE glob: the glob matches
/// if it matches a `/`-element prefix of `path` (same element count),
/// element by element with `path.Match` rules. Compared case-insensitively
/// (over-matching only makes more modules private). A glob using `[` or `\`
/// (character classes / escapes) is treated as matching its prefix length —
/// fail closed rather than mis-parse.
pub(crate) fn go_glob_matches(glob: &str, path: &str) -> bool {
    let glob = glob.trim();
    if glob.is_empty() {
        return false;
    }
    let glob = glob.to_ascii_lowercase();
    let path = path.to_ascii_lowercase();
    let g_elems: Vec<&str> = glob.trim_end_matches('/').split('/').collect();
    let p_elems: Vec<&str> = path.split('/').collect();
    if p_elems.len() < g_elems.len() {
        return false;
    }
    g_elems
        .iter()
        .zip(p_elems.iter())
        .all(|(g, p)| element_matches(g, p))
}

/// `path.Match` for one element: `*` = any run of non-`/` chars, `?` = one
/// char. Classic iterative wildcard matcher with single-star backtracking.
fn element_matches(glob: &str, s: &str) -> bool {
    if glob.contains(['[', '\\']) {
        return true;
    }
    let g: Vec<char> = glob.chars().collect();
    let t: Vec<char> = s.chars().collect();
    let (mut gi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        match g.get(gi) {
            Some('?') => {
                gi += 1;
                ti += 1;
            }
            Some('*') => {
                star = Some((gi, ti));
                gi += 1;
            }
            Some(c) if *c == t[ti] => {
                gi += 1;
                ti += 1;
            }
            _ => match star {
                Some((sg, st)) => {
                    gi = sg + 1;
                    ti = st + 1;
                    star = Some((sg, st + 1));
                }
                None => return false,
            },
        }
    }
    g[gi..].iter().all(|c| *c == '*')
}

/// NuGet package source mapping: the most specific pattern wins — an exact
/// id beats any prefix, a longer prefix beats a shorter one, `*` is the
/// weakest. Case-insensitive.
pub(crate) fn nuget_best_mapping<'a>(
    mappings: &'a [NugetMapping],
    id: &str,
) -> Option<&'a NugetMapping> {
    let id = id.to_ascii_lowercase();
    mappings
        .iter()
        .filter_map(|m| {
            let pat = m.pattern.to_ascii_lowercase();
            let score = if let Some(prefix) = pat.strip_suffix('*') {
                id.starts_with(prefix).then_some(prefix.len())
            } else {
                (pat == id).then_some(usize::MAX)
            };
            score.map(|s| (s, m))
        })
        .max_by_key(|(s, _)| *s)
        .map(|(_, m)| m)
}

#[cfg(test)]
#[path = "globs_tests.rs"]
mod tests;

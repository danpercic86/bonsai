//! P91 §7.2 — **home-directory masking**: replace the user's home-dir prefix
//! with [`HOME_TOKEN`], keeping everything below it verbatim.
//!
//! ONE concern, kept out of `scrub.rs` (which owns credential SHAPES) so both
//! files stay small: `scrub_string` calls [`mask_home_with`] as its first pass,
//! with the home string its writer was configured with.
//!
//! # Why
//!
//! Raw mode keeps real paths, and a raw absolute repo path carries the OS
//! ACCOUNT NAME (`C:\Users\jane\…`, `/Users/jane/…`, `/home/jane/…`). An export
//! zip is mailed to a third party, so that name leaves the machine with it. The
//! path itself is what makes a raw log debuggable, so only the prefix is
//! replaced — `<home>\Repos\bonsai` still tells the reader which repo it is.
//!
//! # Cross-platform by resolution, NOT by pattern (user requirement 2026-09-11)
//!
//! The home directory is RESOLVED at runtime by [`super::home_resolve`] and
//! carried in `writer::WriterConfig::home_mask` — this module holds NO global
//! state, so the wiring is unit-testable and a writer either has a home string
//! or stamps `homeMasking: false` in its own session header. There is
//! deliberately no `C:\Users\*` pattern: that would miss a redirected profile, a
//! domain home on a network path, `/var/root`, a container `/root`, and every
//! non-English Windows install. One resolved string collapses all three layouts.
//!
//! # Matching rules
//!
//! * **Separator-insensitive** (`\` ≡ `/`) — a Windows path arrives both ways,
//!   including inside `file:///C:/Users/jane/…`.
//! * **Case-insensitive**, unconditionally — Windows and macOS are
//!   case-insensitive, and on Linux the cost of the extra match is masking
//!   *another* account's lookalike path, i.e. MORE redaction. Over-redaction is
//!   this module's deliberate failure direction (§7.1).
//! * **Boundary-anchored**: `/home/bob` must not match `/home/bobby` or
//!   `/home/bob.bak`. A match is accepted only at end-of-string or when the next
//!   character cannot continue a path segment.
//! * **Every occurrence** is replaced, not just the first.

use std::borrow::Cow;
use std::path::Path;

use super::redact::HOME_TOKEN;

/// Fold one character for comparison: `\` and `/` are the same separator, and
/// case never matters. `to_lowercase` can yield several chars for one input
/// (`İ`); the first is taken, which is a comparison approximation that can only
/// cost a match, never a false one on ASCII paths.
fn fold(c: char) -> char {
    match c {
        '\\' => '/',
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}

/// Can `c` continue a path SEGMENT? Used for the boundary check, so `/home/bob`
/// does not match inside `/home/bobby` or `/home/bob.bak`.
fn continues_segment(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '~' | '+')
}

/// Normalize a resolved home dir into the comparison form, or `None` when it is
/// unusable.
///
/// Two rejections, both because masking with that value would be WORSE than not
/// masking at all:
///
/// * a filesystem ROOT (`/`, `C:\`, `C:`) — it would mask the prefix of every
///   absolute path on the machine and turn a raw log into `<home>/...` noise;
/// * a SHARED parent of all accounts (`/home`, `/Users`, `C:\Users`) — masking
///   `C:\Users\jane\x` with it yields `<home>\jane\x`, i.e. the account name this
///   module exists to remove is still there, now behind a token claiming
///   otherwise. No real home directory is one of these, so refusing them costs
///   nothing and bounds a too-greedy [`super::home_resolve`] candidate.
pub(super) fn normalize_home(home: &Path) -> Option<String> {
    let folded: String = home.to_string_lossy().chars().map(fold).collect();
    let trimmed = folded.trim_end_matches('/');
    // After folding+trimming: "" was `/` or empty; "c:" was `C:\`.
    if trimmed.is_empty() || (trimmed.len() == 2 && trimmed.ends_with(':')) {
        return None;
    }
    if is_shared_account_parent(trimmed) {
        return None;
    }
    Some(trimmed.to_string())
}

/// Is `folded` the container of EVERY account's home rather than one home?
/// `/home`, `/users`, `<drive>:/users` — compared on the already-folded form, so
/// `C:\Users` and `c:/users` are the same value.
fn is_shared_account_parent(folded: &str) -> bool {
    let bare = match folded.strip_prefix('/') {
        Some(rest) => rest,
        // `c:/users` → `users`; anything without a drive prefix is unchanged.
        None => match folded.split_once(":/") {
            Some((drive, rest)) if drive.len() == 1 => rest,
            _ => folded,
        },
    };
    matches!(bare, "home" | "users")
}

/// Mask the home prefix in `s`.
///
/// `home` must already be folded (see [`normalize_home`]) and reaches here from
/// `writer::WriterConfig::home_mask` — there is deliberately NO process-global
/// fallback, so a writer that was never handed a home cannot silently mask
/// nothing while a reader assumes it did. Every test drives this directly with a
/// SYNTHETIC home, so the suite is portable across the three layouts.
///
/// Borrows on no match. Any field at least as long (in bytes) as the folded home
/// builds two `Vec<char>` first; shorter fields — most log fields are a command
/// name or a ref — take the O(1) reject and allocate nothing.
pub fn mask_home_with<'a>(s: &'a str, home: &str) -> Cow<'a, str> {
    if home.is_empty() || s.is_empty() {
        return Cow::Borrowed(s);
    }
    let home: Vec<char> = home.chars().collect();
    // Cheap O(1) reject before the per-char allocation: a match consumes exactly
    // `home.len()` CHARS of `s`, and every char is at least one byte, so a
    // shorter byte length cannot possibly contain one. Most log fields are short
    // strings (`"push"`, a branch name), so this is the common path and it keeps
    // the writer thread from allocating a `Vec<char>` per field. Longer fields DO
    // pay two `Vec<char>` builds even when they carry no path — a byte-level
    // prescan would not save them, since every POSIX home begins with `/`, which
    // ref names and messages contain routinely.
    if s.len() < home.len() {
        return Cow::Borrowed(s);
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out: Option<String> = None;
    let mut i = 0usize;
    while i < chars.len() {
        if chars.len() - i >= home.len() && matches_home_at(&chars, i, &home) {
            let buf = out.get_or_insert_with(|| chars[..i].iter().collect());
            buf.push_str(HOME_TOKEN);
            i += home.len();
            continue;
        }
        if let Some(buf) = out.as_mut() {
            buf.push(chars[i]);
        }
        i += 1;
    }
    match out {
        Some(buf) => Cow::Owned(buf),
        None => Cow::Borrowed(s),
    }
}

/// Does the folded `home` occur at `chars[at..]`, ending on a path boundary?
fn matches_home_at(chars: &[char], at: usize, home: &[char]) -> bool {
    if !home
        .iter()
        .enumerate()
        .all(|(k, h)| fold(chars[at + k]) == *h)
    {
        return false;
    }
    // End of string, a separator, or anything that cannot continue a segment
    // (`"`, `,`, `)`, whitespace …). A letter/digit/`.`/`-` means this is a
    // SIBLING directory (`/home/bobby`), not the home dir.
    match chars.get(at + home.len()) {
        None => true,
        Some(&c) => c == '/' || c == '\\' || !continues_segment(c),
    }
}

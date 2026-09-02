//! Frontmatter parse / serialize (§4).

use super::FrontmatterField;

/// Parse a `---`-delimited frontmatter fence at the very top of `raw` (§4.1).
///
/// Returns `(fields, body, complex)`:
/// - `fields`: flat `key: value` entries in file order (unknown keys preserved).
/// - `body`: everything after the closing fence's newline, verbatim; the whole
///   file when there is no (well-formed) fence.
/// - `complex`: `true` if any fence line is not a flat `key: scalar` (sequences,
///   nested maps, block scalars, indented lines) — the asset becomes read-only
///   (§4.3). Blank lines and `#` comments inside the fence are dropped.
pub fn parse_frontmatter(raw: &str) -> (Vec<FrontmatterField>, String, bool) {
    // Strip a leading UTF-8 BOM; normalize fence checks on `\n` (accept `\r\n`).
    let content = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);

    let first_end = content.find('\n');
    let first_line_raw = match first_end {
        Some(i) => &content[..i],
        None => content,
    };
    let first_line = first_line_raw.strip_suffix('\r').unwrap_or(first_line_raw);
    if first_line != "---" {
        // No opening fence (common for slash commands).
        return (Vec::new(), content.to_string(), false);
    }
    let Some(first_nl) = first_end else {
        // Bare `---` with no newline: no closing fence -> treat as no frontmatter.
        return (Vec::new(), content.to_string(), false);
    };

    let after_open = &content[first_nl + 1..];
    let mut fields = Vec::new();
    let mut complex = false;
    let mut body_start = None;
    let mut pos = 0usize;
    for seg in after_open.split_inclusive('\n') {
        let line = seg.strip_suffix('\n').unwrap_or(seg);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line == "---" {
            body_start = Some(pos + seg.len());
            break;
        }
        parse_fm_line(line, &mut fields, &mut complex);
        pos += seg.len();
    }

    match body_start {
        Some(bs) => (fields, after_open[bs..].to_string(), complex),
        // No closing fence -> not frontmatter; the whole file is the body.
        None => (Vec::new(), content.to_string(), false),
    }
}

/// Parse a single fence line into a field, or flag `complex` (§4.1 step 4).
fn parse_fm_line(line: &str, fields: &mut Vec<FrontmatterField>, complex: &mut bool) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        // Blank line or comment -> dropped (documented loss).
        return;
    }
    match parse_flat_kv(line) {
        Some((key, value)) if !is_block_scalar_indicator(&value) => {
            fields.push(FrontmatterField { key, value });
        }
        // A block-scalar indicator (`key: |`, `key: >`) OR any non-flat line
        // (`- item`, ` nested: x`, `key:value`) -> multi-line YAML we cannot
        // round-trip. Flag and skip.
        _ => *complex = true,
    }
}

/// Match `^([A-Za-z0-9_.-]+):(?: (.*))?$`: a flat `key:` or `key: value` line.
/// Returns `(key, value)`; `value` is verbatim text after `": "` (empty for a
/// bare `key:`). `None` for anything else (indented, sequence, `key:value`).
fn parse_flat_kv(line: &str) -> Option<(String, String)> {
    let colon = line.find(':')?;
    let key = &line[..colon];
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return None;
    }
    let rest = &line[colon + 1..];
    let value = if rest.is_empty() {
        String::new()
    } else {
        // `key:value` (no space after the colon) is not the flat inline form
        // -> `strip_prefix` returns None -> `?` bails to a complex/non-field line.
        rest.strip_prefix(' ')?.to_string()
    };
    Some((key.to_string(), value))
}

/// A YAML block-scalar indicator value (`|`, `>`, `|-`, `>+`, `|2`, …) signals
/// a multi-line scalar this editor cannot round-trip -> treat as complex.
fn is_block_scalar_indicator(value: &str) -> bool {
    let v = value.trim();
    let mut chars = v.chars();
    match chars.next() {
        Some('|') | Some('>') => chars.all(|c| matches!(c, '+' | '-') || c.is_ascii_digit()),
        _ => false,
    }
}

/// Serialize flat `frontmatter` + `body` back to file bytes (§4.2). Empty
/// frontmatter -> body only, no fence. Values are written verbatim (opaque
/// scalars, no auto-quoting). The ONLY normalization is ensuring the output ends
/// with exactly one trailing `\n` (appended if missing; interior whitespace is
/// untouched).
pub fn serialize_asset(frontmatter: &[FrontmatterField], body: &str) -> String {
    let mut out = String::new();
    if !frontmatter.is_empty() {
        out.push_str("---\n");
        for f in frontmatter {
            if f.value.is_empty() {
                out.push_str(&f.key);
                out.push_str(":\n");
            } else {
                out.push_str(&f.key);
                out.push_str(": ");
                out.push_str(&f.value);
                out.push('\n');
            }
        }
        out.push_str("---\n");
    }
    out.push_str(body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

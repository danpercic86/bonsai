use super::*;
use crate::git::exec::GitOutput;

#[test]
fn git_raw_date_formats_offset() {
    assert_eq!(git_raw_date(&git2::Time::new(1_000_000_000, 120)), "1000000000 +0200");
    assert_eq!(git_raw_date(&git2::Time::new(0, -300)), "0 -0500");
    assert_eq!(git_raw_date(&git2::Time::new(42, 0)), "42 +0000");
}

// ---- verify: build_verify_args / parse / map (pure, git-free) ---------
#[test]
fn build_verify_args_prefix_and_drops_non_hex() {
    let good = "a".repeat(40);
    let upper = "A".repeat(40); // uppercase is still 40-hex
    let mixed = "0123456789abcdef0123456789abcdef01234567".to_string();
    let oids = vec![
        good.clone(),
        "not-hex".to_string(),
        "#".to_string(),
        String::new(),
        "b".repeat(39), // too short
        upper.clone(),
        mixed.clone(),
    ];
    let args = build_verify_args(&oids);
    assert_eq!(args[0], "log");
    assert_eq!(args[1], "--no-walk=unsorted");
    assert_eq!(args[2], "--ignore-missing");
    assert_eq!(args[3], "--format=%H%x1f%G?%x1f%GS%x1f%GK");
    assert_eq!(&args[VERIFY_ARG_PREFIX..], &[good, upper, mixed][..], "only 40-hex, in order");
}

#[test]
fn build_verify_args_caps_at_max_batch() {
    let oids = vec!["a".repeat(40); MAX_VERIFY_BATCH + 50];
    assert_eq!(build_verify_args(&oids).len(), VERIFY_ARG_PREFIX + MAX_VERIFY_BATCH);
}

#[test]
fn parse_verify_output_splits_maps_and_empties_to_none() {
    let us = '\u{1f}';
    let stdout = format!(
        "{a}{us}G{us}Ada Lovelace{us}KEY1\n{b}{us}N{us}{us}\n{c}{us}U{us}ssh-user{us}SHA256:xyz\n",
        a = "1".repeat(40),
        b = "2".repeat(40),
        c = "3".repeat(40),
    );
    let v = parse_verify_output(&stdout);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].oid, "1".repeat(40));
    assert_eq!(v[0].status, VerifyStatus::Good);
    assert_eq!(v[0].signer.as_deref(), Some("Ada Lovelace"));
    assert_eq!(v[0].key.as_deref(), Some("KEY1"));
    assert_eq!(v[1].status, VerifyStatus::Unsigned);
    assert_eq!(v[1].signer, None, "empty %GS ⇒ None");
    assert_eq!(v[1].key, None, "empty %GK ⇒ None");
    assert_eq!(v[2].status, VerifyStatus::GoodUnknown);
    assert_eq!(v[2].signer.as_deref(), Some("ssh-user"));
    assert_eq!(v[2].key.as_deref(), Some("SHA256:xyz"));
    // blank / oid-less lines skipped.
    assert!(parse_verify_output("\n\n").is_empty());
}

#[test]
fn map_status_code_full_table() {
    use VerifyStatus::*;
    assert_eq!(map_status_code('G'), Good);
    assert_eq!(map_status_code('U'), GoodUnknown);
    assert_eq!(map_status_code('B'), Bad);
    assert_eq!(map_status_code('X'), Expired);
    assert_eq!(map_status_code('Y'), ExpiredKey);
    assert_eq!(map_status_code('R'), Revoked);
    assert_eq!(map_status_code('E'), CannotCheck);
    assert_eq!(map_status_code('N'), Unsigned);
    assert_eq!(map_status_code('?'), Unsigned, "unrecognized ⇒ Unsigned");
}

// ---- verify_commits: empty / all-invalid never spawns (P58 §8) --------
/// Panics if invoked — proves `verify_commits` does not spawn git when no
/// oid survives 40-hex validation. (The wholesale-failure → CannotCheck
/// degrade is covered hermetically in `tests/signing_cli.rs`.)
struct PanicExec;
impl GitExec for PanicExec {
    fn exec(
        &self,
        _args: &[&str],
        _cwd: &Path,
        _stdin: Option<&[u8]>,
        _env: &[(&str, &str)],
    ) -> Result<GitOutput, AppError> {
        panic!("verify_commits must not spawn git for an empty/all-invalid set");
    }
}

#[test]
fn verify_commits_empty_or_all_invalid_does_not_spawn() {
    assert!(verify_commits(&PanicExec, Path::new("."), &[]).unwrap().verifications.is_empty());
    let junk = vec!["not-hex".to_string(), "#".to_string(), "b".repeat(39)];
    assert!(verify_commits(&PanicExec, Path::new("."), &junk).unwrap().verifications.is_empty());
}

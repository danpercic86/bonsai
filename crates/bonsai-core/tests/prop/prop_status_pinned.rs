//! Pinned regression cases for the T5 status property suite.
//!
//! These are the three inputs that used to live in
//! `prop_status.proptest-regressions` as `cc` seeds. That file was DELETED, for
//! two reasons:
//!
//! 1. COVERAGE. proptest keys its persistence file per SOURCE FILE, not per test
//!    fn, so after the op-count banding all four bands replayed all three seeds
//!    (3 replays -> 12). Worse, a `cc` seed only stores an RNG seed: it
//!    regenerates values through the CURRENT strategy. The strategy changed when
//!    op kind 5 (fs-rename) was added back after F-T5-3 was fixed, so the seeds
//!    no longer reproduce the inputs recorded in their own `# shrinks to`
//!    comments — they had degraded into three extra random cases wearing a
//!    regression label. The recorded inputs below are transcribed VERBATIM from
//!    those comments, so the coverage is now exact and strategy-independent.
//! 2. COST. Replaying the seeds cost ~25% of every band (4.30s/band with the
//!    file vs 3.30s without) for coverage that was not the recorded coverage.
//!
//! Each case runs through the SAME `run_case` helper as the randomized bands, so
//! setup, mutation and the porcelain-oracle observation are byte-identical; only
//! the input is fixed instead of generated.

use super::{run_case, RawOp};
use crate::prop_common::common;

/// Run one pinned case: same body as a band case, fixed input.
///
/// The path selector is recorded as a `u64` (proptest generated a `usize` on a
/// 64-bit host). `as usize` is exact on 64-bit; on a hypothetical 32-bit target
/// it truncates, which only changes WHICH known path an op picks — the porcelain
/// oracle assertion stays valid either way.
fn check(initial: &[(&str, u32)], ops: &[(u8, u64, u32, &str)]) {
    if !common::have_git() {
        eprintln!("skipping: `git` CLI not found on PATH");
        return;
    }
    let initial: Vec<(String, u32)> =
        initial.iter().map(|(p, s)| ((*p).to_string(), *s)).collect();
    let ops: Vec<RawOp> = ops
        .iter()
        .map(|(kind, sel, seed, name)| (*kind, *sel as usize, *seed, (*name).to_string()))
        .collect();
    let (read, oracle) = run_case(&initial, &ops);
    assert_eq!(read, oracle, "read_status disagrees with git porcelain oracle");
}

/// Recorded verbatim from the `# shrinks to` comment of seed
/// `cc 13f95da30185206c9d086bee5369e1e12286c085b12d2c3fa37acaa99b812f25`:
/// 4 initial files, 10 ops.
#[test]
fn pinned_regression_13f95da3_matches_porcelain() {
    check(
        &[
            ("ruqwx", 1911788117),
            ("alm", 700086318),
            ("ximls", 2396754153),
            ("u", 2988843570),
        ],
        &[
            (0, 18203140971037284238, 926914791, "cp/rnhn"),
            (0, 10775616892291283855, 3111048552, "ugd/e"),
            (0, 11083742540702776680, 2661298741, "odbxw/dh"),
            (1, 1606327690994615240, 391441276, "nsanm"),
            (2, 13998801983347458184, 4155359291, "ah"),
            (5, 2184314842576123877, 3727600457, "v/u"),
            (1, 1449100855286681087, 2740716679, "jtdz"),
            (1, 17864374445535088156, 1201773604, "tvx/h"),
            (1, 4006106442088580475, 2807008583, "ecc/jlj"),
            (3, 3940698385498079931, 291430655, "ak"),
        ],
    );
}

/// Recorded verbatim from the `# shrinks to` comment of seed
/// `cc acc610652b5dd17c6fdc3ecb8f53c25796c8767eef856627a34c62c5328bb1f7`:
/// 2 initial files, 7 ops.
#[test]
fn pinned_regression_acc61065_matches_porcelain() {
    check(
        &[
            ("lct/kcecx", 3798304349),
            ("fuqk/vf", 2423275639),
        ],
        &[
            (3, 3171494165003405745, 4107985199, "zkm"),
            (1, 13848031023464332986, 2070612901, "fpy"),
            (1, 5348270848812959325, 2257737856, "y/hlnl"),
            (0, 7452100959276159898, 1566049049, "blpe"),
            (4, 14691069584409737351, 1270308094, "jbzr/uq"),
            (3, 7598700570601327154, 1590088738, "l"),
            (4, 8156402955417471662, 2875003128, "fy/exq"),
        ],
    );
}

/// Recorded verbatim from the `# shrinks to` comment of seed
/// `cc 7092b6a8ad052d40b3a382fbaf1450dde7bd1d77ac401b9e7af9a23db3965a5a`:
/// 6 initial files, 7 ops.
#[test]
fn pinned_regression_7092b6a8_matches_porcelain() {
    check(
        &[
            ("js", 830085073),
            ("y/hdq", 750409876),
            ("zsl/ozrm", 2913031937),
            ("io", 3268388894),
            ("e/pwn", 3853189158),
            ("ap", 2793669611),
        ],
        &[
            (1, 4217376688305137722, 1942386613, "gnry"),
            (4, 3842487655791072095, 1278675183, "bp"),
            (1, 10402710393222198989, 1229272639, "mf/r"),
            (0, 13825244896185113770, 2802026316, "sgno"),
            (3, 5327597818003222344, 1785525724, "yeqjj/sd"),
            (5, 3279438120187805109, 3677744552, "gso/vvtix"),
            (1, 6214002843715606778, 2793541552, "lggl"),
        ],
    );
}

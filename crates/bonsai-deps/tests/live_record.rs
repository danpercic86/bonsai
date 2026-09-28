//! P125d live recorder `[LOW-4]` — IGNORED; never part of the default
//! (offline) run. It makes real HTTPS calls to api.deps.dev and
//! api.osv.dev for the well-known public packages of `common::corpus` only,
//! through the production `ReqwestTransport` and the full `run_check`
//! pipeline, and writes the responses to `tests/fixtures/remote/`.
//!
//! Re-record (Git Bash, from the repo root):
//!
//! ```text
//! rm -rf crates/bonsai-deps/tests/fixtures/remote
//! cargo test -p bonsai-deps --test live_record -- --ignored --nocapture
//! ```
//!
//! then re-run `cargo nextest run -p bonsai-deps` and update the §0.1
//! live-probe rows if a recorded fact changed.

mod common;

use bonsai_deps::registry_ctx::FakeHostEnv;
use bonsai_deps::remote::{RecordingTransport, ReqwestTransport};
use bonsai_deps::{run_check, CancelToken, CheckCtx};

use common::corpus::{corpus_repo, fixtures_dir};

#[test]
#[ignore = "live network: records deps.dev / OSV fixtures (manual step)"]
fn record_live_fixtures() {
    let real = match ReqwestTransport::new() {
        Ok(t) => t,
        Err(e) => panic!("transport: {e:?}"),
    };
    let dir = fixtures_dir();
    let rec = RecordingTransport::new(&real, &dir);
    let r = corpus_repo();
    let cancel = CancelToken::new();
    let ctx = CheckCtx {
        transport: &rec,
        env: &FakeHostEnv::new(),
        cancel: &cancel,
        progress: &|d, t| eprintln!("progress {d}/{t}"),
        prev: None,
        now_ms: 0,
    };
    match run_check(&r.wd, "live-record", &ctx) {
        Ok(c) => {
            for g in &c.report.groups {
                for row in &g.rows {
                    eprintln!(
                        "{} {} cur={:?} latest={:?} vulns={}",
                        g.manifest_path,
                        row.name,
                        row.current,
                        row.latest,
                        row.vulns.len()
                    );
                }
            }
        }
        Err(e) => panic!("live check failed: {e:?}"),
    }
}

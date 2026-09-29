//! Test doubles for the `deps` tests. src-tauri depends on `bonsai-deps`
//! WITHOUT `test-support`, so the crate's `FakeTransport` / `FakeHostEnv`
//! are not visible here; these are the minimal local equivalents. Fixture
//! repos are built with git2 (this directory is scanned by the no-process
//! test, so no `git` CLI).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use bonsai_deps::registry_ctx::{HostEnv, UserConfig, UserRead};
use bonsai_deps::remote::{OutboundRequest, RawResponse, TransportError};
use bonsai_deps::Transport;

use super::{DepsChangedKind, DepsChangedPayload, DepsDeps, DepsEmitFn, DepsState};
use crate::settings::DepTracker;

/// Hermetic `HostEnv`: only what the test configured, no IO.
#[derive(Default)]
pub(super) struct TestEnv {
    pub user: HashMap<UserConfig, String>,
}

impl HostEnv for TestEnv {
    fn var(&self, _name: &str) -> Option<String> {
        None
    }
    fn var_present_ci(&self, _name: &str) -> bool {
        false
    }
    fn vars_with_prefix_ci(&self, _p: &str, _keep: fn(&str) -> bool) -> Vec<(String, String)> {
        Vec::new()
    }
    fn read_user_file(&self, which: UserConfig) -> UserRead {
        self.user
            .get(&which)
            .map_or(UserRead::Absent, |s| UserRead::Content(s.clone()))
    }
    fn read_user_files(&self, which: UserConfig) -> Vec<UserRead> {
        vec![self.read_user_file(which)]
    }
    fn read_above_workdir(&self, _dir: &Path, _file_name: &str) -> UserRead {
        UserRead::Absent
    }
}

/// Counts every send that reached it (i.e. passed the gate) and answers
/// 404 (deps.dev ⇒ Unknown). While `hold` is set, each send blocks until
/// `release` — the "6 workers blocked" shape of E4 AC12.
#[derive(Default)]
pub(super) struct TestTransport {
    pub sends: AtomicUsize,
    pub in_flight: AtomicUsize,
    gate: Mutex<bool>,
    cv: Condvar,
}

impl TestTransport {
    pub fn holding() -> Arc<Self> {
        let t = Arc::new(Self::default());
        *t.gate.lock().expect("gate") = true;
        t
    }
    pub fn hold(&self) {
        *self.gate.lock().expect("gate") = true;
    }
    pub fn release(&self) {
        *self.gate.lock().expect("gate") = false;
        self.cv.notify_all();
    }
    pub fn sends(&self) -> usize {
        self.sends.load(Ordering::SeqCst)
    }
}

impl Transport for TestTransport {
    fn send(&self, _req: &OutboundRequest, _cap: usize) -> Result<RawResponse, TransportError> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let mut held = self.gate.lock().expect("gate");
        while *held {
            held = self.cv.wait(held).expect("gate");
        }
        drop(held);
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(RawResponse {
            status: 404,
            body: Vec::new(),
        })
    }
}

/// Collected `deps-changed` payloads.
#[derive(Clone, Default)]
pub(super) struct Events(pub Arc<Mutex<Vec<DepsChangedPayload>>>);

impl Events {
    pub fn emitter(&self) -> DepsEmitFn {
        let sink = Arc::clone(&self.0);
        Arc::new(move |p| sink.lock().expect("events").push(p))
    }
    pub fn all(&self) -> Vec<DepsChangedPayload> {
        self.0.lock().expect("events").clone()
    }
    pub fn kinds(&self, repo_id: &str) -> Vec<DepsChangedKind> {
        self.all()
            .into_iter()
            .filter(|p| p.repo_id == repo_id)
            .map(|p| p.kind)
            .collect()
    }
}

/// Polls `f` for up to 10 s.
pub(super) fn wait_until(what: &str, f: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Scratch dirs live under `D:\Data\Temp` on Windows (global temp rule).
pub(super) fn scratch() -> tempfile::TempDir {
    let root = if cfg!(windows) {
        PathBuf::from("D:\\Data\\Temp\\bonsai-scratch")
    } else {
        std::env::temp_dir().join("bonsai-scratch")
    };
    std::fs::create_dir_all(&root).expect("scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-deps-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// A git2 repo at `<dir>/repo` holding `files` (added to the index);
/// returns its canonical workdir, which is also its repoId.
pub(super) fn repo(dir: &Path, files: &[(&str, &str)]) -> PathBuf {
    let wd = dir.join("repo");
    std::fs::create_dir_all(&wd).expect("workdir");
    let repo = git2::Repository::init(&wd).expect("init");
    let mut index = repo.index().expect("index");
    for (rel, body) in files {
        let p = wd.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("dirs");
        }
        std::fs::write(&p, body).expect("write fixture file");
        index.add_path(Path::new(rel)).expect("add");
    }
    index.write().expect("write index");
    std::fs::canonicalize(&wd).expect("canonical workdir")
}

/// `package.json` with `n` public npm deps (ranges only, no lock ⇒ only
/// deps.dev lookups, no OSV batch).
pub(super) fn npm_manifest(n: usize) -> String {
    let deps: Vec<String> = (0..n).map(|i| format!("\"pkg-{i}\": \"^1.0.0\"")).collect();
    format!(
        "{{\"name\":\"app\",\"dependencies\":{{{}}}}}",
        deps.join(",")
    )
}

pub(super) fn id(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

pub(super) struct Harness {
    pub state: DepsState,
    pub transport: Arc<TestTransport>,
    pub events: Events,
    pub data_root: PathBuf,
}

pub(super) fn harness(
    dir: &Path,
    enabled: bool,
    transport: Arc<TestTransport>,
    env: TestEnv,
    deadline: Duration,
) -> Harness {
    let events = Events::default();
    let data_root = dir.join("appdata");
    let state = DepsState::with_deadline(
        DepsDeps {
            cfg: DepTracker {
                enabled,
                interval_hours: 12,
            },
            transport: transport.clone(),
            env: Arc::new(env),
            data_root: Some(data_root.clone()),
            emit: events.emitter(),
        },
        deadline,
    );
    Harness {
        state,
        transport,
        events,
        data_root,
    }
}

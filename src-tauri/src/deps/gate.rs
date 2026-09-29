//! `GatedTransport` (contract §7.2, AC1 / AC12, `[LOW-1]`): the ONLY
//! transport a check ever sees. It refuses with `TransportError::Gated` and
//! performs no I/O unless the tracker is enabled AND the run's epoch is still
//! the current one and the run is not cancelled. One instance per run (the gate needs the run's epoch), all
//! wrapping the one shared raw transport.
//!
//! After a disable the gate closes for every later `send`; a request already
//! past the gate (at most `DEPSDEV_PARALLEL` = 6, each bounded by the 30 s
//! request timeout) may still complete — its result is discarded by the
//! epoch check in `exec::finish`.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use bonsai_deps::remote::{OutboundRequest, RawResponse, TransportError};
use bonsai_deps::{CancelToken, Transport};

use super::DepsState;

pub(crate) struct GatedTransport {
    inner: Arc<dyn Transport>,
    state: DepsState,
    run_epoch: u64,
    /// `[LOW-E3]` the run's token: a worker past its deadline (or a
    /// cancelled run) cannot start one more request.
    token: CancelToken,
}

impl GatedTransport {
    pub(crate) fn new(state: &DepsState, run_epoch: u64, token: CancelToken) -> Self {
        GatedTransport {
            inner: Arc::clone(&state.transport),
            state: state.clone(),
            run_epoch,
            token,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.state.enabled.load(Ordering::SeqCst)
            && self.state.epoch.load(Ordering::SeqCst) == self.run_epoch
            && !self.token.is_cancelled()
    }
}

impl Transport for GatedTransport {
    fn send(&self, req: &OutboundRequest, cap: usize) -> Result<RawResponse, TransportError> {
        if !self.is_open() {
            return Err(TransportError::Gated);
        }
        self.inner.send(req, cap)
    }
}

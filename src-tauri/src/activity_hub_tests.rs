//! P123 R3-R8: hub routing, ids, idempotent unsubscribe, StrictMode sequence,
//! prune-on-send-failure.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bonsai_core::git::activity::{
    ActivityEmitter, GitActivityCategory, GitActivityEvent, GitPhaseKind,
};
use tauri::ipc::Channel;

use super::*;

/// A channel that counts the messages it receives.
fn counting() -> (Channel<GitActivityEvent>, Arc<AtomicUsize>) {
    let n = Arc::new(AtomicUsize::new(0));
    let sink = Arc::clone(&n);
    let ch = Channel::new(move |_| {
        sink.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    (ch, n)
}

/// One real `started` event (the only way to build a `GitActivityEvent`).
fn event() -> GitActivityEvent {
    let out = Arc::new(std::sync::Mutex::new(None));
    let slot = Arc::clone(&out);
    let em = ActivityEmitter::new(
        "git-hub-test".into(),
        None,
        Box::new(move |ev| *slot.lock().expect("slot") = Some(ev)),
    );
    em.started(GitActivityCategory::Fetch, GitPhaseKind::Preparing);
    let ev = out.lock().expect("slot").take();
    ev.expect("started emitted")
}

fn got(n: &AtomicUsize) -> usize {
    n.load(Ordering::SeqCst)
}

/// R3: distinct ids; the count tracks subscribe/unsubscribe.
#[test]
fn subscribe_returns_distinct_ids_and_count_tracks() {
    let hub = GitActivityHub::default();
    let a = hub.subscribe(Some("a".into()), counting().0);
    let b = hub.subscribe(Some("a".into()), counting().0);
    assert_ne!(a, b);
    assert!(a.starts_with("gas-") && b.starts_with("gas-"));
    assert_eq!(hub.subscriber_count(), 2);
    assert!(hub.unsubscribe(&a));
    assert_eq!(hub.subscriber_count(), 1);
    assert!(hub.unsubscribe(&b));
    assert_eq!(hub.subscriber_count(), 0);
}

/// R4: a scoped emit reaches only its repo's subs (+ wildcard); an unscoped
/// emit reaches everyone.
#[test]
fn emit_routes_by_repo() {
    let hub = GitActivityHub::default();
    let (a1, n_a1) = counting();
    let (a2, n_a2) = counting();
    let (b, n_b) = counting();
    let (w, n_w) = counting();
    hub.subscribe(Some("a".into()), a1);
    hub.subscribe(Some("a".into()), a2);
    hub.subscribe(Some("b".into()), b);
    hub.subscribe(None, w);

    hub.emit_to(Some("a"), event());
    assert_eq!((got(&n_a1), got(&n_a2), got(&n_b), got(&n_w)), (1, 1, 0, 1));

    hub.emit_to(None, event());
    assert_eq!((got(&n_a1), got(&n_a2), got(&n_b), got(&n_w)), (2, 2, 1, 2));

    hub.emit_to(Some("b"), event());
    assert_eq!((got(&n_a1), got(&n_a2), got(&n_b), got(&n_w)), (2, 2, 2, 3));
}

/// R5: `is_active_for` is per route; `None` = any sub.
#[test]
fn is_active_for_is_per_route() {
    let hub = GitActivityHub::default();
    assert!(!hub.is_active_for(None));
    assert!(!hub.is_active());
    hub.subscribe(Some("a".into()), counting().0);
    assert!(hub.is_active_for(Some("a")));
    assert!(!hub.is_active_for(Some("b")));
    assert!(hub.is_active_for(None));
    assert!(hub.is_active());

    let scoped_b = ScopedActivityHub::new(hub.clone(), Some("b".into()));
    assert!(!scoped_b.is_active());
    assert_eq!(scoped_b.repo_id(), Some("b"));
    assert!(ScopedActivityHub::new(hub, None).is_active());
}

/// R6: unsubscribe removes exactly one sub; repeat / unknown ids are no-ops.
#[test]
fn unsubscribe_is_exact_and_idempotent() {
    let hub = GitActivityHub::default();
    let (c1, n1) = counting();
    let (c2, n2) = counting();
    let s1 = hub.subscribe(Some("a".into()), c1);
    hub.subscribe(Some("a".into()), c2);
    assert!(hub.unsubscribe(&s1));
    hub.emit_to(Some("a"), event());
    assert_eq!((got(&n1), got(&n2)), (0, 1));

    assert!(!hub.unsubscribe(&s1), "second unsubscribe is a no-op");
    assert!(!hub.unsubscribe("gas-999"), "unknown id is a no-op");
    assert_eq!(hub.subscriber_count(), 1);
}

/// R7: StrictMode — mount, cleanup, mount ⇒ exactly one live subscription and
/// one delivery per emit.
#[test]
fn strict_mode_sequence_leaves_one_subscription() {
    let hub = GitActivityHub::default();
    let (c1, n1) = counting();
    let (c2, n2) = counting();
    let s1 = hub.subscribe(Some("a".into()), c1);
    let _s2 = hub.subscribe(Some("a".into()), c2);
    hub.unsubscribe(&s1);
    assert_eq!(hub.subscriber_count(), 1);
    hub.emit_to(Some("a"), event());
    assert_eq!(got(&n1) + got(&n2), 1);
}

/// R8: a channel whose send errors is pruned on emit (safety net kept);
/// a non-matching failing channel is not touched.
#[test]
fn failing_channel_is_pruned_on_emit() {
    let hub = GitActivityHub::default();
    let failing = || Channel::new(|_| Err(tauri::Error::FailedToReceiveMessage));
    hub.subscribe(Some("a".into()), failing());
    hub.subscribe(Some("b".into()), failing());
    let (ok, n_ok) = counting();
    hub.subscribe(Some("a".into()), ok);
    hub.emit_to(Some("a"), event());
    assert_eq!(got(&n_ok), 1);
    assert_eq!(hub.subscriber_count(), 2, "only a's failing channel pruned");
    hub.emit_to(None, event());
    assert_eq!(hub.subscriber_count(), 1);
}

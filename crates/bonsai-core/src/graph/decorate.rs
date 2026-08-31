//! Pure ref-decoration rewrite for cached graph chunks (P86 B1 HitRedecorate).
//!
//! Split out of `stream.rs` (file-size discipline): the walk lives there, the
//! decoration-only rewrite lives here. No repo, no revwalk.

use super::{GraphChunk, RefMap};

/// Pure, no repo, no revwalk (P86 B1 HitRedecorate). Rewrites the ref decoration
/// on an already-walked, cached [`GraphChunk`] stream from a FRESH [`RefMap`]:
/// the walk itself — rows, lanes, edges, ordering — is byte-identical, only the
/// pills change (e.g. a branch created/renamed/deleted at an existing commit, or
/// HEAD moving onto an already-walked commit). Each node's `refs` is replaced by
/// the fresh labels for its oid (empty when it has none); `Meta.head_oid` and
/// `Done.head_index` are recomputed from `head`. O(nodes), no object reads.
///
/// A node id that fails to parse as an oid (never happens for a real walk) is
/// given empty refs and skipped for head resolution — a tolerated no-op, never a
/// wrong pill.
pub fn redecorate_chunks(chunks: &mut [GraphChunk], refs: &RefMap, head: Option<git2::Oid>) {
    let head_hex = head.map(|h| h.to_string());
    let mut head_index: Option<u32> = None;
    for chunk in chunks.iter_mut() {
        match chunk {
            GraphChunk::Meta { head_oid, .. } => *head_oid = head_hex.clone(),
            GraphChunk::Batch {
                start_row, nodes, ..
            } => {
                for (i, node) in nodes.iter_mut().enumerate() {
                    match git2::Oid::from_str(&node.id) {
                        Ok(oid) => {
                            node.refs = refs.get(&oid).cloned().unwrap_or_default();
                            if head == Some(oid) {
                                head_index = Some(*start_row + i as u32);
                            }
                        }
                        Err(_) => node.refs = Vec::new(),
                    }
                }
            }
            GraphChunk::Done { head_index: hi, .. } => *hi = head_index,
        }
    }
}

/// P86 B1 (AC-B1a): `redecorate_chunks` rewrites ONLY the pills + head; the walk
/// math (lanes, edges, ordering, row extents) is byte-identical. A node present
/// in the fresh `RefMap` gets its pills; one absent gets emptied; and
/// `Meta.head_oid` / `Done.head_index` are recomputed from `head`.
#[cfg(test)]
mod tests {
    use super::redecorate_chunks;
    use crate::graph::{GraphChunk, GraphStreamEdge, RefKind, RefLabel, StreamNode};
    use std::collections::HashMap;

    #[test]
    fn redecorate_rewrites_pills_and_head_only() {
        let id_a = "a".repeat(40);
        let id_b = "b".repeat(40);
        let oid_a = git2::Oid::from_str(&id_a).expect("oid a");
        let oid_b = git2::Oid::from_str(&id_b).expect("oid b");

        let mut chunks = vec![
            GraphChunk::Meta {
                total: Some(2),
                head_oid: None,
            },
            GraphChunk::Batch {
                start_row: 0,
                lane_count_so_far: 2,
                nodes: vec![
                    StreamNode {
                        id: id_a.clone(),
                        lane: 0,
                        refs: vec![],
                        summary: "A".into(),
                        author: "x".into(),
                        ts: 2,
                        committer_ts: 2,
                    },
                    StreamNode {
                        id: id_b.clone(),
                        lane: 1,
                        // A STALE pill that must be cleared (oid_b absent below).
                        refs: vec![RefLabel {
                            name: "stale".into(),
                            kind: RefKind::LocalBranch,
                            is_head: false,
                        }],
                        summary: "B".into(),
                        author: "x".into(),
                        ts: 1,
                        committer_ts: 1,
                    },
                ],
                edges: vec![GraphStreamEdge {
                    from: 0,
                    to: 1,
                    lane: 0,
                    ord: 0,
                }],
            },
            GraphChunk::Done {
                total_rows: 2,
                lane_count: 2,
                head_index: None,
                truncated: false,
            },
        ];

        let mut refs: HashMap<git2::Oid, Vec<RefLabel>> = HashMap::new();
        refs.insert(
            oid_a,
            vec![RefLabel {
                name: "main".into(),
                kind: RefKind::LocalBranch,
                is_head: true,
            }],
        );
        // oid_b intentionally absent → its stale pill must be cleared.
        let _ = oid_b;

        redecorate_chunks(&mut chunks, &refs, Some(oid_a));

        match &chunks[0] {
            GraphChunk::Meta { head_oid, .. } => {
                assert_eq!(head_oid.as_deref(), Some(id_a.as_str()))
            }
            _ => panic!("chunk 0 must be Meta"),
        }
        match &chunks[2] {
            GraphChunk::Done { head_index, .. } => assert_eq!(*head_index, Some(0)),
            _ => panic!("chunk 2 must be Done"),
        }
        match &chunks[1] {
            GraphChunk::Batch {
                nodes,
                edges,
                lane_count_so_far,
                start_row,
            } => {
                // Pills rewritten from the fresh RefMap ...
                assert_eq!(
                    nodes[0].refs,
                    vec![RefLabel {
                        name: "main".into(),
                        kind: RefKind::LocalBranch,
                        is_head: true,
                    }]
                );
                assert!(nodes[1].refs.is_empty(), "absent oid → pills cleared");
                // ... walk math untouched.
                assert_eq!(nodes[0].lane, 0);
                assert_eq!(nodes[1].lane, 1);
                assert_eq!(*lane_count_so_far, 2);
                assert_eq!(*start_row, 0);
                assert_eq!(edges.len(), 1);
                assert_eq!(
                    (edges[0].from, edges[0].to, edges[0].lane, edges[0].ord),
                    (0, 1, 0, 0)
                );
            }
            _ => panic!("chunk 1 must be Batch"),
        }
    }
}

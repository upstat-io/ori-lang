//! SCC-based cycle detection over `UserBurdenSpec` type-id edges.
//!
//! Inputs: a corpus of `(Idx, &UserBurdenSpec)` pairs covering every
//! user-type / monomorphized-generic that has a registered burden. Edges are
//! reachable type-Idx references per `UserBurdenSpec` fields:
//!
//! - `owned_fields[i].field_type`
//! - `borrowed_fields[i].field_type` (only when the borrow extends the
//!   parent lifetime — same SCC contribution; conservatively included today
//!   because `Tag::Borrowed` is target-only, so any borrowed-field surfaced
//!   here participates in the parent's drop walk)
//! - `element_burden` when `Some(idx)` (List/Map/Set element type Idx)
//! - `variant_burdens[i].retained_owned[j].field_type` (enum variant's
//!   retained owned fields)
//!
//! Output: `Vec<Vec<Idx>>` SCC partition produced by Tarjan's iterative
//! algorithm (matches `petgraph::algo::tarjan_scc` shape — single DFS,
//! `O(V+E)`).
//!
//! Decision rule for `compiled_drop`:
//! `compiled_drop = Some(_) iff (in non-singleton SCC) OR (self-loop) OR
//! (user_drop = Some(_))`.
//!
//! Spec: Annex E §AIMS — compiled drop-glue per type is a typed pre-pass
//! input feeding lattice-driven analysis. SCC detection at registration
//! time wires the cycle-safe drop walk before Phase 5 emission.

use core::num::NonZeroU32;

use ori_registry::burden::FnSym;
use rustc_hash::FxHashMap;

use crate::registry::burden::UserBurdenSpec;
use crate::Idx;

/// Sentinel for "node has not been assigned a DFS index yet" in Tarjan's
/// algorithm. Equivalent to `-1` in the reference implementation; using
/// `u32::MAX` keeps the indices unsigned and avoids signed/unsigned casts
/// (clippy denies `usize as i64` on 64-bit targets per workspace lints).
const UNVISITED: u32 = u32::MAX;

/// Explicit DFS frame for iterative Tarjan SCC.
///
/// Lifted to module scope so the inner loop body does not declare a nested
/// item (clippy's `items_after_statements` rule).
struct DfsFrame {
    node: usize,
    next_child: usize,
}

/// Compute the SCC partition over the given burden corpus.
///
/// Each entry in the corpus is a `(type_idx, &UserBurdenSpec)` pair. The
/// returned partition is a `Vec<Vec<Idx>>` where each inner `Vec` is one
/// SCC; intra-SCC order is the iterative-Tarjan stack order (deterministic
/// per input). The order of SCCs in the outer `Vec` is reverse-topological
/// (sinks first, sources last) — matching `petgraph::algo::tarjan_scc`.
///
/// Types referenced by edges but absent from the corpus (e.g., primitive
/// `Idx::INT`, or a borrowed `field_type` whose burden is empty) are
/// excluded from the partition — they cannot participate in a cycle and
/// require no compiled drop function.
///
/// The algorithm is iterative to avoid host-stack overflow on deeply
/// nested types (per `ori_stack` discipline).
#[must_use]
pub fn compute_scc_partition(corpus: &[(Idx, &UserBurdenSpec)]) -> Vec<Vec<Idx>> {
    // Build adjacency: for each node in the corpus, collect every outgoing
    // edge target. Edges to nodes outside the corpus are dropped (only
    // in-corpus nodes can participate in cycles within this analysis).
    let mut node_index: FxHashMap<Idx, usize> = FxHashMap::default();
    for (i, (idx, _)) in corpus.iter().enumerate() {
        node_index.insert(*idx, i);
    }

    let n = corpus.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, (_, spec)) in corpus.iter().enumerate() {
        collect_edges(spec, |target: Idx| {
            if let Some(&j) = node_index.get(&target) {
                adj[i].push(j);
            }
        });
        // Deterministic intra-SCC ordering: sort adjacency by node index.
        // Tarjan's correctness does not depend on iteration order, but
        // tests pin partition contents and benefit from a stable layout.
        adj[i].sort_unstable();
        adj[i].dedup();
    }

    // Iterative Tarjan SCC.
    let sccs_by_node = tarjan_scc(&adj);

    sccs_by_node
        .into_iter()
        .map(|component| component.into_iter().map(|i| corpus[i].0).collect())
        .collect()
}

/// Iterate every type-Idx referenced by a `UserBurdenSpec`.
///
/// Edges enumerated per §04.1 design (a)-(d):
///
/// - (a) `owned_fields[*].field_type`
/// - (b) `borrowed_fields[*].field_type`
/// - (c) `element_burden` when `Some(_)`
/// - (d) `variant_burdens[*].retained_owned[*].field_type`
fn collect_edges<F: FnMut(Idx)>(spec: &UserBurdenSpec, mut emit: F) {
    for field in &spec.owned_fields {
        emit(field.field_type);
    }
    for field in &spec.borrowed_fields {
        emit(field.field_type);
    }
    if let Some(elem) = spec.element_burden {
        emit(elem);
    }
    for variant in &spec.variant_burdens {
        for field in &variant.retained_owned {
            emit(field.field_type);
        }
    }
}

/// Decide whether a type's `UserBurdenSpec` SHALL carry `compiled_drop =
/// Some(_)`. Per §04.1 decision rule:
/// `compiled_drop = Some iff (in non-singleton SCC) OR (self-loop) OR
/// (user_drop = Some(_))`.
///
/// `scc_size` = number of members in this type's SCC.
/// `has_self_loop` = `true` when this type's outgoing edge set contains
/// itself (self-recursive singleton).
#[must_use]
pub fn needs_compiled_drop(scc_size: usize, has_self_loop: bool, spec: &UserBurdenSpec) -> bool {
    scc_size >= 2 || has_self_loop || spec.user_drop.is_some()
}

/// Mint a deterministic `FnSym` for a type's compiled drop function.
///
/// Mangling at codegen-time keys on the type Idx (`_ori_drop$<idx_raw>`
/// per `drop_gen.rs:45`), so the `FnSym` carries the type's Idx raw value
/// — two distinct types yield two distinct `FnSym`s even when they share
/// an SCC. `Idx::ERROR` (raw == 8) is a valid non-zero source, but
/// `Idx::INT` (raw == 0) would collapse to `MIN`; we shift up by one to
/// preserve the per-Idx distinctness contract and reserve raw 0 as the
/// "unminted" sentinel (matching `ori_registry::burden::FnSym`'s
/// `NonZeroU32` discipline).
#[must_use]
pub fn mint_compiled_drop_fn_sym(idx: Idx) -> FnSym {
    // raw + 1 keeps the per-Idx distinctness; saturating add avoids
    // overflow at u32::MAX (which would only arise from Idx::NONE, a
    // sentinel that never participates in the burden corpus).
    let shifted = idx.raw().saturating_add(1);
    // `shifted` is at least 1 since saturating_add(0, 1) = 1; the fall
    // back to NonZeroU32::MIN documents the unreachable branch instead
    // of panicking.
    let nz = NonZeroU32::new(shifted).unwrap_or(NonZeroU32::MIN);
    FnSym::new(nz)
}

/// True when `this_idx` has an out-edge pointing at itself in the
/// adjacency graph — a self-loop forms a singleton SCC that still needs
/// `compiled_drop` per `needs_compiled_drop`.
fn has_self_loop(spec: &UserBurdenSpec, this_idx: Idx) -> bool {
    let mut found = false;
    collect_edges(spec, |target| {
        if target == this_idx {
            found = true;
        }
    });
    found
}

/// Apply the §04.1 SCC + self-loop + `user_drop` decision rule to a corpus,
/// returning per-Idx `Option<FnSym>` mapping for `compiled_drop`.
///
/// `Idx` not present in the corpus has no entry in the returned map.
/// `Idx` whose decision evaluates to `false` is absent from the map
/// (callers preserve any pre-existing `compiled_drop = None`).
#[must_use]
pub fn assign_compiled_drop_fn_syms(corpus: &[(Idx, &UserBurdenSpec)]) -> FxHashMap<Idx, FnSym> {
    let partition = compute_scc_partition(corpus);

    // Build idx -> SCC size lookup
    let mut scc_size_by_idx: FxHashMap<Idx, usize> = FxHashMap::default();
    for component in &partition {
        for &idx in component {
            scc_size_by_idx.insert(idx, component.len());
        }
    }

    let mut out: FxHashMap<Idx, FnSym> = FxHashMap::default();
    for (idx, spec) in corpus {
        let scc_size = scc_size_by_idx.get(idx).copied().unwrap_or(1);
        let self_loop = has_self_loop(spec, *idx);
        if needs_compiled_drop(scc_size, self_loop, spec) {
            out.insert(*idx, mint_compiled_drop_fn_sym(*idx));
        }
    }
    out
}

/// Iterative Tarjan's SCC over an adjacency list keyed by node index.
///
/// Returns components in reverse topological order (sinks first). Each
/// component is an inner `Vec<usize>` of node indices into the original
/// adjacency.
fn tarjan_scc(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut indices: Vec<u32> = vec![UNVISITED; n];
    let mut low_links: Vec<u32> = vec![0; n];
    let mut on_stack: Vec<bool> = vec![false; n];
    let mut stack: Vec<usize> = Vec::with_capacity(n);
    let mut sccs: Vec<Vec<usize>> = Vec::new();
    let mut next_index: u32 = 0;

    for root in 0..n {
        if indices[root] != UNVISITED {
            continue;
        }

        // Initialize the root frame
        let mut call_stack: Vec<DfsFrame> = Vec::new();

        indices[root] = next_index;
        low_links[root] = next_index;
        next_index += 1;
        stack.push(root);
        on_stack[root] = true;
        call_stack.push(DfsFrame {
            node: root,
            next_child: 0,
        });

        while let Some(frame) = call_stack.last_mut() {
            let v = frame.node;
            let neighbors = &adj[v];

            if frame.next_child < neighbors.len() {
                let w = neighbors[frame.next_child];
                frame.next_child += 1;

                if indices[w] == UNVISITED {
                    // Unvisited — descend
                    indices[w] = next_index;
                    low_links[w] = next_index;
                    next_index += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    call_stack.push(DfsFrame {
                        node: w,
                        next_child: 0,
                    });
                } else if on_stack[w] {
                    // Back-edge to a node on the stack — update low-link
                    low_links[v] = low_links[v].min(indices[w]);
                }
            } else {
                // All children processed — pop and possibly emit SCC
                if low_links[v] == indices[v] {
                    let mut component: Vec<usize> = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        component.push(w);
                        if w == v {
                            break;
                        }
                    }
                    sccs.push(component);
                }
                let popped_low = low_links[v];
                call_stack.pop();

                // Propagate low-link to parent if any
                if let Some(parent_frame) = call_stack.last() {
                    let parent_low = low_links[parent_frame.node];
                    low_links[parent_frame.node] = parent_low.min(popped_low);
                }
            }
        }
    }

    sccs
}

#[cfg(test)]
mod tests;

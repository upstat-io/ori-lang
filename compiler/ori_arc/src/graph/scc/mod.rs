//! Strongly connected component (SCC) computation for the call graph.
//!
//! Uses Tarjan's algorithm to decompose the call graph into SCCs, which
//! are the units of borrow inference in the per-function Salsa pipeline.
//!
//! - **Single-function SCCs** (non-recursive): per-function Salsa query,
//!   one-pass analysis, maximum incrementality.
//! - **Multi-function SCCs** (mutually recursive): fixed-point iteration
//!   within the SCC, cached as one Salsa query result.
//!
//! Tarjan's algorithm runs in O(V + E) and produces SCCs in **reverse
//! topological order** — leaf SCCs (callees) before root SCCs (callers).
//! This is the natural evaluation order for borrow inference: callee
//! signatures are finalized before any caller is analyzed.
//!
//! Reference: Tarjan, "Depth-First Search and Linear Graph Algorithms"
//! (SIAM J. Computing, 1972).

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::call_graph::CallGraph;

#[cfg(test)]
mod tests;

/// A strongly connected component of the call graph.
///
/// Contains one or more functions that form a cycle (or a single
/// non-recursive function). Each function in the program appears in
/// exactly one SCC.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Scc {
    /// Function names in this SCC. Single-element for non-recursive
    /// functions; multiple elements for mutually recursive groups.
    pub members: Vec<Name>,
}

impl Scc {
    /// Whether this SCC contains mutual recursion (2+ members)
    /// or self-recursion (1 member that calls itself).
    pub fn is_recursive(&self, graph: &CallGraph) -> bool {
        self.members.len() > 1 || (self.members.len() == 1 && graph.is_recursive(self.members[0]))
    }
}

/// Compute strongly connected components of the call graph using
/// Tarjan's algorithm.
///
/// Returns SCCs in **forward topological order**: leaf SCCs (no outgoing
/// cross-SCC edges) first, root SCCs last. This is the natural output
/// of Tarjan's and the correct evaluation order for borrow inference —
/// callee signatures are finalized before any caller is analyzed.
///
/// Only functions in the call graph's function set are included. External
/// callees (e.g., `ori_*` runtime functions) are not part of any SCC.
pub fn compute_sccs(graph: &CallGraph) -> Vec<Scc> {
    let mut state = TarjanState {
        index_counter: 0,
        stack: Vec::new(),
        on_stack: FxHashSet::default(),
        indices: FxHashMap::default(),
        lowlinks: FxHashMap::default(),
        sccs: Vec::new(),
    };

    // Visit every function in the graph (handles disconnected components).
    for name in graph.functions() {
        if !state.indices.contains_key(&name) {
            strongconnect(name, graph, &mut state);
        }
    }

    state.sccs
}

/// Return SCCs in **forward topological order** (callees first, callers last).
///
/// Tarjan's algorithm naturally produces SCCs in this order: leaf SCCs
/// (no outgoing cross-SCC edges) are completed first, root SCCs last.
/// In forward order, when we process SCC-A, all SCCs that A's functions
/// call into have already been computed.
///
/// This function returns a borrowed view into the same ordering as
/// [`compute_sccs`] — no reversal needed.
pub fn topological_order(sccs: &[Scc]) -> Vec<&Scc> {
    sccs.iter().collect()
}

/// Internal state for Tarjan's iterative algorithm.
struct TarjanState {
    index_counter: u32,
    stack: Vec<Name>,
    on_stack: FxHashSet<Name>,
    indices: FxHashMap<Name, u32>,
    lowlinks: FxHashMap<Name, u32>,
    sccs: Vec<Scc>,
}

/// Iterative Tarjan's strongconnect using an explicit frame stack.
///
/// Avoids recursion depth issues on deeply connected call graphs.
/// Each frame tracks the current node, its callee iterator position,
/// and whether we're resuming after a recursive call.
fn strongconnect(root: Name, graph: &CallGraph, state: &mut TarjanState) {
    // Frame for the explicit call stack.
    struct Frame {
        name: Name,
        /// Callees to visit, collected at frame creation. We track progress
        /// via `next_callee_idx`.
        callees: Vec<Name>,
        next_callee_idx: usize,
    }

    let mut call_stack: Vec<Frame> = Vec::new();

    // Initialize root frame.
    let idx = state.index_counter;
    state.index_counter += 1;
    state.indices.insert(root, idx);
    state.lowlinks.insert(root, idx);
    state.stack.push(root);
    state.on_stack.insert(root);

    let callees: Vec<Name> = graph
        .callees_of(root)
        .iter()
        .copied()
        .filter(|c| graph.contains(*c))
        .collect();

    call_stack.push(Frame {
        name: root,
        callees,
        next_callee_idx: 0,
    });

    while let Some(frame) = call_stack.last_mut() {
        if frame.next_callee_idx < frame.callees.len() {
            let callee = frame.callees[frame.next_callee_idx];
            frame.next_callee_idx += 1;

            if !state.indices.contains_key(&callee) {
                // Callee not yet visited — "recurse" by pushing a new frame.
                let idx = state.index_counter;
                state.index_counter += 1;
                state.indices.insert(callee, idx);
                state.lowlinks.insert(callee, idx);
                state.stack.push(callee);
                state.on_stack.insert(callee);

                let callee_callees: Vec<Name> = graph
                    .callees_of(callee)
                    .iter()
                    .copied()
                    .filter(|c| graph.contains(*c))
                    .collect();

                call_stack.push(Frame {
                    name: callee,
                    callees: callee_callees,
                    next_callee_idx: 0,
                });
            } else if state.on_stack.contains(&callee) {
                // Callee is on the stack — part of the current SCC.
                let caller = frame.name;
                let callee_idx = state.indices[&callee];
                if let Some(caller_lowlink) = state.lowlinks.get_mut(&caller) {
                    *caller_lowlink = (*caller_lowlink).min(callee_idx);
                }
            }
        } else {
            // All callees processed — check if this is an SCC root.
            let name = frame.name;
            let _ = call_stack.pop();

            // Propagate lowlink to parent frame.
            if let Some(parent) = call_stack.last() {
                let child_lowlink = state.lowlinks[&name];
                if let Some(parent_lowlink) = state.lowlinks.get_mut(&parent.name) {
                    *parent_lowlink = (*parent_lowlink).min(child_lowlink);
                }
            }

            // If this node is the root of an SCC, pop the SCC from the stack.
            if state.lowlinks[&name] == state.indices[&name] {
                let mut members = Vec::new();
                loop {
                    let Some(w) = state.stack.pop() else {
                        debug_assert!(false, "SCC root guarantees stack is non-empty");
                        break;
                    };
                    state.on_stack.remove(&w);
                    members.push(w);
                    if w == name {
                        break;
                    }
                }
                state.sccs.push(Scc { members });
            }
        }
    }
}

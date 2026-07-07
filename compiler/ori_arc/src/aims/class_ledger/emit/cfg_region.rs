//! Per-function CFG cycle regions, computed once per plan run.
//!
//! One iterative Tarjan SCC pass answers every cycle query the planner
//! needs: whether a block sits inside a cycle (loop-invariance
//! discrimination) and the exit frontier of the cycle region containing a
//! block (loop-threaded credit release placement). Spec: Annex E §AIMS
//! RL-4 + RL-5.

use crate::ir::ArcFunction;

use super::super::events::successors_of;

/// Cycle facts for one function: SCC ids, per-block cycle membership, and
/// per-SCC exit frontiers.
pub(crate) struct CycleRegions {
    scc_id: Vec<usize>,
    in_cycle: Vec<bool>,
    frontiers: Vec<Vec<usize>>,
}

impl CycleRegions {
    /// One Tarjan pass over `func`'s successor graph.
    pub(crate) fn compute(func: &ArcFunction) -> Self {
        let n = func.blocks.len();
        let successors: Vec<Vec<usize>> = (0..n)
            .map(|b| {
                successors_of(func, b)
                    .into_iter()
                    .filter(|&s| s < n)
                    .collect()
            })
            .collect();
        let scc_id = tarjan_scc(n, &successors);
        let scc_count = scc_id.iter().copied().max().map_or(0, |m| m + 1);
        let mut scc_size = vec![0usize; scc_count];
        for &id in &scc_id {
            scc_size[id] += 1;
        }
        let mut in_cycle = vec![false; n];
        for block in 0..n {
            let self_loop = successors[block].contains(&block);
            in_cycle[block] = scc_size[scc_id[block]] > 1 || self_loop;
        }
        let mut frontiers: Vec<Vec<usize>> = vec![Vec::new(); scc_count];
        for block in 0..n {
            let id = scc_id[block];
            for &succ in &successors[block] {
                if scc_id[succ] != id && !frontiers[id].contains(&succ) {
                    frontiers[id].push(succ);
                }
            }
        }
        for frontier in &mut frontiers {
            frontier.sort_unstable();
        }
        Self {
            scc_id,
            in_cycle,
            frontiers,
        }
    }

    /// Whether `block` can reach itself via successor edges.
    pub(crate) fn in_cycle(&self, block: usize) -> bool {
        self.in_cycle.get(block).copied().unwrap_or(false)
    }

    /// The exit frontier of the cycle region containing `block`: every
    /// successor outside the region of a block inside it, sorted.
    pub(crate) fn exit_frontier(&self, block: usize) -> &[usize] {
        self.scc_id
            .get(block)
            .map_or(&[], |&id| self.frontiers[id].as_slice())
    }
}

/// Iterative Tarjan strongly-connected components; returns per-node SCC id.
fn tarjan_scc(n: usize, successors: &[Vec<usize>]) -> Vec<usize> {
    const UNVISITED: usize = usize::MAX;
    let mut index = vec![UNVISITED; n];
    let mut lowlink = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut scc_id = vec![0usize; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut next_index = 0usize;
    let mut next_scc = 0usize;
    // Explicit DFS frames: (node, next-successor position).
    let mut frames: Vec<(usize, usize)> = Vec::new();
    for root in 0..n {
        if index[root] != UNVISITED {
            continue;
        }
        frames.push((root, 0));
        index[root] = next_index;
        lowlink[root] = next_index;
        next_index += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&mut (node, ref mut pos)) = frames.last_mut() {
            if *pos < successors[node].len() {
                let succ = successors[node][*pos];
                *pos += 1;
                if index[succ] == UNVISITED {
                    index[succ] = next_index;
                    lowlink[succ] = next_index;
                    next_index += 1;
                    stack.push(succ);
                    on_stack[succ] = true;
                    frames.push((succ, 0));
                } else if on_stack[succ] {
                    lowlink[node] = lowlink[node].min(index[succ]);
                }
            } else {
                frames.pop();
                if let Some(&(parent, _)) = frames.last() {
                    lowlink[parent] = lowlink[parent].min(lowlink[node]);
                }
                if lowlink[node] == index[node] {
                    while let Some(member) = stack.pop() {
                        on_stack[member] = false;
                        scc_id[member] = next_scc;
                        if member == node {
                            break;
                        }
                    }
                    next_scc += 1;
                }
            }
        }
    }
    scc_id
}

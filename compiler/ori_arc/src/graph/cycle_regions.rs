//! Per-function CFG cycle regions.
//!
//! One iterative Tarjan SCC pass answers whether a block can execute more than
//! once in one function invocation and computes the exit frontier of its cycle
//! region. Consumers use the same neutral CFG fact for ownership planning and
//! physical allocation eligibility.

use crate::ir::ArcFunction;

use super::successor_block_ids;

/// Cycle facts for one function: SCC ids, per-block cycle membership, and
/// per-SCC exit frontiers.
pub struct CycleRegions {
    scc_id: Vec<usize>,
    in_cycle: Vec<bool>,
    frontiers: Vec<Vec<usize>>,
}

impl CycleRegions {
    /// Compute all cycle regions in one Tarjan pass over `func`.
    pub fn compute(func: &ArcFunction) -> Self {
        let n = func.blocks.len();
        let successors: Vec<Vec<usize>> = func
            .blocks
            .iter()
            .map(|block| {
                successor_block_ids(&block.terminator)
                    .into_iter()
                    .map(crate::ir::ArcBlockId::index)
                    .filter(|&successor| successor < n)
                    .collect()
            })
            .collect();
        let scc_id = tarjan_scc(n, &successors);
        let scc_count = scc_id
            .iter()
            .copied()
            .max()
            .map_or(0, |maximum| maximum + 1);
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
            for &successor in &successors[block] {
                if scc_id[successor] != id && !frontiers[id].contains(&successor) {
                    frontiers[id].push(successor);
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
    pub fn is_in_cycle(&self, block: usize) -> bool {
        self.in_cycle.get(block).copied().unwrap_or(false)
    }

    /// Every successor outside the cycle region containing `block`, sorted.
    pub(crate) fn exit_frontier(&self, block: usize) -> &[usize] {
        self.scc_id
            .get(block)
            .map_or(&[], |&id| self.frontiers[id].as_slice())
    }
}

/// Iterative Tarjan strongly-connected components; returns one SCC id per node.
fn tarjan_scc(n: usize, successors: &[Vec<usize>]) -> Vec<usize> {
    const UNVISITED: usize = usize::MAX;
    let mut index = vec![UNVISITED; n];
    let mut lowlink = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut scc_id = vec![0usize; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut next_index = 0usize;
    let mut next_scc = 0usize;
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
        while let Some(&mut (node, ref mut position)) = frames.last_mut() {
            if *position < successors[node].len() {
                let successor = successors[node][*position];
                *position += 1;
                if index[successor] == UNVISITED {
                    index[successor] = next_index;
                    lowlink[successor] = next_index;
                    next_index += 1;
                    stack.push(successor);
                    on_stack[successor] = true;
                    frames.push((successor, 0));
                } else if on_stack[successor] {
                    lowlink[node] = lowlink[node].min(index[successor]);
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

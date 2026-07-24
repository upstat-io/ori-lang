//! Reachability and local owner-credit balance for realized evidence.

use std::collections::VecDeque;

use crate::graph::successor_block_ids;
use crate::ir::{ArcBlockId, ArcFunction};

use super::evidence::FundingEvent;

pub(super) fn reachable_blocks(func: &ArcFunction) -> Vec<bool> {
    let mut reachable = vec![false; func.blocks.len()];
    if func.blocks.is_empty() {
        return reachable;
    }
    let entry = func.entry.index();
    if entry >= func.blocks.len() {
        return reachable;
    }
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        pending.extend(
            successor_block_ids(&func.blocks[block].terminator)
                .into_iter()
                .map(ArcBlockId::index)
                .filter(|&successor| successor < func.blocks.len()),
        );
    }
    reachable
}

pub(super) fn locally_funded(
    func: &ArcFunction,
    events: &[Vec<FundingEvent>],
    incoming_whole_value_credit: bool,
) -> bool {
    if func.blocks.is_empty() {
        return true;
    }

    let block_count = func.blocks.len();
    let mut incoming = vec![None; block_count];
    let entry = func.entry.index();
    if entry >= block_count {
        return true;
    }
    incoming[entry] = Some(i128::from(incoming_whole_value_credit));
    let mut path_lengths = vec![0usize; block_count];
    let mut queued = vec![false; block_count];
    let mut pending = VecDeque::from([entry]);
    queued[entry] = true;

    while let Some(block) = pending.pop_front() {
        queued[block] = false;
        let Some(mut balance) = incoming[block] else {
            continue;
        };

        for event in &events[block] {
            match event {
                FundingEvent::Credit(count) => balance += i128::from(*count),
                FundingEvent::Consume => {
                    if balance == 0 {
                        return false;
                    }
                    balance -= 1;
                }
                FundingEvent::IterTransfer => {}
            }
        }

        for successor in successor_block_ids(&func.blocks[block].terminator) {
            let successor = successor.index();
            if successor >= block_count
                || incoming[successor].is_some_and(|current| balance >= current)
            {
                continue;
            }

            let path_length = path_lengths[block] + 1;
            if path_length >= block_count {
                return false;
            }
            incoming[successor] = Some(balance);
            path_lengths[successor] = path_length;
            if !queued[successor] {
                queued[successor] = true;
                pending.push_back(successor);
            }
        }
    }
    true
}

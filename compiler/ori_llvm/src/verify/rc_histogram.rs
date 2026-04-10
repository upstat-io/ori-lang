//! Per-block RC operation histogram.
//!
//! Syntactic counter for all 5 RC operation types (alloc/inc/dec/free/cow)
//! per basic block per function. Architecturally separate from `rc_balance`,
//! which is a semantic lifecycle state machine tracking pointer ownership.
//!
//! The histogram counts raw instruction occurrences without tracking pointer
//! identity or state transitions — a fundamentally different concern.

use inkwell::module::Module;
use inkwell::values::InstructionOpcode;

use super::AuditOptions;

/// Raw RC operation classification for histogram counting.
///
/// Covers all 5 RC operation types emitted by codegen: allocation,
/// increment, decrement, free, and copy-on-write. This is the counting
/// vocabulary — not a lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum RcOpKind {
    Alloc,
    Inc,
    Dec,
    Free,
    Cow,
}

impl RcOpKind {
    /// Convert to array index for `BlockHistogram::counts`.
    fn index(self) -> usize {
        self as usize
    }
}

/// Classify an RC runtime function call into an operation kind.
///
/// Returns `None` for non-RC functions and for RC helper functions
/// that don't represent countable operations (e.g., uniqueness checks,
/// reallocation).
///
/// Source of truth for RC function names:
/// `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`
pub(super) fn classify_rc_call(name: &str) -> Option<RcOpKind> {
    // Alloc: core + collection literal allocation wrappers
    if name == "ori_rc_alloc"
        || name == "ori_list_alloc_data"
        || name == "ori_map_literal_alloc"
        || name == "ori_set_literal_alloc"
    {
        return Some(RcOpKind::Alloc);
    }

    // Inc: core + type-specific inc
    if name == "ori_rc_inc" || name == "ori_str_rc_inc" || name == "ori_list_rc_inc" {
        return Some(RcOpKind::Inc);
    }

    // Dec: core + type-specific dec
    if name == "ori_rc_dec"
        || name == "ori_str_rc_dec"
        || name == "ori_buffer_rc_dec"
        || name == "ori_map_buffer_rc_dec"
        || name == "ori_set_buffer_rc_dec"
    {
        return Some(RcOpKind::Dec);
    }

    // Free: core + free wrappers + unique-drop functions
    if name == "ori_rc_free"
        || name == "ori_list_free_data"
        || name == "ori_buffer_drop_unique"
        || name == "ori_set_buffer_drop_unique"
        || name == "ori_map_buffer_drop_unique"
    {
        return Some(RcOpKind::Free);
    }

    // Cow: shared predicate from verify/mod.rs
    if super::is_cow_function(name) {
        return Some(RcOpKind::Cow);
    }

    None
}

/// Per-basic-block RC operation counts.
#[derive(Debug, PartialEq)]
pub(super) struct BlockHistogram {
    /// Block label (from LLVM, or `bb_N` fallback for unnamed blocks).
    pub label: String,
    /// Counts indexed by `RcOpKind` discriminant: [alloc, inc, dec, free, cow].
    pub counts: [u32; 5],
}

/// Per-function RC operation histogram.
#[derive(Debug, PartialEq)]
pub(super) struct FunctionHistogram {
    /// Demangled function name.
    pub name: String,
    /// Per-block histograms.
    pub blocks: Vec<BlockHistogram>,
}

/// Collect per-block RC operation histograms for all functions in a module.
///
/// Walks the LLVM IR in-memory (via inkwell), classifying every call/invoke
/// instruction into an `RcOpKind`. Respects `AuditOptions::function_filter`.
pub(super) fn collect_module_histogram(
    module: &Module<'_>,
    options: &AuditOptions,
) -> Vec<FunctionHistogram> {
    let mut results = Vec::new();

    let mut func = module.get_first_function();
    while let Some(f) = func {
        func = f.get_next_function();

        // Skip declarations (no body).
        if f.get_first_basic_block().is_none() {
            continue;
        }

        let raw_name = f.get_name().to_string_lossy().into_owned();
        if !super::rc_balance::should_audit_fn(&raw_name, options) {
            continue;
        }

        // Demangle using the canonical AOT demangler.
        let display_name = crate::aot::demangle(&raw_name).unwrap_or_else(|| raw_name.clone());

        let mut blocks = Vec::new();
        let mut bb = f.get_first_basic_block();
        let mut block_index: usize = 0;

        while let Some(block) = bb {
            bb = block.get_next_basic_block();

            let label = {
                let name = block.get_name().to_string_lossy();
                if name.is_empty() {
                    format!("bb_{block_index}")
                } else {
                    name.into_owned()
                }
            };

            let mut counts = [0u32; 5];
            let mut inst = block.get_first_instruction();
            while let Some(instruction) = inst {
                inst = instruction.get_next_instruction();

                let opcode = instruction.get_opcode();
                if opcode != InstructionOpcode::Call && opcode != InstructionOpcode::Invoke {
                    continue;
                }

                if let Some(callee) = super::callee_name(instruction) {
                    if let Some(kind) = classify_rc_call(&callee) {
                        counts[kind.index()] += 1;
                    }
                }
            }

            blocks.push(BlockHistogram { label, counts });
            block_index += 1;
        }

        results.push(FunctionHistogram {
            name: display_name,
            blocks,
        });
    }

    results
}

#[cfg(test)]
mod tests;

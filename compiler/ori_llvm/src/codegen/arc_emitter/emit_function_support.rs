//! Stack-backed block labels and function pre-scans.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcVarId};
use ori_types::Idx;
use rustc_hash::FxHashMap;

/// Stack-backed `bb{index}` label used while LLVM copies a block name.
pub(super) struct BlockLabel {
    bytes: [u8; 32],
    len: usize,
}

impl BlockLabel {
    pub(super) fn new(mut index: usize) -> Self {
        const PREFIX_LEN: usize = 2;
        const DIGITS: &[u8; 10] = b"0123456789";

        let mut bytes = [0; 32];
        let mut cursor = bytes.len();
        loop {
            cursor -= 1;
            bytes[cursor] = DIGITS[index % 10];
            index /= 10;
            if index == 0 {
                break;
            }
        }

        let digit_count = bytes.len() - cursor;
        let len = PREFIX_LEN + digit_count;
        bytes.copy_within(cursor.., PREFIX_LEN);
        bytes[..PREFIX_LEN].copy_from_slice(b"bb");
        Self { bytes, len }
    }

    pub(super) fn as_str(&self) -> &str {
        // SAFETY: Every byte in `bytes[..len]` is initialized with ASCII text.
        unsafe { std::str::from_utf8_unchecked(&self.bytes[..self.len]) }
    }
}

/// Maps for-yield element-size variables to their collection and element types.
/// LLVM uses the map to replace source-layout sizes for reordered aggregates and
/// to derive the integer subset eligible for narrowed-size overrides.
pub(super) fn scan_for_yield_elem_size_types(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    yield_lineages: &ori_arc::YieldLineageIndex,
) -> FxHashMap<ArcVarId, (Idx, Idx)> {
    // Why: Comparing interned names avoids a per-instruction interner lookup.
    let list_push = interner.intern("ori_list_push");
    let mut result = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if *callee == list_push && args.len() == 3 {
                    let elem_ty = func.var_type(args[1]);
                    let collection_ty = yield_lineages.result_for_receiver(args[0]).map_or_else(
                        || func.var_type(args[0]),
                        |yield_result| func.var_type(yield_result),
                    );
                    result.insert(args[2], (collection_ty, elem_ty));
                }
            }
        }
    }
    result
}

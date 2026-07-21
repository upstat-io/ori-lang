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
        let mut bytes = [0; 32];
        let mut cursor = bytes.len();
        loop {
            let Some(next_cursor) = cursor.checked_sub(1) else {
                panic!("block index decimal representation must fit label buffer");
            };
            cursor = next_cursor;
            let Ok(digit) = u8::try_from(index % 10) else {
                unreachable!("decimal digit must fit u8");
            };
            let Some(ascii_digit) = b'0'.checked_add(digit) else {
                unreachable!("decimal digit must fit ASCII digit range");
            };
            let Some(slot) = bytes.get_mut(cursor) else {
                unreachable!("checked label cursor must stay within the buffer");
            };
            *slot = ascii_digit;
            index /= 10;
            if index == 0 {
                break;
            }
        }

        let Some(digit_count) = bytes.len().checked_sub(cursor) else {
            unreachable!("digit cursor must stay within label buffer");
        };
        let Some(len) = 2usize.checked_add(digit_count) else {
            unreachable!("block label length must fit usize");
        };
        bytes.copy_within(cursor.., 2);
        bytes[..2].copy_from_slice(b"bb");
        Self { bytes, len }
    }

    pub(super) fn as_str(&self) -> &str {
        let Some(label) = self.bytes.get(..self.len) else {
            unreachable!("block label length must stay within its buffer");
        };
        let Ok(label) = std::str::from_utf8(label) else {
            unreachable!("block label must contain only ASCII bytes");
        };
        label
    }
}

/// Pre-scan: map ALL for-yield `elem_size` `ArcVarId`s to their element type.
///
/// For reordered structs/tuples, `pool_type_store_size` (used by ARC lowering)
/// returns the ORIGINAL layout size, but LLVM's struct layout uses the
/// REORDERED size. The LLVM emitter overrides the literal with
/// `element_store_size(elem_ty)` to ensure the runtime list stride matches
/// LLVM's GEP stride.
///
/// The int-element accumulator subset (safe for narrowed-size overrides) is
/// derived from this map at `emit_arc_function` — one scan feeds both.
pub(super) fn scan_for_yield_elem_size_types(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    yield_lineages: &ori_arc::YieldLineageIndex,
) -> FxHashMap<ArcVarId, (Idx, Idx)> {
    // The for-yield lowerer interns the runtime symbol as a `Name`; compare
    // interned Names instead of per-instruction string lookups.
    let list_push = interner.intern("ori_list_push");
    let mut result = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                // ori_list_push(list_ptr, elem_val, elem_size_var)
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

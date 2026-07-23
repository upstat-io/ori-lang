//! Stack-backed block labels and function pre-scans.

use ori_arc::ir::{ArcFunction, ArcVarId};
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

/// Index typed yield facts by their shared element-size operand.
pub(super) fn index_for_yield_elem_size_types(
    func: &ArcFunction,
) -> FxHashMap<ArcVarId, (Idx, Idx)> {
    func.yield_allocations
        .iter()
        .map(|fact| {
            (
                fact.elem_size_var,
                (func.var_type(fact.result), fact.elem_ty),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ori_arc::ir::{
        AllocationSiteId, ArcFunction, ArcVarId, YieldAllocationFact, YieldAllocationLocality,
        YieldExtent,
    };
    use ori_types::Idx;

    use super::index_for_yield_elem_size_types;

    #[test]
    fn yield_element_size_index_reads_the_typed_fact() {
        let elem_size_var = ArcVarId::new(2);
        let function = ArcFunction {
            var_types: vec![Idx::UNIT, Idx::BOOL, Idx::INT],
            yield_allocations: vec![YieldAllocationFact {
                site: AllocationSiteId::new(0),
                builder: ArcVarId::new(0),
                result: ArcVarId::new(1),
                elem_ty: Idx::INT,
                elem_size_var,
                elem_size: 8,
                extent: YieldExtent::StaticExact(4),
                locality: YieldAllocationLocality::Local,
            }],
            ..ArcFunction::default()
        };

        let index = index_for_yield_elem_size_types(&function);
        assert_eq!(index.get(&elem_size_var), Some(&(Idx::BOOL, Idx::INT)));
    }
}

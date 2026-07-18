use ori_ir::Name;

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId, PrimOp};

pub(super) fn locate_invoke(
    function: &ArcFunction,
    destination: ArcVarId,
    receiver: ArcVarId,
    operation: PrimOp,
    method: Name,
    predecessors: &[Vec<usize>],
) -> Option<usize> {
    let expected_arity = match operation {
        PrimOp::Binary(_) => 2,
        PrimOp::Unary(_) => 1,
    };
    let mut found = None;
    for (block_index, block) in function.blocks.iter().enumerate() {
        if matches!(
            &block.terminator,
            ArcTerminator::Invoke {
                dst,
                ty,
                func,
                args,
                arg_ownership,
                normal,
                unwind,
                ..
            } if *dst == destination
                && function.var_types.get(destination.index()) == Some(ty)
                && *func == method
                && args.first() == Some(&receiver)
                && args.len() == expected_arity
                && (arg_ownership.is_empty() || arg_ownership.len() == args.len())
                && normal != unwind
                && normal.index() != block_index
                && unwind.index() != block_index
                && normal.index() < function.blocks.len()
                && unwind.index() < function.blocks.len()
                && function.blocks[normal.index()].params.is_empty()
                && predecessors[normal.index()] == [block_index]
                && dedicated_unwind_block(
                    function,
                    unwind.index(),
                    block_index,
                    normal.index(),
                    predecessors,
                )
        ) {
            if found.is_some() {
                return None;
            }
            found = Some(block_index);
        }
    }
    found
}

fn dedicated_unwind_block(
    function: &ArcFunction,
    unwind: usize,
    source: usize,
    normal: usize,
    predecessors: &[Vec<usize>],
) -> bool {
    let block = &function.blocks[unwind];
    let valid_exit = match &block.terminator {
        ArcTerminator::Resume => true,
        ArcTerminator::Jump { target, args } => {
            let target = target.index();
            args.is_empty()
                && target < function.blocks.len()
                && target != source
                && target != normal
                && target != unwind
                && function.blocks[target].params.is_empty()
        }
        _ => false,
    };
    predecessors[unwind] == [source]
        && block.params.is_empty()
        && block.body.is_empty()
        && function.spans.get(unwind).is_none_or(Vec::is_empty)
        && valid_exit
}

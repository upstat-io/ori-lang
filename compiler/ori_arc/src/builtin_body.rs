//! Ordinary ARC bodies for compiler-owned builtin callables.

use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::classify::ArcClassifier;
use crate::ir::{compute_var_reprs, ArcFunction, ArcParam, CtorKind};
use crate::lower::ArcIrBuilder;
use crate::Ownership;

/// Build the registered `Error(str) -> Error` constructor as ordinary ARC.
///
/// The callable census inserts this body only when a first-class reference to
/// the registered constructor exists. AIMS then owns its argument transfer,
/// construction, and cleanup facts exactly as it does for source bodies.
pub fn build_builtin_error_constructor(
    name: Name,
    error_type: Idx,
    trace_list_type: Idx,
    pool: &Pool,
) -> ArcFunction {
    let mut builder = ArcIrBuilder::new();
    let message = builder.fresh_var(Idx::STR);
    let entry = builder.entry_block();
    let trace = builder.emit_construct(trace_list_type, CtorKind::ListLiteral, Vec::new(), None);
    let result = builder.emit_construct(
        error_type,
        CtorKind::Struct(name),
        vec![message, trace],
        None,
    );
    builder.terminate_return(result);

    let mut function = builder.finish(
        name,
        vec![ArcParam {
            var: message,
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        error_type,
        entry,
        false,
    );
    let classifier = ArcClassifier::new(pool);
    let representations = compute_var_reprs(&function, &classifier, pool);
    function.replace_variable_representations(representations);
    function
}

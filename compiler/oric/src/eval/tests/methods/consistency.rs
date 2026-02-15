//! Tests for consistency between evaluator builtin methods and the
//! `ori_ir` builtin method registry (the single source of truth).

use std::collections::BTreeSet;

use ori_eval::EVAL_BUILTIN_METHODS;
use ori_ir::builtin_methods::BUILTIN_METHODS;

/// Collection types that have eval methods but are not yet in the
/// `ori_ir` builtin method registry. These are tracked as a gap to fix.
/// Proper-cased names match `EVAL_BUILTIN_METHODS` (and `TypeNames`).
const COLLECTION_TYPES: &[&str] = &["Option", "Result", "list", "map", "range", "tuple"];

/// IR registry methods that are implemented in the evaluator through method
/// resolvers (`UserRegistryResolver`, `CollectionMethodResolver`, etc.) rather
/// than through direct dispatch in `dispatch_builtin_method`.
///
/// These are NOT missing from eval — they work at runtime. They're just
/// dispatched through a different mechanism than `EVAL_BUILTIN_METHODS`.
const IR_METHODS_DISPATCHED_VIA_RESOLVERS: &[(&str, &str)] = &[
    // float — numeric methods dispatched via method resolvers
    ("float", "abs"),
    ("float", "ceil"),
    ("float", "floor"),
    ("float", "max"),
    ("float", "min"),
    ("float", "round"),
    ("float", "sqrt"),
    // int — numeric methods dispatched via method resolvers
    ("int", "abs"),
    ("int", "max"),
    ("int", "min"),
];

/// Eval methods for primitive types that are not yet in the IR builtin
/// method registry. These need to be added to `ori_ir/src/builtin_methods.rs`.
const EVAL_METHODS_NOT_IN_IR: &[(&str, &str)] = &[
    // Duration/Size operator aliases — eval accepts both short and long forms
    // (e.g., "sub" and "subtract"), but IR only registers the short form.
    ("Duration", "divide"),
    ("Duration", "multiply"),
    ("Duration", "negate"),
    ("Duration", "remainder"),
    ("Duration", "subtract"),
    ("Size", "divide"),
    ("Size", "multiply"),
    ("Size", "remainder"),
    ("Size", "subtract"),
    // Debug trait — implemented in eval, not yet in IR for all types
    ("bool", "debug"),
    ("byte", "debug"),
    ("char", "debug"),
    ("float", "debug"),
    ("int", "debug"),
    ("str", "debug"),
    // Printable for str (str.to_str returns itself)
    ("str", "to_str"),
];

/// Build the set of `(type_name, method_name)` from the IR registry.
fn ir_method_set() -> BTreeSet<(&'static str, &'static str)> {
    BUILTIN_METHODS
        .iter()
        .map(|m| (m.receiver.name(), m.name))
        .collect()
}

/// Every method in the IR builtin registry should be implemented in the
/// evaluator (either via direct dispatch or method resolvers).
#[test]
fn ir_methods_implemented_in_eval() {
    let eval_set: BTreeSet<_> = EVAL_BUILTIN_METHODS.iter().copied().collect();
    let resolver_set: BTreeSet<_> = IR_METHODS_DISPATCHED_VIA_RESOLVERS
        .iter()
        .copied()
        .collect();
    let ir_set = ir_method_set();

    let mut missing = Vec::new();
    for &(ty, method) in &ir_set {
        if !eval_set.contains(&(ty, method)) && !resolver_set.contains(&(ty, method)) {
            missing.push((ty, method));
        }
    }

    assert!(
        missing.is_empty(),
        "IR registry has methods not accounted for in evaluator: {missing:?}\n\
         Either add to EVAL_BUILTIN_METHODS (direct dispatch) or \
         IR_METHODS_DISPATCHED_VIA_RESOLVERS (method resolver dispatch)"
    );
}

/// Every eval method for primitive types should be in the IR registry
/// (the single source of truth for builtin method signatures).
#[test]
fn eval_primitive_methods_in_ir() {
    let ir_set = ir_method_set();
    let known_set: BTreeSet<_> = EVAL_METHODS_NOT_IN_IR.iter().copied().collect();

    let mut missing = Vec::new();
    for &(ty, method) in EVAL_BUILTIN_METHODS {
        // Skip collection types (not yet in IR registry)
        if COLLECTION_TYPES.contains(&ty) {
            continue;
        }
        if !ir_set.contains(&(ty, method)) && !known_set.contains(&(ty, method)) {
            missing.push((ty, method));
        }
    }

    assert!(
        missing.is_empty(),
        "Evaluator has primitive methods not in IR registry: {missing:?}\n\
         Add method definitions in ori_ir/src/builtin_methods.rs or \
         add to EVAL_METHODS_NOT_IN_IR"
    );
}

/// The eval method list must be sorted (type, then method) for reliable
/// comparison and easy diffing.
#[test]
fn eval_method_list_is_sorted() {
    for window in EVAL_BUILTIN_METHODS.windows(2) {
        assert!(
            window[0] <= window[1],
            "EVAL_BUILTIN_METHODS not sorted: {:?} > {:?}",
            window[0],
            window[1]
        );
    }
}

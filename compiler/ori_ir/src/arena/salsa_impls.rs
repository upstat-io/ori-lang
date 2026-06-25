//! Manual `PartialEq` / `Eq` / `Hash` / `Debug` impls for [`ExprArena`].
//!
//! Why: `ExprArena` is a Salsa input; its `Eq`/`Hash` key downstream query
//! memoization. The 22 structural fields define arena identity. The call-site
//! type-args side-tables (`method_call_type_args`/`receiver_type_args`) are
//! EXCLUDED — they are sparse parse-recorded metadata, not arena structure;
//! folding them into the Salsa key shifts memoization and perturbs downstream
//! (AIMS RC realization) determinism. A blanket `#[derive(PartialEq, Eq, Hash)]`
//! would include them.

use std::fmt;
use std::hash::{Hash, Hasher};

use super::ExprArena;

impl PartialEq for ExprArena {
    fn eq(&self, other: &Self) -> bool {
        self.expr_kinds == other.expr_kinds
            && self.expr_spans == other.expr_spans
            && self.expr_lists == other.expr_lists
            && self.stmts == other.stmts
            && self.params == other.params
            && self.arms == other.arms
            && self.map_entries == other.map_entries
            && self.field_inits == other.field_inits
            && self.struct_lit_fields == other.struct_lit_fields
            && self.list_elements == other.list_elements
            && self.map_elements == other.map_elements
            && self.named_exprs == other.named_exprs
            && self.call_args == other.call_args
            && self.generic_params == other.generic_params
            && self.parsed_types == other.parsed_types
            && self.parsed_type_lists == other.parsed_type_lists
            && self.match_patterns == other.match_patterns
            && self.match_pattern_lists == other.match_pattern_lists
            && self.binding_patterns == other.binding_patterns
            && self.function_seqs == other.function_seqs
            && self.function_exps == other.function_exps
            && self.template_parts == other.template_parts
            && self.access_steps == other.access_steps
    }
}

impl Eq for ExprArena {}

impl Hash for ExprArena {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.expr_kinds.hash(state);
        self.expr_spans.hash(state);
        self.expr_lists.hash(state);
        self.stmts.hash(state);
        self.params.hash(state);
        self.arms.hash(state);
        self.map_entries.hash(state);
        self.field_inits.hash(state);
        self.struct_lit_fields.hash(state);
        self.list_elements.hash(state);
        self.map_elements.hash(state);
        self.named_exprs.hash(state);
        self.call_args.hash(state);
        self.generic_params.hash(state);
        self.parsed_types.hash(state);
        self.parsed_type_lists.hash(state);
        self.match_patterns.hash(state);
        self.match_pattern_lists.hash(state);
        self.binding_patterns.hash(state);
        self.function_seqs.hash(state);
        self.function_exps.hash(state);
        self.template_parts.hash(state);
        self.access_steps.hash(state);
    }
}

impl fmt::Debug for ExprArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExprArena {{ {} exprs, {} lists, {} stmts, {} params }}",
            self.expr_kinds.len(),
            self.expr_lists.len(),
            self.stmts.len(),
            self.params.len()
        )
    }
}

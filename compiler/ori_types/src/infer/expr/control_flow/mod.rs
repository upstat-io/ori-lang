//! Control flow inference — if, match, for, loop, break, continue.

mod conditionals;
mod loops;
mod matches;
mod or_pattern;
mod substitution;

pub(crate) use conditionals::infer_if;
pub(crate) use loops::{
    for_loop_elem_ty, infer_break, infer_continue, infer_for, infer_loop, infer_while,
};
pub(crate) use matches::{check_match_pattern, infer_match};
pub(crate) use substitution::substitute_type_params_with_map;

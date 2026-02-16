//! Type diffing for specific problem identification.
//!
//! This module compares two types and identifies specific problems,
//! transforming generic "type mismatch" into actionable diagnoses.
//!
//! # Design
//!
//! Instead of just reporting "expected int, found str", we identify:
//! - `IntFloat` when mixing numeric types
//! - `NeedsUnwrap` when using Option<T> where T is expected
//! - `FieldTypo` when a field name is close to a valid one
//!
//! This enables targeted suggestions and better error messages.

use ori_ir::Name;

use super::TypeProblem;
use crate::{Idx, Pool, Tag};

/// Compare two types and identify specific problems.
///
/// Returns a list of problems found. May return multiple problems
/// if there are nested mismatches (e.g., wrong function argument AND return).
pub fn diff_types(pool: &Pool, expected: Idx, found: Idx) -> Vec<TypeProblem> {
    let mut problems = Vec::new();
    diff_types_inner(pool, expected, found, &mut problems);
    problems
}

/// Inner diffing logic that accumulates problems.
fn diff_types_inner(pool: &Pool, expected: Idx, found: Idx, problems: &mut Vec<TypeProblem>) {
    // Same type? No problem.
    if expected == found {
        return;
    }

    let exp_tag = pool.tag(expected);
    let found_tag = pool.tag(found);

    // Check for specific patterns
    match (exp_tag, found_tag) {
        // === Numeric Problems ===

        // Int vs Float
        (Tag::Int, Tag::Float) | (Tag::Float, Tag::Int) => {
            problems.push(TypeProblem::IntFloat {
                expected: tag_name(exp_tag),
                found: tag_name(found_tag),
            });
        }

        // String vs Number
        (Tag::Str, Tag::Int | Tag::Float) => {
            problems.push(TypeProblem::NumberToString);
        }
        (Tag::Int | Tag::Float, Tag::Str) => {
            problems.push(TypeProblem::StringToNumber);
        }

        // Byte vs Int or Str
        (Tag::Byte, Tag::Int | Tag::Str) | (Tag::Int | Tag::Str, Tag::Byte) => {
            problems.push(TypeProblem::NumericTypeMismatch {
                expected: tag_name(exp_tag),
                found: tag_name(found_tag),
            });
        }

        // === Collection Problems ===

        // Expected List, got something else
        (Tag::List, other) if other != Tag::List => {
            problems.push(TypeProblem::ExpectedList {
                found: tag_name(other),
            });
        }

        // List element mismatch
        (Tag::List, Tag::List) => {
            let exp_elem = Idx::from_raw(pool.data(expected));
            let found_elem = Idx::from_raw(pool.data(found));
            if exp_elem != found_elem {
                problems.push(TypeProblem::ListElementMismatch { index: 0 });
                // Recurse to find nested problems
                diff_types_inner(pool, exp_elem, found_elem, problems);
            }
        }

        // Expected Option, got something else
        (Tag::Option, other) if other != Tag::Option => {
            // Check if the inner type matches - might just need wrapping
            let inner = Idx::from_raw(pool.data(expected));
            if inner == found {
                // Found T where Option<T> expected - suggest wrapping
                problems.push(TypeProblem::TypeMismatch {
                    expected_category: "option",
                    found_category: tag_name(other),
                });
            } else {
                problems.push(TypeProblem::ExpectedOption);
            }
        }

        // Using Option<T> where T is expected - needs unwrap
        (other, Tag::Option) if other != Tag::Option => {
            let inner = Idx::from_raw(pool.data(found));
            if inner == expected || types_compatible(pool, expected, inner) {
                problems.push(TypeProblem::NeedsUnwrap { inner_type: inner });
            } else {
                problems.push(TypeProblem::TypeMismatch {
                    expected_category: tag_name(other),
                    found_category: "option",
                });
            }
        }

        // Map mismatches
        (Tag::Map, Tag::Map) => {
            let exp_key = pool.map_key(expected);
            let found_key = pool.map_key(found);
            let exp_val = pool.map_value(expected);
            let found_val = pool.map_value(found);

            if exp_key != found_key {
                problems.push(TypeProblem::MapKeyMismatch);
                diff_types_inner(pool, exp_key, found_key, problems);
            }
            if exp_val != found_val {
                problems.push(TypeProblem::MapValueMismatch);
                diff_types_inner(pool, exp_val, found_val, problems);
            }
        }

        // Wrong collection type
        (Tag::List, Tag::Set) | (Tag::Set, Tag::List) => {
            problems.push(TypeProblem::WrongCollectionType {
                expected: if exp_tag == Tag::List { "list" } else { "set" },
                found: if found_tag == Tag::List {
                    "list"
                } else {
                    "set"
                },
            });
        }
        (Tag::List | Tag::Set, Tag::Map) | (Tag::Map, Tag::List | Tag::Set) => {
            problems.push(TypeProblem::WrongCollectionType {
                expected: tag_name(exp_tag),
                found: tag_name(found_tag),
            });
        }

        // === Function Problems ===

        // Function arity/type mismatch
        (Tag::Function, Tag::Function) => {
            let exp_params = pool.function_params(expected);
            let found_params = pool.function_params(found);
            let exp_ret = pool.function_return(expected);
            let found_ret = pool.function_return(found);

            if exp_params.len() == found_params.len() {
                // Check each parameter
                for (i, (exp_p, found_p)) in exp_params.iter().zip(found_params.iter()).enumerate()
                {
                    if exp_p != found_p {
                        problems.push(TypeProblem::ArgumentMismatch {
                            arg_index: i,
                            expected: *exp_p,
                            found: *found_p,
                        });
                    }
                }
            } else {
                problems.push(TypeProblem::WrongArity {
                    expected: exp_params.len(),
                    found: found_params.len(),
                });
            }

            if exp_ret != found_ret {
                problems.push(TypeProblem::ReturnMismatch {
                    expected: exp_ret,
                    found: found_ret,
                });
            }
        }

        // Expected function, got something else
        (Tag::Function, other) => {
            problems.push(TypeProblem::NotCallable { actual_type: found });
            let _ = other; // suppress unused warning
        }

        // === Tuple Problems ===
        (Tag::Tuple, Tag::Tuple) => {
            let exp_elems = pool.tuple_elems(expected);
            let found_elems = pool.tuple_elems(found);

            if exp_elems.len() == found_elems.len() {
                for (i, (exp_e, found_e)) in exp_elems.iter().zip(found_elems.iter()).enumerate() {
                    if exp_e != found_e {
                        diff_types_inner(pool, *exp_e, *found_e, problems);
                        // Add context about which tuple element
                        if problems.is_empty() {
                            problems.push(TypeProblem::TypeMismatch {
                                expected_category: "tuple element",
                                found_category: "tuple element",
                            });
                        }
                        let _ = i; // suppress unused warning
                    }
                }
            } else {
                problems.push(TypeProblem::WrongArity {
                    expected: exp_elems.len(),
                    found: found_elems.len(),
                });
            }
        }

        // === Named Type Problems ===
        (Tag::Named, Tag::Named) => {
            let exp_name = pool.named_name(expected);
            let found_name = pool.named_name(found);
            if exp_name != found_name {
                problems.push(TypeProblem::WrongRecordType {
                    expected: exp_name,
                    found: found_name,
                });
            }
        }

        // === Applied Type Problems ===
        (Tag::Applied, Tag::Applied) => {
            let exp_name = pool.applied_name(expected);
            let found_name = pool.applied_name(found);
            let exp_args = pool.applied_args(expected);
            let found_args = pool.applied_args(found);

            if exp_name != found_name {
                problems.push(TypeProblem::WrongRecordType {
                    expected: exp_name,
                    found: found_name,
                });
            } else if exp_args.len() != found_args.len() {
                problems.push(TypeProblem::WrongArity {
                    expected: exp_args.len(),
                    found: found_args.len(),
                });
            } else {
                for (exp_a, found_a) in exp_args.iter().zip(found_args.iter()) {
                    if exp_a != found_a {
                        diff_types_inner(pool, *exp_a, *found_a, problems);
                    }
                }
            }
        }

        // === Generic Fallback ===
        _ => {
            problems.push(TypeProblem::TypeMismatch {
                expected_category: tag_name(exp_tag),
                found_category: tag_name(found_tag),
            });
        }
    }
}

/// Check if two types are compatible (same after resolving variables).
fn types_compatible(pool: &Pool, a: Idx, b: Idx) -> bool {
    if a == b {
        return true;
    }

    let a_tag = pool.tag(a);
    let b_tag = pool.tag(b);

    // Variables are compatible with anything
    if a_tag == Tag::Var || b_tag == Tag::Var {
        return true;
    }

    // Same tag means potentially compatible
    a_tag == b_tag
}

/// Get a human-readable name for a tag.
fn tag_name(tag: Tag) -> &'static str {
    match tag {
        Tag::Int => "int",
        Tag::Float => "float",
        Tag::Bool => "bool",
        Tag::Str => "str",
        Tag::Char => "char",
        Tag::Byte => "byte",
        Tag::Unit => "unit",
        Tag::Never => "never",
        Tag::Error => "error",
        Tag::Duration => "duration",
        Tag::Size => "size",
        Tag::Ordering => "ordering",
        Tag::List => "list",
        Tag::Option => "option",
        Tag::Set => "set",
        Tag::Channel => "channel",
        Tag::Range => "range",
        Tag::Iterator => "Iterator",
        Tag::DoubleEndedIterator => "DoubleEndedIterator",
        Tag::Map => "map",
        Tag::Result => "result",
        Tag::Borrowed => "borrowed reference",
        Tag::Function => "function",
        Tag::Tuple => "tuple",
        Tag::Struct => "struct",
        Tag::Enum => "enum",
        Tag::Named => "named type",
        Tag::Applied => "generic type",
        Tag::Alias => "type alias",
        Tag::Var => "type variable",
        Tag::BoundVar => "bound variable",
        Tag::RigidVar => "type parameter",
        Tag::Scheme => "type scheme",
        Tag::Projection => "type projection",
        Tag::ModuleNs => "module",
        Tag::Infer => "inferred type",
        Tag::SelfType => "Self",
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Edit Distance (for typo detection)
// ════════════════════════════════════════════════════════════════════════════

/// Compute the Levenshtein edit distance between two strings.
///
/// Returns the minimum number of single-character edits (insertions,
/// deletions, or substitutions) required to change one string into the other.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    // Early termination for empty strings
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Use two rows instead of full matrix (space optimization)
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for (i, a_char) in a.chars().enumerate() {
        curr_row[0] = i + 1;

        for (j, b_char) in b.chars().enumerate() {
            let cost = usize::from(a_char != b_char);

            curr_row[j + 1] = (prev_row[j + 1] + 1) // deletion
                .min(curr_row[j] + 1) // insertion
                .min(prev_row[j] + cost); // substitution
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Find the closest field name to a given name using edit distance.
///
/// Returns `Some((name, distance))` if a close match is found,
/// `None` if no field is within the threshold.
pub fn find_closest_field(
    attempted: &str,
    available: &[Name],
    interner: &ori_ir::StringInterner,
    max_distance: usize,
) -> Option<(Name, usize)> {
    let mut best: Option<(Name, usize)> = None;

    for &name in available {
        let field_str = interner.lookup(name);
        let distance = edit_distance(attempted, field_str);

        if distance <= max_distance {
            match best {
                None => best = Some((name, distance)),
                Some((_, best_dist)) if distance < best_dist => best = Some((name, distance)),
                _ => {}
            }
        }
    }

    best
}

/// Suggest a field typo problem if the attempted name is close to an available field.
pub fn suggest_field_typo(
    attempted: Name,
    available: &[Name],
    interner: &ori_ir::StringInterner,
) -> Option<TypeProblem> {
    let attempted_str = interner.lookup(attempted);

    // Threshold: allow 2 edits for short names, 3 for longer names
    let max_distance = if attempted_str.len() <= 4 { 2 } else { 3 };

    find_closest_field(attempted_str, available, interner, max_distance).map(
        |(suggestion, distance)| TypeProblem::FieldTypo {
            attempted,
            suggestion,
            distance,
        },
    )
}

#[cfg(test)]
mod tests;

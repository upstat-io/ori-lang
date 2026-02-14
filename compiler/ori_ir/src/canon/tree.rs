//! Decision tree types for pattern matching.
//!
//! These types represent compiled decision trees as produced by the Maranget (2008)
//! algorithm. They are shared between `ori_canon` (builds them during lowering),
//! `ori_eval` (interprets them), and `ori_arc` (emits them as ARC IR blocks).
//!
//! # Architecture
//!
//! The TYPE DEFINITIONS live here in `ori_ir` (shared crate). The compilation
//! ALGORITHM lives in `ori_arc::decision_tree::compile` (and will move to
//! `ori_canon::patterns` in `eval_v2` Section 03). The ARC IR emission logic
//! stays in `ori_arc::decision_tree::emit`.
//!
//! # References
//!
//! - Maranget (2008) "Compiling Pattern Matching to Good Decision Trees"
//! - Roc `crates/compiler/mono/src/ir/decision_tree.rs`
//! - Elm `compiler/src/Nitpick/PatternMatches.hs`

use super::CanId;
use crate::Name;

// Scrutinee Path Tracking

/// A path from the root scrutinee to a sub-value.
///
/// When testing nested patterns, the scrutinee for inner tests is derived
/// by projecting fields from the outer scrutinee. A `ScrutineePath` tracks
/// how to reach any sub-scrutinee from the root.
///
/// # Example
///
/// Matching `Cons(Pair(x, _), _)`:
/// - Root scrutinee: the list value
/// - Path to `x`: `[TagPayload(0), TupleIndex(0)]`
///   (get Cons payload field 0, then get first element of Pair)
///
/// # Performance
///
/// Uses `Vec<PathInstruction>` to avoid heap allocation for
/// typical pattern depths (≤ 4). Deeply nested patterns spill to heap,
/// which is acceptable since they are rare. This matters because the
/// Maranget algorithm clones paths frequently during matrix specialization.
pub type ScrutineePath = Vec<PathInstruction>;

/// One step in a scrutinee path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathInstruction {
    /// Extract the payload of an enum variant at the given field index.
    /// Used after a tag test confirms the variant.
    TagPayload(u32),
    /// Extract element at index from a tuple.
    TupleIndex(u32),
    /// Extract a named field from a struct (by position, since struct
    /// field order is fixed after type checking).
    StructField(u32),
    /// Extract element at index from a list (for list pattern matching).
    ListElement(u32),
    /// Extract the sub-list starting at the given index (for `..rest` patterns).
    /// `ListRest(2)` on `[a, b, c, d]` yields `[c, d]`.
    ListRest(u32),
}

// Test Kinds

/// What kind of test to perform on a scrutinee.
///
/// The test kind is separate from the test value. A `Switch` node has
/// one `TestKind` and multiple `TestValue` edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TestKind {
    /// Compare the tag of an enum/union value.
    /// Edges are `TestValue::Tag` variants.
    EnumTag,
    /// Compare an integer value (equality).
    /// Edges are `TestValue::Int` variants.
    IntEq,
    /// Compare a string value (equality).
    /// Edges are `TestValue::Str` variants.
    StrEq,
    /// Compare a boolean value (equality).
    /// Edges are `TestValue::Bool` variants.
    BoolEq,
    /// Compare a float value (exact bit equality).
    /// Edges are `TestValue::Float` variants.
    ///
    /// Forward-looking: may not be in 0.1-alpha spec.
    FloatEq,
    /// Check if a value falls within an integer range (inclusive).
    /// Edges are `TestValue::IntRange` variants.
    ///
    /// Forward-looking: may not be in 0.1-alpha spec.
    IntRange,
    /// Compare a char value (equality).
    /// Edges are `TestValue::Char` variants.
    CharEq,
    /// Check the length of a list (for list patterns).
    /// Edges are `TestValue::ListLen` variants.
    ListLen,
}

/// A specific test value for one edge of a `Switch` node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TestValue {
    /// Tag match for an enum variant.
    Tag {
        /// Discriminant index used for the switch instruction.
        variant_index: u32,
        /// Variant name for diagnostics and readability.
        variant_name: Name,
    },
    /// Integer literal match.
    Int(i64),
    /// String literal match.
    Str(Name),
    /// Boolean literal match.
    Bool(bool),
    /// Float literal match (exact bit equality via `u64` bits).
    Float(u64),
    /// Char literal match.
    Char(char),
    /// Integer range match.
    IntRange {
        lo: i64,
        hi: i64,
        /// If `true`, the upper bound is inclusive (`lo..=hi`).
        /// If `false`, the upper bound is exclusive (`lo..hi`).
        inclusive: bool,
    },
    /// List length match.
    ListLen {
        /// Expected length.
        len: u32,
        /// If `true`, exact match. If `false`, minimum length (has rest pattern).
        is_exact: bool,
    },
}

// Decision Tree

/// A compiled decision tree for pattern matching.
///
/// Constructed during canonicalization (or currently during AST → ARC IR
/// lowering in `ori_arc`). The tree structure follows Maranget (2008),
/// as implemented by Roc and Elm.
///
/// # Consumers
///
/// - `ori_eval`: interprets the tree by walking nodes and evaluating guards
/// - `ori_arc`: emits ARC IR basic blocks with `Switch`/`Branch` terminators
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionTree {
    /// Test a scrutinee, branch based on the result.
    ///
    /// Each edge maps a test value to a subtree. The `default` subtree
    /// handles values not covered by any edge (wildcards, catch-all).
    Switch {
        /// How to reach the value being tested (path from root scrutinee).
        path: ScrutineePath,
        /// The kind of test being performed.
        test_kind: TestKind,
        /// Branches: each edge maps a test value to a subtree.
        edges: Vec<(TestValue, DecisionTree)>,
        /// Default subtree for values not covered by any edge.
        default: Option<Box<DecisionTree>>,
    },
    /// Reached a match arm. Bind variables and execute the body.
    Leaf {
        /// Index of the arm in the original match expression.
        arm_index: usize,
        /// Variable bindings: each maps a name to the path where its
        /// value can be found relative to the root scrutinee.
        bindings: Vec<(Name, ScrutineePath)>,
    },
    /// Guarded leaf. Test a guard condition; if it fails, fall through
    /// to the next compatible arm (not just the next arm in source order).
    Guard {
        /// Index of the arm in the original match expression.
        arm_index: usize,
        /// Variable bindings for this arm.
        bindings: Vec<(Name, ScrutineePath)>,
        /// The guard expression to evaluate (canonical).
        guard: CanId,
        /// Decision tree to execute if the guard fails. This contains
        /// the remaining compatible arms — not just the next sequential arm.
        on_fail: Box<DecisionTree>,
    },
    /// Unreachable. Exhaustiveness guarantees this won't execute.
    /// Maps to LLVM `unreachable` instruction.
    Fail,
}

// Flat Pattern (for the Maranget algorithm)

/// A single pattern in the matrix, flattened for the algorithm.
///
/// This is the algorithm's internal representation of a `MatchPattern`.
/// It strips arena indirection and normalizes patterns for uniform
/// handling during specialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlatPattern {
    /// Matches anything without binding.
    Wildcard,
    /// Matches anything and binds the value to a name.
    Binding(Name),
    /// Matches a specific integer literal.
    LitInt(i64),
    /// Matches a specific float literal (bits).
    LitFloat(u64),
    /// Matches a specific boolean literal.
    LitBool(bool),
    /// Matches a specific string literal (interned).
    LitStr(Name),
    /// Matches a specific char literal.
    LitChar(char),
    /// Matches an enum variant with sub-patterns for the payload fields.
    Variant {
        variant_name: Name,
        variant_index: u32,
        fields: Vec<FlatPattern>,
    },
    /// Matches a tuple with sub-patterns for each element.
    Tuple(Vec<FlatPattern>),
    /// Matches a struct with named fields.
    Struct {
        /// (`field_name`, `sub_pattern`) — position-indexed.
        fields: Vec<(Name, FlatPattern)>,
    },
    /// Matches a list with a specific length and element patterns.
    List {
        elements: Vec<FlatPattern>,
        /// If `Some(name)`, binds the rest of the list.
        rest: Option<Name>,
    },
    /// Matches a range of integers (inclusive).
    Range {
        start: Option<i64>,
        end: Option<i64>,
        inclusive: bool,
    },
    /// Or-pattern: matches if any sub-pattern matches.
    /// All alternatives must bind the same names.
    Or(Vec<FlatPattern>),
    /// At-pattern: binds the scrutinee to a name AND matches a sub-pattern.
    At { name: Name, inner: Box<FlatPattern> },
}

impl FlatPattern {
    /// Returns `true` if this pattern matches anything (wildcard or binding).
    ///
    /// Or-patterns are wildcard-like if any alternative is wildcard-like
    /// (e.g., `Or([LitInt(1), Wildcard])` matches anything).
    /// At-patterns delegate to the inner pattern.
    pub fn is_wildcard_like(&self) -> bool {
        match self {
            FlatPattern::Wildcard | FlatPattern::Binding(_) => true,
            FlatPattern::Or(alts) => alts.iter().any(FlatPattern::is_wildcard_like),
            FlatPattern::At { inner, .. } => inner.is_wildcard_like(),
            _ => false,
        }
    }

    /// Returns `true` if this pattern is a constructor (produces sub-patterns
    /// on specialization).
    pub fn is_constructor(&self) -> bool {
        matches!(
            self,
            FlatPattern::Variant { .. }
                | FlatPattern::Tuple(_)
                | FlatPattern::Struct { .. }
                | FlatPattern::List { .. }
                | FlatPattern::LitInt(_)
                | FlatPattern::LitFloat(_)
                | FlatPattern::LitBool(_)
                | FlatPattern::LitStr(_)
                | FlatPattern::LitChar(_)
                | FlatPattern::Range { .. }
        )
    }

    /// Extract the variable bindings from this pattern at a given path.
    ///
    /// Recursively walks nested patterns, extending the path at each step.
    pub fn extract_bindings(&self, path: &ScrutineePath) -> Vec<(Name, ScrutineePath)> {
        let mut bindings = Vec::new();
        self.collect_bindings(path, &mut bindings);
        bindings
    }

    /// Collect variable bindings from this pattern at a given path, appending
    /// to an existing Vec. Useful for accumulating bindings across multiple
    /// patterns in a row (avoiding per-pattern allocation).
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Field indices are always < u32::MAX"
    )]
    pub fn collect_bindings(&self, path: &ScrutineePath, out: &mut Vec<(Name, ScrutineePath)>) {
        match self {
            FlatPattern::Binding(name) => {
                out.push((*name, path.clone()));
            }
            FlatPattern::Wildcard
            | FlatPattern::LitInt(_)
            | FlatPattern::LitFloat(_)
            | FlatPattern::LitBool(_)
            | FlatPattern::LitStr(_)
            | FlatPattern::LitChar(_)
            | FlatPattern::Range { .. } => {}
            FlatPattern::Variant { fields, .. } => {
                for (i, field) in fields.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(PathInstruction::TagPayload(i as u32));
                    field.collect_bindings(&child_path, out);
                }
            }
            FlatPattern::Tuple(elements) => {
                for (i, elem) in elements.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(PathInstruction::TupleIndex(i as u32));
                    elem.collect_bindings(&child_path, out);
                }
            }
            FlatPattern::Struct { fields } => {
                for (i, (_name, sub)) in fields.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(PathInstruction::StructField(i as u32));
                    sub.collect_bindings(&child_path, out);
                }
            }
            FlatPattern::List { elements, rest } => {
                for (i, elem) in elements.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(PathInstruction::ListElement(i as u32));
                    elem.collect_bindings(&child_path, out);
                }
                if let Some(rest_name) = rest {
                    let mut rest_path = path.clone();
                    rest_path.push(PathInstruction::ListRest(elements.len() as u32));
                    out.push((*rest_name, rest_path));
                }
            }
            FlatPattern::Or(alternatives) => {
                // All alternatives bind the same names (enforced by type checker).
                // Use the first alternative's bindings.
                if let Some(first) = alternatives.first() {
                    first.collect_bindings(path, out);
                }
            }
            FlatPattern::At { name, inner } => {
                // Bind the name at the current path (the whole scrutinee).
                out.push((*name, path.clone()));
                // Then collect bindings from the inner pattern.
                inner.collect_bindings(path, out);
            }
        }
    }
}

/// A row in the pattern matrix (one match arm).
#[derive(Clone, Debug)]
pub struct PatternRow {
    /// Remaining patterns to test (one per column).
    pub patterns: Vec<FlatPattern>,
    /// The arm index in the original match expression.
    pub arm_index: usize,
    /// Guard expression, if any (canonical).
    pub guard: Option<CanId>,
    /// Accumulated variable bindings from specialization steps.
    ///
    /// When a `Binding(name)` or `At { name, .. }` pattern is consumed during
    /// column specialization, the binding `(name, path)` is recorded here.
    /// These are merged with pattern-derived bindings at the Leaf/Guard node.
    pub bindings: Vec<(Name, ScrutineePath)>,
}

/// The pattern matrix: rows of arms, columns of sub-patterns.
pub type PatternMatrix = Vec<PatternRow>;

#[cfg(test)]
mod tests {
    use std::mem;

    use crate::Name;

    use super::*;

    // ── PathInstruction ─────────────────────────────────────────

    #[test]
    fn path_instruction_size() {
        // PathInstruction should be small (8 bytes: discriminant + u32).
        assert!(mem::size_of::<PathInstruction>() <= 8);
    }

    #[test]
    fn path_instruction_equality() {
        assert_eq!(
            PathInstruction::TagPayload(0),
            PathInstruction::TagPayload(0),
        );
        assert_ne!(
            PathInstruction::TagPayload(0),
            PathInstruction::TagPayload(1),
        );
        assert_ne!(
            PathInstruction::TagPayload(0),
            PathInstruction::TupleIndex(0),
        );
    }

    // ── ScrutineePath ───────────────────────────────────────────

    #[test]
    fn scrutinee_path_empty() {
        let path: ScrutineePath = Vec::new();
        assert!(path.is_empty());
    }

    #[test]
    fn scrutinee_path_multiple_elements() {
        let path: ScrutineePath = vec![
            PathInstruction::TagPayload(0),
            PathInstruction::TupleIndex(1),
            PathInstruction::StructField(2),
            PathInstruction::ListElement(3),
        ];
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn scrutinee_path_clone_independence() {
        let original: ScrutineePath = vec![PathInstruction::TagPayload(0)];
        let mut cloned = original.clone();
        cloned.push(PathInstruction::TupleIndex(1));
        assert_eq!(original.len(), 1);
        assert_eq!(cloned.len(), 2);
    }

    // ── TestKind ────────────────────────────────────────────────

    #[test]
    fn test_kind_equality() {
        assert_eq!(TestKind::EnumTag, TestKind::EnumTag);
        assert_ne!(TestKind::EnumTag, TestKind::IntEq);
    }

    // ── TestValue ───────────────────────────────────────────────

    #[test]
    fn test_value_tag() {
        let tv = TestValue::Tag {
            variant_index: 0,
            variant_name: Name::from_raw(42),
        };
        assert_eq!(
            tv,
            TestValue::Tag {
                variant_index: 0,
                variant_name: Name::from_raw(42),
            }
        );
    }

    #[test]
    fn test_value_int() {
        assert_eq!(TestValue::Int(42), TestValue::Int(42));
        assert_ne!(TestValue::Int(42), TestValue::Int(43));
    }

    #[test]
    fn test_value_bool() {
        assert_ne!(TestValue::Bool(true), TestValue::Bool(false));
    }

    #[test]
    fn test_value_str() {
        let s1 = TestValue::Str(Name::from_raw(1));
        let s2 = TestValue::Str(Name::from_raw(1));
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_value_list_len() {
        let exact = TestValue::ListLen {
            len: 3,
            is_exact: true,
        };
        let min = TestValue::ListLen {
            len: 3,
            is_exact: false,
        };
        assert_ne!(exact, min);
    }

    #[test]
    fn test_value_int_range() {
        let r = TestValue::IntRange {
            lo: 1,
            hi: 10,
            inclusive: false,
        };
        assert_eq!(
            r,
            TestValue::IntRange {
                lo: 1,
                hi: 10,
                inclusive: false
            }
        );
        assert_ne!(
            r,
            TestValue::IntRange {
                lo: 1,
                hi: 11,
                inclusive: false
            }
        );
    }

    // ── DecisionTree ────────────────────────────────────────────

    #[test]
    fn decision_tree_leaf() {
        let leaf = DecisionTree::Leaf {
            arm_index: 0,
            bindings: vec![],
        };
        assert!(matches!(leaf, DecisionTree::Leaf { arm_index: 0, .. }));
    }

    #[test]
    fn decision_tree_fail() {
        let fail = DecisionTree::Fail;
        assert!(matches!(fail, DecisionTree::Fail));
    }

    #[test]
    fn decision_tree_switch_simple() {
        // match x { true -> 1, false -> 0 }
        let tree = DecisionTree::Switch {
            path: Vec::new(),
            test_kind: TestKind::BoolEq,
            edges: vec![
                (
                    TestValue::Bool(true),
                    DecisionTree::Leaf {
                        arm_index: 0,
                        bindings: vec![],
                    },
                ),
                (
                    TestValue::Bool(false),
                    DecisionTree::Leaf {
                        arm_index: 1,
                        bindings: vec![],
                    },
                ),
            ],
            default: None,
        };
        if let DecisionTree::Switch { edges, .. } = &tree {
            assert_eq!(edges.len(), 2);
        } else {
            panic!("expected Switch");
        }
    }

    #[test]
    fn decision_tree_switch_with_default() {
        // match n { 1 -> a, 2 -> b, _ -> c }
        let tree = DecisionTree::Switch {
            path: Vec::new(),
            test_kind: TestKind::IntEq,
            edges: vec![
                (
                    TestValue::Int(1),
                    DecisionTree::Leaf {
                        arm_index: 0,
                        bindings: vec![],
                    },
                ),
                (
                    TestValue::Int(2),
                    DecisionTree::Leaf {
                        arm_index: 1,
                        bindings: vec![],
                    },
                ),
            ],
            default: Some(Box::new(DecisionTree::Leaf {
                arm_index: 2,
                bindings: vec![],
            })),
        };
        if let DecisionTree::Switch { default, .. } = &tree {
            assert!(default.is_some());
        } else {
            panic!("expected Switch");
        }
    }

    #[test]
    fn decision_tree_guard() {
        // match x { Some(v) if v > 0 -> pos, Some(v) -> other, None -> none }
        let guard_tree = DecisionTree::Guard {
            arm_index: 0,
            bindings: vec![(Name::from_raw(1), vec![PathInstruction::TagPayload(0)])],
            guard: CanId::new(100),
            on_fail: Box::new(DecisionTree::Leaf {
                arm_index: 1,
                bindings: vec![(Name::from_raw(1), vec![PathInstruction::TagPayload(0)])],
            }),
        };
        assert!(matches!(
            guard_tree,
            DecisionTree::Guard { arm_index: 0, .. }
        ));
    }

    #[test]
    fn decision_tree_nested_switch() {
        // match (tag, payload) { Some(Some(x)) -> deep, Some(None) -> inner_none, None -> outer_none }
        let inner_switch = DecisionTree::Switch {
            path: vec![PathInstruction::TagPayload(0)],
            test_kind: TestKind::EnumTag,
            edges: vec![
                (
                    TestValue::Tag {
                        variant_index: 1,
                        variant_name: Name::from_raw(10),
                    },
                    DecisionTree::Leaf {
                        arm_index: 0,
                        bindings: vec![],
                    },
                ),
                (
                    TestValue::Tag {
                        variant_index: 0,
                        variant_name: Name::from_raw(11),
                    },
                    DecisionTree::Leaf {
                        arm_index: 1,
                        bindings: vec![],
                    },
                ),
            ],
            default: None,
        };

        let tree = DecisionTree::Switch {
            path: Vec::new(),
            test_kind: TestKind::EnumTag,
            edges: vec![
                (
                    TestValue::Tag {
                        variant_index: 1,
                        variant_name: Name::from_raw(10),
                    },
                    inner_switch,
                ),
                (
                    TestValue::Tag {
                        variant_index: 0,
                        variant_name: Name::from_raw(11),
                    },
                    DecisionTree::Leaf {
                        arm_index: 2,
                        bindings: vec![],
                    },
                ),
            ],
            default: None,
        };

        if let DecisionTree::Switch { edges, .. } = &tree {
            assert_eq!(edges.len(), 2);
            // First edge leads to another Switch (nested).
            assert!(matches!(&edges[0].1, DecisionTree::Switch { .. }));
        } else {
            panic!("expected Switch");
        }
    }

    // ── FlatPattern ─────────────────────────────────────────────

    #[test]
    fn flat_pattern_wildcard_like() {
        assert!(FlatPattern::Wildcard.is_wildcard_like());
        assert!(FlatPattern::Binding(Name::from_raw(1)).is_wildcard_like());
        assert!(!FlatPattern::LitInt(42).is_wildcard_like());
        assert!(!FlatPattern::LitBool(true).is_wildcard_like());
    }

    #[test]
    fn flat_pattern_is_constructor() {
        assert!(FlatPattern::LitInt(42).is_constructor());
        assert!(FlatPattern::LitBool(true).is_constructor());
        assert!(FlatPattern::Tuple(vec![]).is_constructor());
        assert!(FlatPattern::Variant {
            variant_name: Name::from_raw(1),
            variant_index: 0,
            fields: vec![],
        }
        .is_constructor());
        assert!(!FlatPattern::Wildcard.is_constructor());
        assert!(!FlatPattern::Binding(Name::from_raw(1)).is_constructor());
    }

    #[test]
    fn extract_bindings_wildcard() {
        let path: ScrutineePath = Vec::new();
        let bindings = FlatPattern::Wildcard.extract_bindings(&path);
        assert!(bindings.is_empty());
    }

    #[test]
    fn extract_bindings_binding() {
        let path: ScrutineePath = Vec::new();
        let name = Name::from_raw(42);
        let bindings = FlatPattern::Binding(name).extract_bindings(&path);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].0, name);
        assert!(bindings[0].1.is_empty());
    }

    #[test]
    fn extract_bindings_variant() {
        // Variant { Some(x) } at root path
        let path: ScrutineePath = Vec::new();
        let name_x = Name::from_raw(1);
        let pat = FlatPattern::Variant {
            variant_name: Name::from_raw(10),
            variant_index: 1,
            fields: vec![FlatPattern::Binding(name_x)],
        };
        let bindings = pat.extract_bindings(&path);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].0, name_x);
        // Path should be [TagPayload(0)] — first payload field.
        assert_eq!(bindings[0].1.len(), 1);
        assert_eq!(bindings[0].1[0], PathInstruction::TagPayload(0));
    }

    #[test]
    fn extract_bindings_nested_tuple() {
        // (a, (b, c))
        let name_a = Name::from_raw(1);
        let name_b = Name::from_raw(2);
        let name_c = Name::from_raw(3);
        let pat = FlatPattern::Tuple(vec![
            FlatPattern::Binding(name_a),
            FlatPattern::Tuple(vec![
                FlatPattern::Binding(name_b),
                FlatPattern::Binding(name_c),
            ]),
        ]);
        let path: ScrutineePath = Vec::new();
        let bindings = pat.extract_bindings(&path);
        assert_eq!(bindings.len(), 3);
        // a at [TupleIndex(0)]
        assert_eq!(bindings[0].0, name_a);
        assert_eq!(bindings[0].1.as_slice(), &[PathInstruction::TupleIndex(0)]);
        // b at [TupleIndex(1), TupleIndex(0)]
        assert_eq!(bindings[1].0, name_b);
        assert_eq!(
            bindings[1].1.as_slice(),
            &[
                PathInstruction::TupleIndex(1),
                PathInstruction::TupleIndex(0)
            ]
        );
        // c at [TupleIndex(1), TupleIndex(1)]
        assert_eq!(bindings[2].0, name_c);
        assert_eq!(
            bindings[2].1.as_slice(),
            &[
                PathInstruction::TupleIndex(1),
                PathInstruction::TupleIndex(1)
            ]
        );
    }

    #[test]
    fn extract_bindings_at_pattern() {
        // x @ Some(y)
        let name_x = Name::from_raw(1);
        let name_y = Name::from_raw(2);
        let pat = FlatPattern::At {
            name: name_x,
            inner: Box::new(FlatPattern::Variant {
                variant_name: Name::from_raw(10),
                variant_index: 1,
                fields: vec![FlatPattern::Binding(name_y)],
            }),
        };
        let path: ScrutineePath = Vec::new();
        let bindings = pat.extract_bindings(&path);
        assert_eq!(bindings.len(), 2);
        // x binds at root path (the whole scrutinee).
        assert_eq!(bindings[0].0, name_x);
        assert!(bindings[0].1.is_empty());
        // y binds at [TagPayload(0)].
        assert_eq!(bindings[1].0, name_y);
        assert_eq!(bindings[1].1.as_slice(), &[PathInstruction::TagPayload(0)]);
    }

    #[test]
    fn extract_bindings_or_uses_first_alternative() {
        // A(x) | B(y) — both bind at the same path
        let name_x = Name::from_raw(1);
        let name_y = Name::from_raw(2);
        let pat = FlatPattern::Or(vec![
            FlatPattern::Variant {
                variant_name: Name::from_raw(10),
                variant_index: 0,
                fields: vec![FlatPattern::Binding(name_x)],
            },
            FlatPattern::Variant {
                variant_name: Name::from_raw(11),
                variant_index: 1,
                fields: vec![FlatPattern::Binding(name_y)],
            },
        ]);
        let path: ScrutineePath = Vec::new();
        let bindings = pat.extract_bindings(&path);
        // Uses first alternative's bindings.
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].0, name_x);
    }

    #[test]
    fn extract_bindings_struct() {
        // { x, y: (a, _) }
        let name_x = Name::from_raw(1);
        let name_y = Name::from_raw(2);
        let name_a = Name::from_raw(3);
        let pat = FlatPattern::Struct {
            fields: vec![
                (Name::from_raw(10), FlatPattern::Binding(name_x)),
                (
                    name_y,
                    FlatPattern::Tuple(vec![FlatPattern::Binding(name_a), FlatPattern::Wildcard]),
                ),
            ],
        };
        let path: ScrutineePath = Vec::new();
        let bindings = pat.extract_bindings(&path);
        assert_eq!(bindings.len(), 2);
        // x at [StructField(0)]
        assert_eq!(bindings[0].0, name_x);
        assert_eq!(bindings[0].1.as_slice(), &[PathInstruction::StructField(0)]);
        // a at [StructField(1), TupleIndex(0)]
        assert_eq!(bindings[1].0, name_a);
        assert_eq!(
            bindings[1].1.as_slice(),
            &[
                PathInstruction::StructField(1),
                PathInstruction::TupleIndex(0)
            ]
        );
    }

    #[test]
    fn extract_bindings_list_with_rest() {
        // [head, ..rest]
        let name_head = Name::from_raw(1);
        let name_rest = Name::from_raw(2);
        let pat = FlatPattern::List {
            elements: vec![FlatPattern::Binding(name_head)],
            rest: Some(name_rest),
        };
        let path: ScrutineePath = Vec::new();
        let bindings = pat.extract_bindings(&path);
        assert_eq!(bindings.len(), 2);
        // head at [ListElement(0)]
        assert_eq!(bindings[0].0, name_head);
        assert_eq!(bindings[0].1.as_slice(), &[PathInstruction::ListElement(0)]);
        // rest at [ListRest(1)] — slice from index 1 onwards.
        assert_eq!(bindings[1].0, name_rest);
        assert_eq!(bindings[1].1.as_slice(), &[PathInstruction::ListRest(1)]);
    }

    // ── PatternRow ──────────────────────────────────────────────

    #[test]
    fn pattern_row_construction() {
        let row = PatternRow {
            patterns: vec![FlatPattern::Wildcard, FlatPattern::LitInt(42)],
            arm_index: 0,
            guard: None,
            bindings: vec![],
        };
        assert_eq!(row.patterns.len(), 2);
        assert_eq!(row.arm_index, 0);
        assert!(row.guard.is_none());
    }

    #[test]
    fn pattern_row_with_guard() {
        let row = PatternRow {
            patterns: vec![FlatPattern::Binding(Name::from_raw(1))],
            arm_index: 1,
            guard: Some(CanId::new(50)),
            bindings: vec![],
        };
        assert!(row.guard.is_some());
    }
}

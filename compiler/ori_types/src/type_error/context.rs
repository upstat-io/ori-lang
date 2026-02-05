//! Context kinds for type expectations.
//!
//! This module classifies WHERE in code a type is expected, enabling
//! precise error messages like "in the 2nd argument to `add`" or
//! "in the condition of this if expression".
//!
//! # Design
//!
//! 30+ context kinds cover all places where types are checked:
//! - Literals (list elements, map keys, tuple elements)
//! - Control flow (if conditions, match arms, loop bodies)
//! - Functions (arguments, returns, lambda bodies)
//! - Operators (binary, unary, pipeline)
//! - Records/Structs (field access, construction, updates)
//! - Patterns (bindings, destructuring, guards)
//! - Special (capabilities, contracts, tests)

use ori_ir::Name;

/// The kind of context that created a type expectation.
///
/// Used to generate precise error messages describing WHERE
/// a type mismatch occurred.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ContextKind {
    // ════════════════════════════════════════════════════════════════════════
    // Literals
    // ════════════════════════════════════════════════════════════════════════
    /// Element of a list literal.
    ListElement {
        /// Zero-based element index.
        index: usize,
    },

    /// Key in a map literal.
    MapKey,

    /// Value in a map literal.
    MapValue,

    /// Element of a tuple literal.
    TupleElement {
        /// Zero-based element index.
        index: usize,
    },

    /// Element of a set literal.
    SetElement,

    /// Element of a range expression.
    RangeElement,

    // ════════════════════════════════════════════════════════════════════════
    // Control Flow
    // ════════════════════════════════════════════════════════════════════════
    /// Condition of an if expression.
    IfCondition,

    /// Then branch of an if expression.
    IfThenBranch,

    /// Else branch of an if expression.
    IfElseBranch {
        /// Zero-based branch index (0 = first else-if, etc.).
        branch_index: usize,
    },

    /// Scrutinee of a match expression.
    MatchScrutinee,

    /// Body of a match arm.
    MatchArm {
        /// Zero-based arm index.
        arm_index: usize,
    },

    /// Pattern in a match arm.
    MatchArmPattern {
        /// Zero-based arm index.
        arm_index: usize,
    },

    /// Guard condition in a match arm.
    MatchArmGuard {
        /// Zero-based arm index.
        arm_index: usize,
    },

    /// Condition of a while loop.
    LoopCondition,

    /// Body of a loop (for, while, loop).
    LoopBody,

    /// Iterator in a for loop.
    ForIterator,

    /// Binding pattern in a for loop.
    ForBinding,

    // ════════════════════════════════════════════════════════════════════════
    // Functions
    // ════════════════════════════════════════════════════════════════════════
    /// Argument to a function call.
    FunctionArgument {
        /// Name of the function being called (if known).
        func_name: Option<Name>,
        /// Zero-based argument index.
        arg_index: usize,
        /// Name of the parameter (if known).
        param_name: Option<Name>,
    },

    /// Return value of a function.
    FunctionReturn {
        /// Name of the function.
        func_name: Option<Name>,
    },

    /// Body of a lambda expression.
    LambdaBody,

    /// Parameter of a lambda expression.
    LambdaParameter {
        /// Zero-based parameter index.
        index: usize,
    },

    /// Implicit return of a lambda.
    LambdaReturn,

    /// Receiver of a method call (the value before the dot).
    MethodReceiver {
        /// Name of the method being called.
        method_name: Name,
    },

    // ════════════════════════════════════════════════════════════════════════
    // Operators
    // ════════════════════════════════════════════════════════════════════════
    /// Left operand of a binary operator.
    BinaryOpLeft {
        /// String representation of the operator.
        op: &'static str,
    },

    /// Right operand of a binary operator.
    BinaryOpRight {
        /// String representation of the operator.
        op: &'static str,
    },

    /// Operand of a unary operator.
    UnaryOpOperand {
        /// String representation of the operator.
        op: &'static str,
    },

    /// Input to a pipeline.
    PipelineInput,

    /// Output from a pipeline stage.
    PipelineOutput,

    /// Left side of a comparison.
    ComparisonLeft,

    /// Right side of a comparison.
    ComparisonRight,

    // ════════════════════════════════════════════════════════════════════════
    // Records/Structs
    // ════════════════════════════════════════════════════════════════════════
    /// Accessing a field on a value.
    FieldAccess {
        /// Name of the field being accessed.
        field_name: Name,
    },

    /// Assigning to a field.
    FieldAssignment {
        /// Name of the field being assigned.
        field_name: Name,
    },

    /// Field in a struct construction.
    StructField {
        /// Name of the struct type.
        struct_name: Name,
        /// Name of the field.
        field_name: Name,
    },

    /// Record update expression (spreading).
    RecordUpdate {
        /// Name of the field being updated.
        field_name: Name,
    },

    /// Struct construction.
    StructConstruction {
        /// Name of the struct type.
        struct_name: Name,
    },

    // ════════════════════════════════════════════════════════════════════════
    // Patterns
    // ════════════════════════════════════════════════════════════════════════
    /// Binding in a pattern.
    PatternBinding {
        /// Kind of pattern (e.g., "let", "match", "function parameter").
        pattern_kind: &'static str,
    },

    /// Pattern matching against a type.
    PatternMatch {
        /// Kind of pattern being matched.
        pattern_kind: &'static str,
    },

    /// Destructuring pattern.
    Destructure,

    /// Start of a range pattern.
    RangeStart,

    /// End of a range pattern.
    RangeEnd,

    // ════════════════════════════════════════════════════════════════════════
    // Special
    // ════════════════════════════════════════════════════════════════════════
    /// Capability requirement in a function signature.
    CapabilityRequirement {
        /// Name of the capability.
        capability: Name,
    },

    /// Pre-condition check.
    PreCheck,

    /// Post-condition check.
    PostCheck,

    /// Body of a test function.
    TestBody,

    /// Assertion in a test.
    TestAssertion,

    /// Assignment to a variable.
    Assignment,

    /// Index operation (e.g., `list[i]`).
    IndexOperation,

    /// Index value in an index operation.
    IndexValue,

    /// Spread element in a list/array.
    SpreadElement,

    /// Return statement (in imperative contexts).
    ReturnStatement,

    /// Break value from a loop.
    BreakValue,

    /// Throw/raise expression.
    ThrowExpression,

    /// Try/catch expression.
    TryExpression,

    /// With expression (capability scoping).
    WithExpression,
}

impl ContextKind {
    /// Get a human-readable description of this context for error messages.
    ///
    /// Returns a phrase like "in the condition of this if expression".
    pub fn describe(&self) -> String {
        match self {
            // Literals
            Self::ListElement { index } => {
                format!("in the {} element of this list", ordinal(*index + 1))
            }
            Self::MapKey => "in a map key".to_string(),
            Self::MapValue => "in a map value".to_string(),
            Self::TupleElement { index } => {
                format!("in the {} element of this tuple", ordinal(*index + 1))
            }
            Self::SetElement => "in a set element".to_string(),
            Self::RangeElement => "in a range element".to_string(),

            // Control flow
            Self::IfCondition => "in the condition of this if expression".to_string(),
            Self::IfThenBranch => "in the then branch".to_string(),
            Self::IfElseBranch { branch_index } => {
                if *branch_index == 0 {
                    "in the else branch".to_string()
                } else {
                    format!("in the {} else-if branch", ordinal(*branch_index + 1))
                }
            }
            Self::MatchScrutinee => "in the match scrutinee".to_string(),
            Self::MatchArm { arm_index } => {
                format!("in the {} match arm", ordinal(*arm_index + 1))
            }
            Self::MatchArmPattern { arm_index } => {
                format!(
                    "in the pattern of the {} match arm",
                    ordinal(*arm_index + 1)
                )
            }
            Self::MatchArmGuard { arm_index } => {
                format!("in the guard of the {} match arm", ordinal(*arm_index + 1))
            }
            Self::LoopCondition => "in the loop condition".to_string(),
            Self::LoopBody => "in the loop body".to_string(),
            Self::ForIterator => "in the for loop iterator".to_string(),
            Self::ForBinding => "in the for loop binding".to_string(),

            // Functions
            Self::FunctionArgument {
                func_name: _,
                arg_index,
                param_name: _,
            } => {
                // Note: func_name and param_name need StringInterner to display
                format!("in the {} argument", ordinal(*arg_index + 1))
            }
            Self::FunctionReturn { .. } => "in the return value".to_string(),
            Self::LambdaBody => "in the lambda body".to_string(),
            Self::LambdaParameter { index } => {
                format!("in the {} lambda parameter", ordinal(*index + 1))
            }
            Self::LambdaReturn => "in the lambda return".to_string(),
            Self::MethodReceiver { .. } => "in the method receiver".to_string(),

            // Operators
            Self::BinaryOpLeft { op } => format!("in the left operand of `{op}`"),
            Self::BinaryOpRight { op } => format!("in the right operand of `{op}`"),
            Self::UnaryOpOperand { op } => format!("in the operand of `{op}`"),
            Self::PipelineInput => "in the pipeline input".to_string(),
            Self::PipelineOutput => "in the pipeline output".to_string(),
            Self::ComparisonLeft => "in the left side of the comparison".to_string(),
            Self::ComparisonRight => "in the right side of the comparison".to_string(),

            // Records/Structs
            Self::FieldAccess { .. } => "in a field access".to_string(),
            Self::FieldAssignment { .. } => "in a field assignment".to_string(),
            Self::StructField { .. } => "in a struct field".to_string(),
            Self::RecordUpdate { .. } => "in a record update".to_string(),
            Self::StructConstruction { .. } => "in struct construction".to_string(),

            // Patterns
            Self::PatternBinding { pattern_kind } => {
                format!("in a {pattern_kind} pattern binding")
            }
            Self::PatternMatch { pattern_kind } => {
                format!("in a {pattern_kind} pattern match")
            }
            Self::Destructure => "in a destructuring pattern".to_string(),
            Self::RangeStart => "in the start of a range pattern".to_string(),
            Self::RangeEnd => "in the end of a range pattern".to_string(),

            // Special
            Self::CapabilityRequirement { .. } => "in a capability requirement".to_string(),
            Self::PreCheck => "in a pre-condition check".to_string(),
            Self::PostCheck => "in a post-condition check".to_string(),
            Self::TestBody => "in a test body".to_string(),
            Self::TestAssertion => "in a test assertion".to_string(),
            Self::Assignment => "in an assignment".to_string(),
            Self::IndexOperation => "in an index operation".to_string(),
            Self::IndexValue => "in an index value".to_string(),
            Self::SpreadElement => "in a spread element".to_string(),
            Self::ReturnStatement => "in a return statement".to_string(),
            Self::BreakValue => "in a break value".to_string(),
            Self::ThrowExpression => "in a throw expression".to_string(),
            Self::TryExpression => "in a try expression".to_string(),
            Self::WithExpression => "in a with expression".to_string(),
        }
    }

    /// Get the reason WHY this context expects a particular type.
    ///
    /// Used by `ExpectedOrigin::Context` to explain expectations.
    pub fn expectation_reason(&self) -> &'static str {
        match self {
            // Literals
            Self::ListElement { .. } => "all list elements must have the same type",
            Self::MapKey => "map keys must be hashable",
            Self::MapValue => "this is the map value type",
            Self::TupleElement { .. } => "tuples have fixed element types",
            Self::SetElement => "set elements must be hashable",
            Self::RangeElement => "range bounds must be the same type",

            // Control flow
            Self::IfCondition => "if conditions must be bool",
            Self::IfThenBranch | Self::IfElseBranch { .. } => {
                "all branches must return the same type"
            }
            Self::MatchScrutinee => "match scrutinee determines pattern types",
            Self::MatchArm { .. } => "all match arms must return the same type",
            Self::MatchArmPattern { .. } => "pattern must match the scrutinee type",
            Self::MatchArmGuard { .. } => "guards must be bool",
            Self::LoopCondition => "loop conditions must be bool",
            Self::LoopBody => "this is the loop body",
            Self::ForIterator => "for loops require an iterable",
            Self::ForBinding => "binding must match iterator element type",

            // Functions
            Self::FunctionArgument { .. } => "argument must match parameter type",
            Self::FunctionReturn { .. } => "return value must match declared type",
            Self::LambdaBody => "lambda body determines return type",
            Self::LambdaParameter { .. } => "parameter type is fixed",
            Self::LambdaReturn => "lambda return type is fixed",
            Self::MethodReceiver { .. } => "method requires this receiver type",

            // Operators
            Self::BinaryOpLeft { .. } | Self::UnaryOpOperand { .. } => {
                "operator requires this type"
            }
            Self::BinaryOpRight { .. } => "operands must be compatible",
            Self::PipelineInput => "pipeline stage expects this input",
            Self::PipelineOutput => "pipeline produces this output",
            Self::ComparisonLeft => "comparison requires comparable types",
            Self::ComparisonRight => "both sides must be the same type",

            // Records/Structs
            Self::FieldAccess { .. } | Self::FieldAssignment { .. } => "field has this type",
            Self::StructField { .. } => "struct field has this type",
            Self::RecordUpdate { .. } => "updated field must match original type",
            Self::StructConstruction { .. } => "struct requires these field types",

            // Patterns
            Self::PatternBinding { .. } => "binding has this type",
            Self::PatternMatch { .. } => "pattern must match value type",
            Self::Destructure => "destructure pattern must match type",
            Self::RangeStart | Self::RangeEnd => "range bounds must match",

            // Special
            Self::CapabilityRequirement { .. } => "capability requires this",
            Self::PreCheck => "pre-conditions must be bool",
            Self::PostCheck => "post-conditions must be bool",
            Self::TestBody => "test body must return void",
            Self::TestAssertion => "assertions must be bool",
            Self::Assignment => "assigned value must match variable type",
            Self::IndexOperation => "container requires this index type",
            Self::IndexValue => "index must be int",
            Self::SpreadElement => "spread element must match container type",
            Self::ReturnStatement => "return value must match function type",
            Self::BreakValue => "break value must match loop type",
            Self::ThrowExpression => "throw requires error type",
            Self::TryExpression => "try expects result type",
            Self::WithExpression => "with requires capability scope",
        }
    }

    /// Check if this context is within a function call.
    pub fn is_function_call(&self) -> bool {
        matches!(
            self,
            Self::FunctionArgument { .. }
                | Self::FunctionReturn { .. }
                | Self::MethodReceiver { .. }
        )
    }

    /// Check if this context is within a control flow construct.
    pub fn is_control_flow(&self) -> bool {
        matches!(
            self,
            Self::IfCondition
                | Self::IfThenBranch
                | Self::IfElseBranch { .. }
                | Self::MatchScrutinee
                | Self::MatchArm { .. }
                | Self::MatchArmPattern { .. }
                | Self::MatchArmGuard { .. }
                | Self::LoopCondition
                | Self::LoopBody
                | Self::ForIterator
                | Self::ForBinding
        )
    }

    /// Check if this context expects a bool type.
    pub fn expects_bool(&self) -> bool {
        matches!(
            self,
            Self::IfCondition
                | Self::LoopCondition
                | Self::MatchArmGuard { .. }
                | Self::PreCheck
                | Self::PostCheck
                | Self::TestAssertion
        )
    }
}

/// Convert a 1-based index to an ordinal string ("1st", "2nd", "3rd", etc.).
fn ordinal(n: usize) -> String {
    let suffix = match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_descriptions() {
        assert_eq!(
            ContextKind::IfCondition.describe(),
            "in the condition of this if expression"
        );

        assert_eq!(
            ContextKind::ListElement { index: 0 }.describe(),
            "in the 1st element of this list"
        );

        assert_eq!(
            ContextKind::ListElement { index: 2 }.describe(),
            "in the 3rd element of this list"
        );

        assert_eq!(
            ContextKind::MatchArm { arm_index: 0 }.describe(),
            "in the 1st match arm"
        );

        assert_eq!(
            ContextKind::BinaryOpLeft { op: "+" }.describe(),
            "in the left operand of `+`"
        );
    }

    #[test]
    fn context_expectation_reasons() {
        assert_eq!(
            ContextKind::IfCondition.expectation_reason(),
            "if conditions must be bool"
        );

        assert_eq!(
            ContextKind::ListElement { index: 0 }.expectation_reason(),
            "all list elements must have the same type"
        );
    }

    #[test]
    fn context_category_checks() {
        assert!(ContextKind::IfCondition.expects_bool());
        assert!(ContextKind::LoopCondition.expects_bool());
        assert!(!ContextKind::ListElement { index: 0 }.expects_bool());

        assert!(ContextKind::IfCondition.is_control_flow());
        assert!(!ContextKind::ListElement { index: 0 }.is_control_flow());

        assert!(ContextKind::FunctionArgument {
            func_name: None,
            arg_index: 0,
            param_name: None,
        }
        .is_function_call());
    }
}

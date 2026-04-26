//! Method implementations for `ContextKind`.
//!
//! Split from `mod.rs` per BUG-02-013 §06 hygiene finding F-06 (file-length BLOAT).
//! Houses the `impl ContextKind` block (`describe`, `expectation_reason`,
//! `is_function_call`, `is_control_flow`, `expects_bool`) plus the `ordinal()`
//! free helper used by `describe`. Identifier visibility is unchanged: methods
//! are pub via the parent enum, `ordinal` stays private to this module.

use super::ContextKind;

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
            Self::HigherOrderClosureReturn { adapter_name } => {
                format!("in the closure return of `{adapter_name}`")
            }
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
            Self::IndexKey => "in the index key".to_string(),
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
            Self::HigherOrderClosureReturn { .. } => "closure must return an iterator type",
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
            Self::PostCheck => "post-conditions must be a predicate on the result",
            Self::TestBody => "test body must return void",
            Self::TestAssertion => "assertions must be bool",
            Self::Assignment => "assigned value must match variable type",
            Self::IndexOperation => "container requires this index type",
            Self::IndexValue => "index must be int",
            Self::IndexKey => "key type must match the Index implementation",
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

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
    // Literals
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

    // Control Flow
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

    // Functions
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

    /// Closure return position inside a higher-order iterator adapter
    /// (e.g., `flat_map(transform: x -> ...)`). Distinct from `LambdaReturn`
    /// because the closure's return type has a structural requirement
    /// (must be `Iterator<U>`) imposed by the enclosing adapter.
    HigherOrderClosureReturn {
        /// Adapter method name as a static string (e.g., `"flat_map"`).
        adapter_name: &'static str,
    },

    /// Receiver of a method call (the value before the dot).
    MethodReceiver {
        /// Name of the method being called.
        method_name: Name,
    },

    // Operators
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

    // Records/Structs
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

    // Patterns
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

    // Special
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

    /// Key expression in a subscript operation `x[key]` dispatched via Index trait.
    IndexKey,

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

mod methods;

#[cfg(test)]
mod tests;

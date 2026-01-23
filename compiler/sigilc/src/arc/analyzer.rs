// Exhaustive ARC Analyzer
//
// This module provides the concrete implementation of all exhaustive classifier
// traits. The ExhaustiveArcAnalyzer handles every IR variant explicitly, making
// it impossible to forget ARC handling when new variants are added.

use crate::ast::{BinaryOp, UnaryOp};
use crate::ir::{
    FuncRef, IterDirection, LocalId, OnError, RetryBackoff, TExpr, TExprKind, TFunction, TMatch,
    TModule, TPattern, TStmt, TTest, Type,
};

use super::analysis::{DefaultTypeClassifier, TypeSizeCalculator};
use super::classifier::{
    classify_expr, classify_pattern, classify_type, ArcExprClassifier, ArcMatchPatternClassifier,
    ArcPatternClassifier, ArcTypeClassifier, ChildVisit, ExprArcInfo, MatchPatternArcInfo,
    PatternArcInfo, TypeArcInfo,
};
use super::traits::{StorageClass, TypeClassifier};
use super::validated::{ArcResult, FunctionArcInfo, LocalArcInfo, ModuleArcInfo};

// =============================================================================
// Exhaustive ARC Analyzer
// =============================================================================

/// The main ARC analyzer that exhaustively classifies all IR variants.
///
/// This type implements all four classifier traits:
/// - ArcExprClassifier: Handles all 34 TExprKind variants
/// - ArcTypeClassifier: Handles all 18 Type variants
/// - ArcPatternClassifier: Handles all 13 TPattern variants
/// - ArcMatchPatternClassifier: Handles all 5 TMatchPattern variants
///
/// Adding a new variant to any of these enums will cause a Rust compile error
/// in the corresponding dispatch function until the variant is handled here.
pub struct ExhaustiveArcAnalyzer {
    /// Type classifier for determining storage classes
    type_classifier: DefaultTypeClassifier,

    /// Size calculator for type sizes
    size_calc: TypeSizeCalculator,
}

impl ExhaustiveArcAnalyzer {
    /// Create a new exhaustive analyzer
    pub fn new() -> Self {
        ExhaustiveArcAnalyzer {
            type_classifier: DefaultTypeClassifier::new(),
            size_calc: TypeSizeCalculator::new(),
        }
    }

    /// Analyze an entire module for ARC requirements
    pub fn analyze_module(&self, module: &TModule) -> ArcResult<ModuleArcInfo> {
        let mut arc_info = ModuleArcInfo::new();

        // Analyze all functions
        for func in &module.functions {
            let func_info = self.analyze_function(func)?;
            arc_info.functions.insert(func.name.clone(), func_info);
        }

        // Analyze all tests
        for test in &module.tests {
            let test_info = self.analyze_test(test)?;
            arc_info.tests.insert(test.name.clone(), test_info);
        }

        // Cache type classifications for user-defined types
        for type_def in &module.types {
            let ty = match &type_def.kind {
                crate::ir::TTypeDefKind::Struct(fields) => Type::Struct {
                    name: type_def.name.clone(),
                    fields: fields
                        .iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect(),
                },
                crate::ir::TTypeDefKind::Enum(variants) => Type::Enum {
                    name: type_def.name.clone(),
                    variants: variants
                        .iter()
                        .map(|v| {
                            (
                                v.name.clone(),
                                v.fields
                                    .iter()
                                    .map(|f| (f.name.clone(), f.ty.clone()))
                                    .collect(),
                            )
                        })
                        .collect(),
                },
                crate::ir::TTypeDefKind::Alias(target) => target.clone(),
            };
            let classification = self.type_classifier.classify(&ty);
            arc_info
                .type_classes
                .insert(type_def.name.clone(), classification);
        }

        Ok(arc_info)
    }

    /// Analyze a single function
    fn analyze_function(&self, func: &TFunction) -> ArcResult<FunctionArcInfo> {
        let mut func_info = FunctionArcInfo::new(func.name.clone());

        // Analyze locals
        for (local_id, local_info) in func.locals.iter() {
            let type_info = classify_type(self, &local_info.ty);

            func_info.local_arc_info.insert(
                local_id,
                LocalArcInfo {
                    local_id,
                    ty: local_info.ty.clone(),
                    storage: type_info.storage_class,
                    needs_arc: type_info.needs_arc,
                    needs_destruction: type_info.needs_destruction,
                },
            );

            if type_info.needs_arc {
                func_info.ref_type_locals += 1;
            }
        }

        // Analyze function body
        self.analyze_expr(&func.body, &mut func_info)?;

        Ok(func_info)
    }

    /// Analyze a test
    fn analyze_test(&self, test: &TTest) -> ArcResult<FunctionArcInfo> {
        let mut func_info = FunctionArcInfo::new(test.name.clone());

        // Analyze locals
        for (local_id, local_info) in test.locals.iter() {
            let type_info = classify_type(self, &local_info.ty);

            func_info.local_arc_info.insert(
                local_id,
                LocalArcInfo {
                    local_id,
                    ty: local_info.ty.clone(),
                    storage: type_info.storage_class,
                    needs_arc: type_info.needs_arc,
                    needs_destruction: type_info.needs_destruction,
                },
            );

            if type_info.needs_arc {
                func_info.ref_type_locals += 1;
            }
        }

        // Analyze test body
        self.analyze_expr(&test.body, &mut func_info)?;

        Ok(func_info)
    }

    /// Analyze an expression recursively
    fn analyze_expr(&self, expr: &TExpr, func_info: &mut FunctionArcInfo) -> ArcResult<ExprArcInfo> {
        // Use the exhaustive classifier
        let info = classify_expr(self, expr);

        // Recursively analyze children
        self.analyze_expr_children(expr, func_info)?;

        Ok(info)
    }

    /// Analyze children of an expression
    fn analyze_expr_children(
        &self,
        expr: &TExpr,
        func_info: &mut FunctionArcInfo,
    ) -> ArcResult<()> {
        match &expr.kind {
            // Literals have no children
            TExprKind::Int(_)
            | TExprKind::Float(_)
            | TExprKind::String(_)
            | TExprKind::Bool(_)
            | TExprKind::Nil
            | TExprKind::None_ => {}

            // Variables have no children
            TExprKind::Local(_) | TExprKind::Param(_) | TExprKind::Config(_) => {}

            // Collections
            TExprKind::List(elems) => {
                for elem in elems {
                    self.analyze_expr(elem, func_info)?;
                }
            }
            TExprKind::MapLiteral(entries) => {
                for (k, v) in entries {
                    self.analyze_expr(k, func_info)?;
                    self.analyze_expr(v, func_info)?;
                }
            }
            TExprKind::Tuple(elems) => {
                for elem in elems {
                    self.analyze_expr(elem, func_info)?;
                }
            }
            TExprKind::Struct { fields, .. } => {
                for (_, value) in fields {
                    self.analyze_expr(value, func_info)?;
                }
            }

            // Operations
            TExprKind::Binary { left, right, .. } => {
                self.analyze_expr(left, func_info)?;
                self.analyze_expr(right, func_info)?;
            }
            TExprKind::Unary { operand, .. } => {
                self.analyze_expr(operand, func_info)?;
            }

            // Access
            TExprKind::Field(expr, _) => {
                self.analyze_expr(expr, func_info)?;
            }
            TExprKind::Index(expr, idx) => {
                self.analyze_expr(expr, func_info)?;
                self.analyze_expr(idx, func_info)?;
            }
            TExprKind::LengthOf(expr) => {
                self.analyze_expr(expr, func_info)?;
            }

            // Calls
            TExprKind::Call { args, .. } => {
                for arg in args {
                    self.analyze_expr(arg, func_info)?;
                }
            }
            TExprKind::MethodCall { receiver, args, .. } => {
                self.analyze_expr(receiver, func_info)?;
                for arg in args {
                    self.analyze_expr(arg, func_info)?;
                }
            }

            // Lambda
            TExprKind::Lambda { body, .. } => {
                self.analyze_expr(body, func_info)?;
            }

            // Control flow
            TExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.analyze_expr(cond, func_info)?;
                self.analyze_expr(then_branch, func_info)?;
                self.analyze_expr(else_branch, func_info)?;
            }
            TExprKind::Match(match_expr) => {
                self.analyze_expr(&match_expr.scrutinee, func_info)?;
                for arm in &match_expr.arms {
                    self.analyze_expr(&arm.body, func_info)?;
                }
            }
            TExprKind::Block(stmts, result) => {
                for stmt in stmts {
                    match stmt {
                        TStmt::Expr(expr) => {
                            self.analyze_expr(expr, func_info)?;
                        }
                        TStmt::Let { value, .. } => {
                            self.analyze_expr(value, func_info)?;
                        }
                    }
                }
                self.analyze_expr(result, func_info)?;
            }
            TExprKind::For { iter, body, .. } => {
                self.analyze_expr(iter, func_info)?;
                self.analyze_expr(body, func_info)?;
            }

            // Assignment
            TExprKind::Assign { value, .. } => {
                self.analyze_expr(value, func_info)?;
            }

            // Range
            TExprKind::Range { start, end } => {
                self.analyze_expr(start, func_info)?;
                self.analyze_expr(end, func_info)?;
            }

            // Patterns
            TExprKind::Pattern(pattern) => {
                self.analyze_pattern_children(pattern, func_info)?;
            }

            // Result/Option constructors
            TExprKind::Ok(v) | TExprKind::Err(v) | TExprKind::Some(v) | TExprKind::Unwrap(v) => {
                self.analyze_expr(v, func_info)?;
            }
            TExprKind::Coalesce { value, default } => {
                self.analyze_expr(value, func_info)?;
                self.analyze_expr(default, func_info)?;
            }

            // Capability
            TExprKind::With {
                implementation,
                body,
                ..
            } => {
                self.analyze_expr(implementation, func_info)?;
                self.analyze_expr(body, func_info)?;
            }
        }
        Ok(())
    }

    /// Analyze children of a pattern
    fn analyze_pattern_children(
        &self,
        pattern: &TPattern,
        func_info: &mut FunctionArcInfo,
    ) -> ArcResult<()> {
        match pattern {
            TPattern::Fold {
                collection,
                init,
                op,
                ..
            } => {
                self.analyze_expr(collection, func_info)?;
                self.analyze_expr(init, func_info)?;
                self.analyze_expr(op, func_info)?;
            }
            TPattern::Map {
                collection,
                transform,
                ..
            } => {
                self.analyze_expr(collection, func_info)?;
                self.analyze_expr(transform, func_info)?;
            }
            TPattern::Filter {
                collection,
                predicate,
                ..
            } => {
                self.analyze_expr(collection, func_info)?;
                self.analyze_expr(predicate, func_info)?;
            }
            TPattern::Collect {
                range, transform, ..
            } => {
                self.analyze_expr(range, func_info)?;
                self.analyze_expr(transform, func_info)?;
            }
            TPattern::Recurse {
                cond, base, step, ..
            } => {
                self.analyze_expr(cond, func_info)?;
                self.analyze_expr(base, func_info)?;
                self.analyze_expr(step, func_info)?;
            }
            TPattern::Iterate {
                over, into, with, ..
            } => {
                self.analyze_expr(over, func_info)?;
                self.analyze_expr(into, func_info)?;
                self.analyze_expr(with, func_info)?;
            }
            TPattern::Transform { input, steps, .. } => {
                self.analyze_expr(input, func_info)?;
                for step in steps {
                    self.analyze_expr(step, func_info)?;
                }
            }
            TPattern::Count {
                collection,
                predicate,
                ..
            } => {
                self.analyze_expr(collection, func_info)?;
                self.analyze_expr(predicate, func_info)?;
            }
            TPattern::Parallel {
                branches, timeout, ..
            } => {
                for (_, expr, _) in branches {
                    self.analyze_expr(expr, func_info)?;
                }
                if let Some(t) = timeout {
                    self.analyze_expr(t, func_info)?;
                }
            }
            TPattern::Find {
                collection,
                predicate,
                default,
                ..
            } => {
                self.analyze_expr(collection, func_info)?;
                self.analyze_expr(predicate, func_info)?;
                if let Some(d) = default {
                    self.analyze_expr(d, func_info)?;
                }
            }
            TPattern::Try { body, catch, .. } => {
                self.analyze_expr(body, func_info)?;
                if let Some(c) = catch {
                    self.analyze_expr(c, func_info)?;
                }
            }
            TPattern::Retry {
                operation,
                max_attempts,
                delay_ms,
                ..
            } => {
                self.analyze_expr(operation, func_info)?;
                self.analyze_expr(max_attempts, func_info)?;
                if let Some(d) = delay_ms {
                    self.analyze_expr(d, func_info)?;
                }
            }
            TPattern::Validate {
                rules, then_value, ..
            } => {
                for (cond, err) in rules {
                    self.analyze_expr(cond, func_info)?;
                    self.analyze_expr(err, func_info)?;
                }
                self.analyze_expr(then_value, func_info)?;
            }
        }
        Ok(())
    }

    /// Determine storage class from a type
    fn storage_for_type(&self, ty: &Type) -> StorageClass {
        self.type_classifier.classify(ty).storage
    }

    /// Check if a type needs ARC
    fn type_needs_arc(&self, ty: &Type) -> bool {
        !self.type_classifier.is_value_type(ty)
    }
}

impl Default for ExhaustiveArcAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ArcExprClassifier Implementation (34 methods for 34 variants)
// =============================================================================

impl ArcExprClassifier for ExhaustiveArcAnalyzer {
    // =========================================================================
    // Literals
    // =========================================================================

    fn classify_int_literal(&self, _value: i64, _ty: &Type) -> ExprArcInfo {
        ExprArcInfo::value()
    }

    fn classify_float_literal(&self, _value: f64, _ty: &Type) -> ExprArcInfo {
        ExprArcInfo::value()
    }

    fn classify_string_literal(&self, _value: &str, _ty: &Type) -> ExprArcInfo {
        // String literals create new strings that need ARC
        ExprArcInfo::reference()
    }

    fn classify_bool_literal(&self, _value: bool, _ty: &Type) -> ExprArcInfo {
        ExprArcInfo::value()
    }

    fn classify_nil(&self, _ty: &Type) -> ExprArcInfo {
        ExprArcInfo::value()
    }

    // =========================================================================
    // Variables
    // =========================================================================

    fn classify_local(&self, _id: LocalId, ty: &Type) -> ExprArcInfo {
        // Local variable access - needs retain if reference type
        if self.type_needs_arc(ty) {
            ExprArcInfo {
                needs_retain: true,  // Reading a local needs retain
                needs_release: false, // Release is handled by scope exit
                storage_class: StorageClass::Reference,
                children_to_visit: Vec::new(),
            }
        } else {
            ExprArcInfo::value()
        }
    }

    fn classify_param(&self, _index: usize, ty: &Type) -> ExprArcInfo {
        // Parameter access - similar to local
        if self.type_needs_arc(ty) {
            ExprArcInfo {
                needs_retain: true,
                needs_release: false,
                storage_class: StorageClass::Reference,
                children_to_visit: Vec::new(),
            }
        } else {
            ExprArcInfo::value()
        }
    }

    fn classify_config(&self, _name: &str, ty: &Type) -> ExprArcInfo {
        // Config variables are compile-time constants, usually value types
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference()
        } else {
            ExprArcInfo::value()
        }
    }

    // =========================================================================
    // Collections
    // =========================================================================

    fn classify_list(&self, elements: &[TExpr], _ty: &Type) -> ExprArcInfo {
        // List creation always creates a reference type
        ExprArcInfo::reference().with_children(
            elements
                .iter()
                .map(|_| ChildVisit::owned("list element"))
                .collect(),
        )
    }

    fn classify_map_literal(&self, entries: &[(TExpr, TExpr)], _ty: &Type) -> ExprArcInfo {
        // Map creation always creates a reference type
        let mut children = Vec::new();
        for _ in entries {
            children.push(ChildVisit::owned("map key"));
            children.push(ChildVisit::owned("map value"));
        }
        ExprArcInfo::reference().with_children(children)
    }

    fn classify_tuple(&self, elements: &[TExpr], ty: &Type) -> ExprArcInfo {
        // Tuple is value or reference based on size and contents
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(
                elements
                    .iter()
                    .map(|_| ChildVisit::owned("tuple element"))
                    .collect(),
            )
        } else {
            ExprArcInfo::value()
        }
    }

    fn classify_struct(&self, _name: &str, fields: &[(String, TExpr)], ty: &Type) -> ExprArcInfo {
        // Struct is value or reference based on size and contents
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(
                fields
                    .iter()
                    .map(|(name, _)| ChildVisit::owned(Box::leak(format!("field {}", name).into_boxed_str())))
                    .collect(),
            )
        } else {
            ExprArcInfo::value()
        }
    }

    // =========================================================================
    // Operations
    // =========================================================================

    fn classify_binary_op(
        &self,
        op: &BinaryOp,
        _left: &TExpr,
        _right: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo {
        // Most binary ops return value types, but string concat returns reference
        match op {
            BinaryOp::Add if matches!(ty, Type::Str) => {
                // String concatenation creates a new string
                ExprArcInfo::reference().with_children(vec![
                    ChildVisit::borrowed("left operand"),
                    ChildVisit::borrowed("right operand"),
                ])
            }
            _ => {
                // Numeric and comparison ops return values
                ExprArcInfo::value().with_children(vec![
                    ChildVisit::borrowed("left operand"),
                    ChildVisit::borrowed("right operand"),
                ])
            }
        }
    }

    fn classify_unary_op(&self, _op: &UnaryOp, _operand: &TExpr, _ty: &Type) -> ExprArcInfo {
        // Unary ops always return value types
        ExprArcInfo::value().with_children(vec![ChildVisit::borrowed("operand")])
    }

    // =========================================================================
    // Access
    // =========================================================================

    fn classify_field(&self, _expr: &TExpr, _field: &str, ty: &Type) -> ExprArcInfo {
        // Field access may return reference type
        if self.type_needs_arc(ty) {
            ExprArcInfo {
                needs_retain: true,  // Need to retain the field value
                needs_release: false, // Handled by scope
                storage_class: StorageClass::Reference,
                children_to_visit: vec![ChildVisit::borrowed("receiver")],
            }
        } else {
            ExprArcInfo::value().with_children(vec![ChildVisit::borrowed("receiver")])
        }
    }

    fn classify_index(&self, _expr: &TExpr, _index: &TExpr, ty: &Type) -> ExprArcInfo {
        // Index access may return reference type
        if self.type_needs_arc(ty) {
            ExprArcInfo {
                needs_retain: true,
                needs_release: false,
                storage_class: StorageClass::Reference,
                children_to_visit: vec![
                    ChildVisit::borrowed("collection"),
                    ChildVisit::borrowed("index"),
                ],
            }
        } else {
            ExprArcInfo::value().with_children(vec![
                ChildVisit::borrowed("collection"),
                ChildVisit::borrowed("index"),
            ])
        }
    }

    fn classify_length_of(&self, _expr: &TExpr, _ty: &Type) -> ExprArcInfo {
        // Length always returns int
        ExprArcInfo::value().with_children(vec![ChildVisit::borrowed("collection")])
    }

    // =========================================================================
    // Calls
    // =========================================================================

    fn classify_call(&self, _func: &FuncRef, args: &[TExpr], ty: &Type) -> ExprArcInfo {
        // Function call result depends on return type
        let children = args.iter().map(|_| ChildVisit::owned("argument")).collect();

        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(children)
        } else {
            ExprArcInfo::value().with_children(children)
        }
    }

    fn classify_method_call(
        &self,
        _receiver: &TExpr,
        _method: &str,
        args: &[TExpr],
        ty: &Type,
    ) -> ExprArcInfo {
        let mut children = vec![ChildVisit::borrowed("receiver")];
        children.extend(args.iter().map(|_| ChildVisit::owned("argument")));

        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(children)
        } else {
            ExprArcInfo::value().with_children(children)
        }
    }

    // =========================================================================
    // Lambda
    // =========================================================================

    fn classify_lambda(
        &self,
        _params: &[(String, Type)],
        captures: &[LocalId],
        _body: &TExpr,
        _ty: &Type,
    ) -> ExprArcInfo {
        // Lambdas are always reference types (closures may capture)
        let children: Vec<_> = captures
            .iter()
            .map(|_| ChildVisit::owned("capture"))
            .collect();

        ExprArcInfo::reference().with_children(children)
    }

    // =========================================================================
    // Control Flow
    // =========================================================================

    fn classify_if(
        &self,
        _cond: &TExpr,
        _then_branch: &TExpr,
        _else_branch: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo {
        let children = vec![
            ChildVisit::borrowed("condition"),
            ChildVisit::owned("then branch"),
            ChildVisit::owned("else branch"),
        ];

        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(children)
        } else {
            ExprArcInfo::value().with_children(children)
        }
    }

    fn classify_match(&self, _match_expr: &TMatch, ty: &Type) -> ExprArcInfo {
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference()
        } else {
            ExprArcInfo::value()
        }
    }

    fn classify_block(&self, _stmts: &[TStmt], _result: &TExpr, ty: &Type) -> ExprArcInfo {
        // Block result type determines ARC needs
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference()
        } else {
            ExprArcInfo::value()
        }
    }

    fn classify_for(
        &self,
        _binding: LocalId,
        _iter: &TExpr,
        _body: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo {
        // For loop result (list from yield, or void)
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference()
        } else {
            ExprArcInfo::value()
        }
    }

    // =========================================================================
    // Assignment
    // =========================================================================

    fn classify_assign(&self, _target: LocalId, _value: &TExpr, _ty: &Type) -> ExprArcInfo {
        // Assignment returns void (the assigned value needs handling separately)
        ExprArcInfo::value().with_children(vec![ChildVisit::owned("value")])
    }

    // =========================================================================
    // Range
    // =========================================================================

    fn classify_range(&self, _start: &TExpr, _end: &TExpr, _ty: &Type) -> ExprArcInfo {
        // Range is a value type (two integers)
        ExprArcInfo::value().with_children(vec![
            ChildVisit::borrowed("start"),
            ChildVisit::borrowed("end"),
        ])
    }

    // =========================================================================
    // Patterns
    // =========================================================================

    fn classify_pattern_expr(&self, pattern: &TPattern, ty: &Type) -> ExprArcInfo {
        // Delegate to pattern classifier
        let pattern_info = classify_pattern(self, pattern);

        if pattern_info.result_needs_arc || self.type_needs_arc(ty) {
            ExprArcInfo::reference()
        } else {
            ExprArcInfo::value()
        }
    }

    // =========================================================================
    // Result/Option Constructors
    // =========================================================================

    fn classify_ok(&self, _value: &TExpr, ty: &Type) -> ExprArcInfo {
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(vec![ChildVisit::owned("ok value")])
        } else {
            ExprArcInfo::value().with_children(vec![ChildVisit::owned("ok value")])
        }
    }

    fn classify_err(&self, _value: &TExpr, ty: &Type) -> ExprArcInfo {
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(vec![ChildVisit::owned("err value")])
        } else {
            ExprArcInfo::value().with_children(vec![ChildVisit::owned("err value")])
        }
    }

    fn classify_some(&self, _value: &TExpr, ty: &Type) -> ExprArcInfo {
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(vec![ChildVisit::owned("some value")])
        } else {
            ExprArcInfo::value().with_children(vec![ChildVisit::owned("some value")])
        }
    }

    fn classify_none(&self, _ty: &Type) -> ExprArcInfo {
        // None is always a value (null pointer or tag)
        ExprArcInfo::value()
    }

    fn classify_coalesce(&self, _value: &TExpr, _default: &TExpr, ty: &Type) -> ExprArcInfo {
        let children = vec![
            ChildVisit::borrowed("value"),
            ChildVisit::owned("default"),
        ];

        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(children)
        } else {
            ExprArcInfo::value().with_children(children)
        }
    }

    fn classify_unwrap(&self, _value: &TExpr, ty: &Type) -> ExprArcInfo {
        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(vec![ChildVisit::borrowed("wrapped")])
        } else {
            ExprArcInfo::value().with_children(vec![ChildVisit::borrowed("wrapped")])
        }
    }

    // =========================================================================
    // Capability
    // =========================================================================

    fn classify_with(
        &self,
        _capability: &str,
        _implementation: &TExpr,
        _body: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo {
        let children = vec![
            ChildVisit::owned("implementation"),
            ChildVisit::owned("body"),
        ];

        if self.type_needs_arc(ty) {
            ExprArcInfo::reference().with_children(children)
        } else {
            ExprArcInfo::value().with_children(children)
        }
    }
}

// =============================================================================
// ArcTypeClassifier Implementation (18 methods for 18 variants)
// =============================================================================

impl ArcTypeClassifier for ExhaustiveArcAnalyzer {
    // =========================================================================
    // Primitives
    // =========================================================================

    fn classify_int(&self) -> TypeArcInfo {
        TypeArcInfo::value(8) // 64-bit int
    }

    fn classify_float(&self) -> TypeArcInfo {
        TypeArcInfo::value(8) // 64-bit float
    }

    fn classify_bool(&self) -> TypeArcInfo {
        TypeArcInfo::value(1)
    }

    fn classify_str(&self) -> TypeArcInfo {
        // Strings are reference types (heap allocated)
        TypeArcInfo::reference(24) // Typical string struct size
    }

    fn classify_void(&self) -> TypeArcInfo {
        TypeArcInfo::value(0)
    }

    // =========================================================================
    // Collections
    // =========================================================================

    fn classify_list(&self, _elem_ty: &Type) -> TypeArcInfo {
        // Lists are always reference types
        TypeArcInfo::reference(24)
    }

    fn classify_map(&self, _key_ty: &Type, _val_ty: &Type) -> TypeArcInfo {
        // Maps are always reference types
        TypeArcInfo::reference(32)
    }

    fn classify_tuple(&self, elem_tys: &[Type]) -> TypeArcInfo {
        // Calculate tuple size
        let size: usize = elem_tys.iter().map(|t| self.size_calc.size_of(t)).sum();

        // Check if any element needs ARC
        let needs_arc = elem_tys.iter().any(|t| self.type_needs_arc(t));

        if needs_arc {
            TypeArcInfo::hybrid(size)
        } else if size <= 32 {
            TypeArcInfo::value(size)
        } else {
            TypeArcInfo::reference(size)
        }
    }

    // =========================================================================
    // User-Defined
    // =========================================================================

    fn classify_struct_type(&self, _name: &str, fields: &[(String, Type)]) -> TypeArcInfo {
        let size: usize = fields.iter().map(|(_, t)| self.size_calc.size_of(t)).sum();
        let needs_arc = fields.iter().any(|(_, t)| self.type_needs_arc(t));

        if needs_arc {
            if size <= 32 {
                TypeArcInfo::hybrid(size)
            } else {
                TypeArcInfo::reference(size)
            }
        } else if size <= 32 {
            TypeArcInfo::value(size)
        } else {
            TypeArcInfo::reference(size)
        }
    }

    fn classify_enum_type(
        &self,
        _name: &str,
        variants: &[(String, Vec<(String, Type)>)],
    ) -> TypeArcInfo {
        // Enum size is max variant size + tag
        let max_size: usize = variants
            .iter()
            .map(|(_, fields)| {
                fields
                    .iter()
                    .map(|(_, t)| self.size_calc.size_of(t))
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);

        let size = max_size + 8; // Add tag size

        let needs_arc = variants
            .iter()
            .any(|(_, fields)| fields.iter().any(|(_, t)| self.type_needs_arc(t)));

        if needs_arc {
            if size <= 32 {
                TypeArcInfo::hybrid(size)
            } else {
                TypeArcInfo::reference(size)
            }
        } else if size <= 32 {
            TypeArcInfo::value(size)
        } else {
            TypeArcInfo::reference(size)
        }
    }

    fn classify_named(&self, _name: &str) -> TypeArcInfo {
        // Named types are conservatively treated as reference types
        // because we don't have the definition here
        TypeArcInfo::reference(24)
    }

    // =========================================================================
    // Function
    // =========================================================================

    fn classify_function(&self, _params: &[Type], _ret: &Type) -> TypeArcInfo {
        // Function pointers/closures are reference types
        TypeArcInfo::reference(16)
    }

    // =========================================================================
    // Result/Option
    // =========================================================================

    fn classify_result(&self, ok_ty: &Type, err_ty: &Type) -> TypeArcInfo {
        let ok_size = self.size_calc.size_of(ok_ty);
        let err_size = self.size_calc.size_of(err_ty);
        let size = ok_size.max(err_size) + 8; // Union + tag

        let needs_arc = self.type_needs_arc(ok_ty) || self.type_needs_arc(err_ty);

        if needs_arc {
            if size <= 32 {
                TypeArcInfo::hybrid(size)
            } else {
                TypeArcInfo::reference(size)
            }
        } else if size <= 32 {
            TypeArcInfo::value(size)
        } else {
            TypeArcInfo::reference(size)
        }
    }

    fn classify_option(&self, inner_ty: &Type) -> TypeArcInfo {
        let inner_size = self.size_calc.size_of(inner_ty);
        let size = inner_size + 8; // Value + tag

        let needs_arc = self.type_needs_arc(inner_ty);

        if needs_arc {
            if size <= 32 {
                TypeArcInfo::hybrid(size)
            } else {
                TypeArcInfo::reference(size)
            }
        } else if size <= 32 {
            TypeArcInfo::value(size)
        } else {
            TypeArcInfo::reference(size)
        }
    }

    // =========================================================================
    // Other
    // =========================================================================

    fn classify_record(&self, fields: &[(String, Type)]) -> TypeArcInfo {
        // Same as struct
        self.classify_struct_type("", fields)
    }

    fn classify_range(&self) -> TypeArcInfo {
        // Range is two integers
        TypeArcInfo::value(16)
    }

    fn classify_any(&self) -> TypeArcInfo {
        // Any is a type-erased pointer
        TypeArcInfo::reference(16)
    }

    fn classify_dyn_trait(&self, _trait_name: &str) -> TypeArcInfo {
        // Trait objects are reference types (vtable pointer + data pointer)
        TypeArcInfo::reference(16)
    }
}

// =============================================================================
// ArcPatternClassifier Implementation (13 methods for 13 variants)
// =============================================================================

impl ArcPatternClassifier for ExhaustiveArcAnalyzer {
    fn classify_fold(
        &self,
        _collection: &TExpr,
        _elem_ty: &Type,
        _init: &TExpr,
        _op: &TExpr,
        result_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true, // Creates accumulator
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: vec![
                ChildVisit::borrowed("collection"),
                ChildVisit::owned("init"),
                ChildVisit::borrowed("op"),
            ],
        }
    }

    fn classify_map(
        &self,
        _collection: &TExpr,
        _elem_ty: &Type,
        _transform: &TExpr,
        result_elem_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true, // Creates result list
            result_needs_arc: true,    // Map always returns a list (reference type)
            children_to_visit: vec![
                ChildVisit::borrowed("collection"),
                ChildVisit::borrowed("transform"),
            ],
        }
    }

    fn classify_filter(
        &self,
        _collection: &TExpr,
        _elem_ty: &Type,
        _predicate: &TExpr,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true, // Creates result list
            result_needs_arc: true,    // Filter always returns a list
            children_to_visit: vec![
                ChildVisit::borrowed("collection"),
                ChildVisit::borrowed("predicate"),
            ],
        }
    }

    fn classify_collect(
        &self,
        _range: &TExpr,
        _transform: &TExpr,
        _result_elem_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true,
            result_needs_arc: true, // Collect returns a list
            children_to_visit: vec![
                ChildVisit::borrowed("range"),
                ChildVisit::borrowed("transform"),
            ],
        }
    }

    fn classify_recurse(
        &self,
        _cond: &TExpr,
        _base: &TExpr,
        _step: &TExpr,
        result_ty: &Type,
        memo: bool,
        _parallel_threshold: i64,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: memo, // Memoization creates temporaries
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: vec![
                ChildVisit::borrowed("cond"),
                ChildVisit::owned("base"),
                ChildVisit::owned("step"),
            ],
        }
    }

    fn classify_iterate(
        &self,
        _over: &TExpr,
        _elem_ty: &Type,
        _direction: IterDirection,
        _into: &TExpr,
        _with: &TExpr,
        result_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true,
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: vec![
                ChildVisit::borrowed("over"),
                ChildVisit::owned("into"),
                ChildVisit::borrowed("with"),
            ],
        }
    }

    fn classify_transform(
        &self,
        _input: &TExpr,
        _steps: &[TExpr],
        result_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true, // Pipeline creates intermediates
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: vec![ChildVisit::owned("input")],
        }
    }

    fn classify_count(
        &self,
        _collection: &TExpr,
        _elem_ty: &Type,
        _predicate: &TExpr,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: false,
            result_needs_arc: false, // Count returns int
            children_to_visit: vec![
                ChildVisit::borrowed("collection"),
                ChildVisit::borrowed("predicate"),
            ],
        }
    }

    fn classify_parallel(
        &self,
        _branches: &[(String, TExpr, Type)],
        _timeout: Option<&TExpr>,
        _on_error: OnError,
        result_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true, // Parallel creates tasks
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: Vec::new(), // Branches handled separately
        }
    }

    fn classify_find(
        &self,
        _collection: &TExpr,
        _elem_ty: &Type,
        _predicate: &TExpr,
        _default: Option<&TExpr>,
        result_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: false,
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: vec![
                ChildVisit::borrowed("collection"),
                ChildVisit::borrowed("predicate"),
            ],
        }
    }

    fn classify_try(
        &self,
        _body: &TExpr,
        _catch: Option<&TExpr>,
        result_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: false,
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: vec![ChildVisit::owned("body")],
        }
    }

    fn classify_retry(
        &self,
        _operation: &TExpr,
        _max_attempts: &TExpr,
        _backoff: RetryBackoff,
        _delay_ms: Option<&TExpr>,
        result_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true, // Retry may create multiple temporaries
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: vec![ChildVisit::owned("operation")],
        }
    }

    fn classify_validate(
        &self,
        _rules: &[(TExpr, TExpr)],
        _then_value: &TExpr,
        result_ty: &Type,
    ) -> PatternArcInfo {
        PatternArcInfo {
            creates_temporaries: true, // May accumulate errors
            result_needs_arc: self.type_needs_arc(result_ty),
            children_to_visit: vec![ChildVisit::owned("then_value")],
        }
    }
}

// =============================================================================
// ArcMatchPatternClassifier Implementation (5 methods for 5 variants)
// =============================================================================

impl ArcMatchPatternClassifier for ExhaustiveArcAnalyzer {
    fn classify_wildcard(&self) -> MatchPatternArcInfo {
        MatchPatternArcInfo::none()
    }

    fn classify_literal(&self, _expr: &TExpr) -> MatchPatternArcInfo {
        // Literals don't bind anything
        MatchPatternArcInfo::none()
    }

    fn classify_binding(&self, local: LocalId, ty: &Type) -> MatchPatternArcInfo {
        MatchPatternArcInfo::with_bindings(vec![local], self.type_needs_arc(ty))
    }

    fn classify_variant(
        &self,
        _name: &str,
        bindings: &[(String, LocalId, Type)],
    ) -> MatchPatternArcInfo {
        let locals: Vec<_> = bindings.iter().map(|(_, id, _)| *id).collect();
        let binds_refs = bindings.iter().any(|(_, _, ty)| self.type_needs_arc(ty));
        MatchPatternArcInfo::with_bindings(locals, binds_refs)
    }

    fn classify_condition(&self, _expr: &TExpr) -> MatchPatternArcInfo {
        // Conditions don't bind anything
        MatchPatternArcInfo::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{LocalTable, TExpr, TFunction};

    fn make_test_function(body: TExpr) -> TFunction {
        TFunction {
            name: "test".to_string(),
            public: false,
            params: vec![],
            return_type: body.ty.clone(),
            locals: LocalTable::new(),
            body,
            span: 0..1,
        }
    }

    #[test]
    fn test_analyzer_creation() {
        let analyzer = ExhaustiveArcAnalyzer::new();
        assert!(!analyzer.type_needs_arc(&Type::Int));
        assert!(analyzer.type_needs_arc(&Type::Str));
    }

    #[test]
    fn test_analyze_simple_function() {
        let analyzer = ExhaustiveArcAnalyzer::new();
        let func = make_test_function(TExpr::new(TExprKind::Int(42), Type::Int, 0..1));

        let result = analyzer.analyze_function(&func);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.name, "test");
        assert_eq!(info.ref_type_locals, 0);
    }

    #[test]
    fn test_classify_int_literal() {
        let analyzer = ExhaustiveArcAnalyzer::new();
        let info = analyzer.classify_int_literal(42, &Type::Int);
        assert!(!info.needs_retain);
        assert!(!info.needs_release);
    }

    #[test]
    fn test_classify_string_literal() {
        let analyzer = ExhaustiveArcAnalyzer::new();
        let info = analyzer.classify_string_literal("hello", &Type::Str);
        assert!(info.needs_retain);
        assert!(info.needs_release);
    }

    #[test]
    fn test_classify_list_type() {
        let analyzer = ExhaustiveArcAnalyzer::new();
        let info = ArcTypeClassifier::classify_list(&analyzer, &Type::Int);
        assert!(info.needs_arc);
        assert_eq!(info.storage_class, StorageClass::Reference);
    }

    #[test]
    fn test_classify_int_type() {
        let analyzer = ExhaustiveArcAnalyzer::new();
        let info = analyzer.classify_int();
        assert!(!info.needs_arc);
        assert_eq!(info.storage_class, StorageClass::Value);
    }

    #[test]
    fn test_analyze_module() {
        let mut module = TModule::new("test".to_string());
        module.functions.push(make_test_function(TExpr::new(
            TExprKind::Int(42),
            Type::Int,
            0..1,
        )));

        let analyzer = ExhaustiveArcAnalyzer::new();
        let result = analyzer.analyze_module(&module);

        assert!(result.is_ok());
        let info = result.unwrap();
        assert!(info.functions.contains_key("test"));
    }
}

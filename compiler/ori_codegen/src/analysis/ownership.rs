//! Ownership analysis for ARC elision.
//!
//! Analyzes each expression to determine if ARC operations (retain/release)
//! can be skipped. This is a key optimization for performance.
//!
//! # Elision Rules
//!
//! 1. **Primitives** - Never need ARC (int, float, bool, etc.)
//! 2. **SSO strings** - Strings ≤23 bytes don't use the heap
//! 3. **Last use** - Move instead of copy at last use of a binding
//! 4. **Return values** - Caller takes ownership
//! 5. **Temporaries** - Consumed immediately, no intermediate retain

use rustc_hash::FxHashSet;
use ori_ir::{ast::{ExprKind, FunctionSeq, SeqBinding}, ExprArena, ExprId, Name, TypeId};
use ori_types::{TypeData, TypeInterner};

/// Ownership status of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    /// Value is owned, needs release on scope exit.
    Owned,
    /// Reference only, no ARC ops needed.
    Borrowed,
    /// Transferred to callee, skip release.
    Moved,
    /// Stored or returned, needs retain.
    Escapes,
}

/// Results of ownership analysis.
#[derive(Clone, Debug, Default)]
pub struct OwnershipInfo {
    /// Expressions where ARC operations can be skipped.
    pub elide_arc: FxHashSet<ExprId>,
    /// Bindings that need release at scope exit.
    pub needs_release: FxHashSet<Name>,
}

/// Ownership analyzer.
///
/// Performs a single pass over the AST to determine which expressions
/// need ARC operations and which can be elided.
pub struct OwnershipAnalysis<'a> {
    arena: &'a ExprArena,
    type_interner: &'a TypeInterner,
    expr_types: &'a [TypeId],
    info: OwnershipInfo,
    /// Track last use of each binding.
    binding_uses: FxHashSet<Name>,
}

impl<'a> OwnershipAnalysis<'a> {
    /// Create a new ownership analyzer.
    pub fn new(
        arena: &'a ExprArena,
        type_interner: &'a TypeInterner,
        expr_types: &'a [TypeId],
    ) -> Self {
        Self {
            arena,
            type_interner,
            expr_types,
            info: OwnershipInfo::default(),
            binding_uses: FxHashSet::default(),
        }
    }

    /// Run ownership analysis on an expression.
    pub fn analyze(mut self, root: ExprId) -> OwnershipInfo {
        self.visit_expr(root, Ownership::Owned);
        self.info
    }

    /// Check if a type needs ARC (is heap-allocated).
    fn needs_arc(&self, type_id: TypeId) -> bool {
        // STR is special - it might need ARC (SSO handles small strings inline)
        if type_id == TypeId::STR {
            return true;
        }

        // Other primitives never need ARC
        if type_id.is_primitive() {
            return false;
        }

        // INFER and SELF_TYPE are special markers, not real types
        if type_id.is_infer() || type_id.is_self_type() {
            return false;
        }

        let type_data = self.type_interner.lookup(type_id);
        match type_data {
            // Primitives don't need ARC
            TypeData::Int
            | TypeData::Float
            | TypeData::Bool
            | TypeData::Char
            | TypeData::Byte
            | TypeData::Unit
            | TypeData::Never
            | TypeData::Duration
            | TypeData::Size
            | TypeData::Error => false,

            // Str might need ARC (depends on SSO at runtime)
            // We conservatively mark as needing ARC, but SSO handles it at runtime
            // Container, function, user, type variable, and projection types
            // conservatively need ARC
            TypeData::Str
            | TypeData::List(_)
            | TypeData::Map { .. }
            | TypeData::Set(_)
            | TypeData::Channel(_)
            | TypeData::Function { .. }
            | TypeData::Named(_)
            | TypeData::Applied { .. }
            | TypeData::Var(_)
            | TypeData::Projection { .. } => true,

            // Option and Result of primitives are unboxed (no ARC)
            TypeData::Option(inner) | TypeData::Range(inner) => self.needs_arc(inner),

            TypeData::Result { ok, err } => self.needs_arc(ok) || self.needs_arc(err),

            // Tuples: need ARC if any element needs ARC
            TypeData::Tuple(elems) => elems.iter().any(|&e| self.needs_arc(e)),
        }
    }

    /// Visit an expression and determine ownership requirements.
    fn visit_expr(&mut self, id: ExprId, context: Ownership) {
        let type_id = self.expr_types.get(id.index()).copied().unwrap_or(TypeId::INFER);

        // Fast path: primitives never need ARC
        if !self.needs_arc(type_id) {
            self.info.elide_arc.insert(id);
            self.visit_children(id);
            return;
        }

        let expr = self.arena.get_expr(id);
        match &expr.kind {
            // Identifiers: last use can be moved
            ExprKind::Ident(name) => {
                // If this is potentially the last use, we can elide
                if context == Ownership::Moved || context == Ownership::Owned {
                    self.info.elide_arc.insert(id);
                }
                self.binding_uses.insert(*name);
            }

            // Literals don't need ARC tracking
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Char(_)
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. }
            | ExprKind::Unit
            | ExprKind::HashLength
            | ExprKind::String(_)
            | ExprKind::None => {
                // Primitives, SSO strings, and None need no ARC
                self.info.elide_arc.insert(id);
            }

            // Some: depends on inner type
            ExprKind::Some(inner) => {
                let inner_type = self.expr_types.get(inner.index()).copied().unwrap_or(TypeId::INFER);
                if !self.needs_arc(inner_type) {
                    // Unboxed Option - no ARC needed
                    self.info.elide_arc.insert(id);
                }
                self.visit_expr(*inner, Ownership::Moved);
            }

            // Ok/Err: depends on inner type
            ExprKind::Ok(inner) | ExprKind::Err(inner) => {
                if let Some(inner_id) = inner {
                    let inner_type = self.expr_types.get(inner_id.index()).copied().unwrap_or(TypeId::INFER);
                    if !self.needs_arc(inner_type) {
                        self.info.elide_arc.insert(id);
                    }
                    self.visit_expr(*inner_id, Ownership::Moved);
                } else {
                    self.info.elide_arc.insert(id);
                }
            }

            // Let bindings: the binding may need release
            ExprKind::Let { pattern, init, .. } => {
                self.visit_expr(*init, Ownership::Moved);
                // Track if binding needs release
                if let ori_ir::ast::BindingPattern::Name(name) = pattern {
                    if self.needs_arc(type_id) {
                        self.info.needs_release.insert(*name);
                    }
                }
            }

            // Function calls: arguments are moved
            ExprKind::Call { func, args } => {
                self.visit_expr(*func, Ownership::Borrowed);
                for &arg in self.arena.get_expr_list(*args) {
                    self.visit_expr(arg, Ownership::Moved);
                }
            }

            ExprKind::CallNamed { func, args } => {
                self.visit_expr(*func, Ownership::Borrowed);
                for arg in self.arena.get_call_args(*args) {
                    self.visit_expr(arg.value, Ownership::Moved);
                }
            }

            // Method calls: receiver is borrowed, args are moved
            ExprKind::MethodCall { receiver, args, .. } => {
                self.visit_expr(*receiver, Ownership::Borrowed);
                for &arg in self.arena.get_expr_list(*args) {
                    self.visit_expr(arg, Ownership::Moved);
                }
            }

            ExprKind::MethodCallNamed { receiver, args, .. } => {
                self.visit_expr(*receiver, Ownership::Borrowed);
                for arg in self.arena.get_call_args(*args) {
                    self.visit_expr(arg.value, Ownership::Moved);
                }
            }

            // Binary/unary ops: operands are borrowed (primitives)
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(*left, Ownership::Borrowed);
                self.visit_expr(*right, Ownership::Borrowed);
            }

            ExprKind::Unary { operand, .. } => {
                self.visit_expr(*operand, Ownership::Borrowed);
            }

            // If expressions: both branches have same ownership as result
            ExprKind::If { cond, then_branch, else_branch } => {
                self.visit_expr(*cond, Ownership::Borrowed);
                self.visit_expr(*then_branch, context);
                if let Some(else_id) = else_branch {
                    self.visit_expr(*else_id, context);
                }
            }

            // Match: scrutinee is borrowed, arms have result ownership
            ExprKind::Match { scrutinee, arms } => {
                self.visit_expr(*scrutinee, Ownership::Borrowed);
                for arm in self.arena.get_arms(*arms) {
                    self.visit_expr(arm.body, context);
                }
            }

            // For: iter is borrowed, body depends on yield
            ExprKind::For { iter, body, guard, .. } => {
                self.visit_expr(*iter, Ownership::Borrowed);
                if let Some(g) = guard {
                    self.visit_expr(*g, Ownership::Borrowed);
                }
                self.visit_expr(*body, Ownership::Owned);
            }

            // Loop: body is owned (used for side effects)
            ExprKind::Loop { body } => {
                self.visit_expr(*body, Ownership::Owned);
            }

            // Block: statements owned, result has block ownership
            ExprKind::Block { stmts, result } => {
                for stmt in self.arena.get_stmt_range(*stmts) {
                    let expr_id = match &stmt.kind {
                        ori_ir::StmtKind::Expr(id) => Some(*id),
                        ori_ir::StmtKind::Let { init, .. } => Some(*init),
                    };
                    if let Some(id) = expr_id {
                        self.visit_expr(id, Ownership::Owned);
                    }
                }
                if let Some(res) = result {
                    self.visit_expr(*res, context);
                }
            }

            // Lambda: body has return ownership
            ExprKind::Lambda { body, .. } => {
                self.visit_expr(*body, Ownership::Escapes);
            }

            // Lists/tuples: elements are moved into the container
            ExprKind::List(elems) | ExprKind::Tuple(elems) => {
                for &elem in self.arena.get_expr_list(*elems) {
                    self.visit_expr(elem, Ownership::Moved);
                }
            }

            // Maps: keys and values are moved
            ExprKind::Map(entries) => {
                for entry in self.arena.get_map_entries(*entries) {
                    self.visit_expr(entry.key, Ownership::Moved);
                    self.visit_expr(entry.value, Ownership::Moved);
                }
            }

            // Structs: fields are moved
            ExprKind::Struct { fields, .. } => {
                for field in self.arena.get_field_inits(*fields) {
                    if let Some(value) = field.value {
                        self.visit_expr(value, Ownership::Moved);
                    }
                }
            }

            // Field access: receiver is borrowed
            ExprKind::Field { receiver, .. } | ExprKind::Index { receiver, .. } => {
                self.visit_expr(*receiver, Ownership::Borrowed);
            }

            // Ranges: start/end are borrowed (primitives)
            ExprKind::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.visit_expr(*s, Ownership::Borrowed);
                }
                if let Some(e) = end {
                    self.visit_expr(*e, Ownership::Borrowed);
                }
            }

            // Return/break: value escapes
            ExprKind::Return(val) | ExprKind::Break(val) => {
                if let Some(v) = val {
                    self.visit_expr(*v, Ownership::Escapes);
                }
            }

            // Assignment: value is moved
            ExprKind::Assign { target, value } => {
                self.visit_expr(*target, Ownership::Borrowed);
                self.visit_expr(*value, Ownership::Moved);
            }

            // Try: inner value propagates
            ExprKind::Try(inner) | ExprKind::Await(inner) => {
                self.visit_expr(*inner, context);
            }

            // With capability: body is owned
            ExprKind::WithCapability { provider, body, .. } => {
                self.visit_expr(*provider, Ownership::Moved);
                self.visit_expr(*body, context);
            }

            // FunctionSeq/FunctionExp: visit all contained expressions
            ExprKind::FunctionSeq(seq) => {
                let bindings_range = match seq {
                    FunctionSeq::Run { bindings, .. }
                    | FunctionSeq::Try { bindings, .. } => Some(*bindings),
                    FunctionSeq::Match { .. }
                    | FunctionSeq::ForPattern { .. } => None,
                };
                if let Some(range) = bindings_range {
                    for binding in self.arena.get_seq_bindings(range) {
                        let value = match binding {
                            SeqBinding::Let { value, .. } => *value,
                            SeqBinding::Stmt { expr, .. } => *expr,
                        };
                        self.visit_expr(value, Ownership::Owned);
                    }
                }
            }

            ExprKind::FunctionExp(exp) => {
                for named in self.arena.get_named_exprs(exp.props) {
                    self.visit_expr(named.value, Ownership::Moved);
                }
            }

            // Config/FunctionRef/SelfRef don't need ARC analysis,
            // Continue has no value, Error is a placeholder
            ExprKind::Config(_)
            | ExprKind::FunctionRef(_)
            | ExprKind::SelfRef
            | ExprKind::Continue
            | ExprKind::Error => {}
        }
    }

    /// Visit children of an expression (for primitives that don't need detailed analysis).
    fn visit_children(&mut self, id: ExprId) {
        let expr = self.arena.get_expr(id);
        match &expr.kind {
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(*left, Ownership::Borrowed);
                self.visit_expr(*right, Ownership::Borrowed);
            }
            ExprKind::Unary { operand, .. } => {
                self.visit_expr(*operand, Ownership::Borrowed);
            }
            ExprKind::If { cond, then_branch, else_branch } => {
                self.visit_expr(*cond, Ownership::Borrowed);
                self.visit_expr(*then_branch, Ownership::Owned);
                if let Some(e) = else_branch {
                    self.visit_expr(*e, Ownership::Owned);
                }
            }
            ExprKind::Let { init, .. } => {
                self.visit_expr(*init, Ownership::Moved);
            }
            ExprKind::Call { func, args } => {
                self.visit_expr(*func, Ownership::Borrowed);
                for &arg in self.arena.get_expr_list(*args) {
                    self.visit_expr(arg, Ownership::Moved);
                }
            }
            ExprKind::CallNamed { func, args } => {
                self.visit_expr(*func, Ownership::Borrowed);
                for arg in self.arena.get_call_args(*args) {
                    self.visit_expr(arg.value, Ownership::Moved);
                }
            }
            _ => {
                // Other expressions handled in visit_expr
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_elision() {
        let type_interner = TypeInterner::new();

        // Primitives don't need ARC
        let analyzer = OwnershipAnalysis {
            arena: &ExprArena::new(),
            type_interner: &type_interner,
            expr_types: &[],
            info: OwnershipInfo::default(),
            binding_uses: FxHashSet::default(),
        };

        assert!(!analyzer.needs_arc(TypeId::INT));
        assert!(!analyzer.needs_arc(TypeId::FLOAT));
        assert!(!analyzer.needs_arc(TypeId::BOOL));
        assert!(!analyzer.needs_arc(TypeId::VOID));
    }

    #[test]
    fn test_option_unboxed() {
        let type_interner = TypeInterner::new();

        // Option<int> doesn't need ARC
        let option_int = type_interner.option(TypeId::INT);

        let analyzer = OwnershipAnalysis {
            arena: &ExprArena::new(),
            type_interner: &type_interner,
            expr_types: &[],
            info: OwnershipInfo::default(),
            binding_uses: FxHashSet::default(),
        };

        assert!(!analyzer.needs_arc(option_int));

        // Option<[int]> needs ARC (list inside)
        let list_int = type_interner.list(TypeId::INT);
        let option_list = type_interner.option(list_int);
        assert!(analyzer.needs_arc(option_list));
    }

    #[test]
    fn test_result_unboxed() {
        let type_interner = TypeInterner::new();

        // Result<int, int> doesn't need ARC
        let result_int = type_interner.result(TypeId::INT, TypeId::INT);

        let analyzer = OwnershipAnalysis {
            arena: &ExprArena::new(),
            type_interner: &type_interner,
            expr_types: &[],
            info: OwnershipInfo::default(),
            binding_uses: FxHashSet::default(),
        };

        assert!(!analyzer.needs_arc(result_int));

        // Result<int, str> needs ARC (str can be heap-allocated)
        let result_str = type_interner.result(TypeId::INT, TypeId::STR);
        assert!(analyzer.needs_arc(result_str));
    }
}

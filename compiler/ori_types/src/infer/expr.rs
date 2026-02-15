//! Expression type inference.
//!
//! This module provides expression-level type inference using the
//! `InferEngine` infrastructure. It dispatches on `ExprKind` to
//! specialized inference functions.
//!
//! # Architecture
//!
//! Expression inference follows Hindley-Milner with bidirectional enhancements:
//!
//! - **Synthesis (infer)**: Bottom-up type derivation from expression structure
//! - **Checking (check)**: Top-down verification against expected type
//!
//! The dispatch is structured to match `ori_ir::ExprKind` variants,
//! with each category delegating to specialized modules:
//!
//! - Literals → direct primitive type
//! - Identifiers → environment lookup + instantiation
//! - Operators → operator inference (binary, unary)
//! - Calls → function/method call inference
//! - Control flow → if/match/loop inference
//! - Lambdas → lambda inference with scope management
//! - Collections → list/map/tuple inference
//!
//! # Usage
//!
//! ```ignore
//! use ori_types::infer::{InferEngine, infer_expr};
//!
//! let mut pool = Pool::new();
//! let mut engine = InferEngine::new(&mut pool);
//!
//! // Infer type of expression
//! let ty = infer_expr(&mut engine, &arena, expr_id);
//! ```

use ori_ir::{
    BinaryOp, ExprArena, ExprId, ExprKind, Name, ParsedType, ParsedTypeRange, Span, TypeId, UnaryOp,
};
use ori_stack::ensure_sufficient_stack;

use rustc_hash::{FxHashMap, FxHashSet};

use super::InferEngine;
use crate::{
    ContextKind, Expected, ExpectedOrigin, Idx, PatternKey, PatternResolution, Pool, SequenceKind,
    Tag, TypeCheckError, TypeKind, TypeRegistry, VariantFields,
};

/// Infer the type of an expression.
///
/// This is the main entry point for expression type inference.
/// It dispatches to specialized handlers based on expression kind.
#[tracing::instrument(level = "trace", skip(engine, arena))]
pub fn infer_expr(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx {
    ensure_sufficient_stack(|| infer_expr_inner(engine, arena, expr_id))
}

/// Inner implementation of expression inference, dispatching on `ExprKind`.
fn infer_expr_inner(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx {
    let expr = arena.get_expr(expr_id);
    let span = expr.span;

    let ty = match &expr.kind {
        // === Literals ===
        ExprKind::Int(_) | ExprKind::HashLength => Idx::INT,
        ExprKind::Float(_) => Idx::FLOAT,
        ExprKind::Bool(_) => Idx::BOOL,
        ExprKind::String(_) | ExprKind::TemplateFull(_) => Idx::STR,
        ExprKind::Char(_) => Idx::CHAR,
        ExprKind::Duration { .. } => Idx::DURATION,
        ExprKind::Size { .. } => Idx::SIZE,
        ExprKind::Unit => Idx::UNIT,

        // === Identifiers ===
        ExprKind::Ident(name) => infer_ident(engine, *name, span),
        ExprKind::FunctionRef(name) => infer_function_ref(engine, *name, span),
        ExprKind::SelfRef => infer_self_ref(engine, span),
        ExprKind::Const(name) => infer_const(engine, *name, span),

        // === Operators ===
        ExprKind::Binary { op, left, right } => {
            infer_binary(engine, arena, *op, *left, *right, span)
        }
        ExprKind::Unary { op, operand } => infer_unary(engine, arena, *op, *operand, span),

        // === Calls ===
        ExprKind::Call { func, args } => infer_call(engine, arena, *func, *args, span),
        ExprKind::CallNamed { func, args } => infer_call_named(engine, arena, *func, *args, span),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => infer_method_call(engine, arena, *receiver, *method, *args, span),
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => infer_method_call_named(engine, arena, *receiver, *method, *args, span),

        // === Field/Index Access ===
        ExprKind::Field { receiver, field } => infer_field(engine, arena, *receiver, *field, span),
        ExprKind::Index { receiver, index } => infer_index(engine, arena, *receiver, *index, span),

        // === Control Flow ===
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => infer_if(engine, arena, *cond, *then_branch, *else_branch, span),
        ExprKind::Match { scrutinee, arms } => infer_match(engine, arena, *scrutinee, *arms, span),
        ExprKind::For {
            binding,
            iter,
            guard,
            body,
            is_yield,
            ..
        } => infer_for(
            engine, arena, *binding, *iter, *guard, *body, *is_yield, span,
        ),
        ExprKind::Loop { body, .. } => infer_loop(engine, arena, *body, span),

        // === Blocks and Bindings ===
        ExprKind::Block { stmts, result } => infer_block(engine, arena, *stmts, *result, span),
        ExprKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => {
            let pat = arena.get_binding_pattern(*pattern);
            let ty_ref = if ty.is_valid() {
                Some(arena.get_parsed_type(*ty))
            } else {
                None
            };
            infer_let(engine, arena, pat, ty_ref, *init, *mutable, span)
        }

        // === Lambdas ===
        ExprKind::Lambda {
            params,
            ret_ty,
            body,
        } => {
            let ret_ty_ref = if ret_ty.is_valid() {
                Some(arena.get_parsed_type(*ret_ty))
            } else {
                None
            };
            infer_lambda(engine, arena, *params, ret_ty_ref, *body, span)
        }

        // === Collections ===
        ExprKind::List(elements) => infer_list(engine, arena, *elements, span),
        ExprKind::ListWithSpread(elements) => infer_list_spread(engine, arena, *elements, span),
        ExprKind::Tuple(elements) => infer_tuple(engine, arena, *elements, span),
        ExprKind::Map(entries) => infer_map_literal(engine, arena, *entries, span),
        ExprKind::MapWithSpread(elements) => infer_map_spread(engine, arena, *elements, span),
        ExprKind::Range {
            start,
            end,
            step,
            inclusive,
        } => infer_range(engine, arena, *start, *end, *step, *inclusive, span),

        // === Structs ===
        ExprKind::Struct { name, fields } => infer_struct(engine, arena, *name, *fields, span),
        ExprKind::StructWithSpread { name, fields } => {
            infer_struct_spread(engine, arena, *name, *fields, span)
        }

        // === Option/Result Constructors ===
        ExprKind::Ok(inner) => infer_ok(engine, arena, *inner, span),
        ExprKind::Err(inner) => infer_err(engine, arena, *inner, span),
        ExprKind::Some(inner) => infer_some(engine, arena, *inner, span),
        ExprKind::None => infer_none(engine),

        // === Control Flow Expressions ===
        ExprKind::Break { value, .. } => infer_break(engine, arena, *value, span),
        ExprKind::Continue { value, .. } => infer_continue(engine, arena, *value, span),
        ExprKind::Try(inner) => infer_try(engine, arena, *inner, span),
        ExprKind::Await(inner) => infer_await(engine, arena, *inner, span),

        // === Casts and Assignment ===
        ExprKind::Cast { expr, ty, fallible } => infer_cast(
            engine,
            arena,
            *expr,
            arena.get_parsed_type(*ty),
            *fallible,
            span,
        ),
        ExprKind::Assign { target, value } => infer_assign(engine, arena, *target, *value, span),

        // === Capabilities ===
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => infer_with_capability(engine, arena, *capability, *provider, *body, span),

        // === Pattern Expressions ===
        ExprKind::FunctionSeq(seq_id) => {
            let func_seq = arena.get_function_seq(*seq_id);
            infer_function_seq(engine, arena, func_seq, span)
        }
        ExprKind::FunctionExp(exp_id) => {
            let func_exp = arena.get_function_exp(*exp_id);
            infer_function_exp(engine, arena, func_exp)
        }

        // === Template Literals ===
        ExprKind::TemplateLiteral { parts, .. } => {
            // Infer each interpolated expression (for error reporting), result is always str
            for part in arena.get_template_parts(*parts) {
                infer_expr(engine, arena, part.expr);
            }
            Idx::STR
        }

        // === Error ===
        ExprKind::Error => Idx::ERROR,
    };

    // Store the inferred type
    engine.store_type(expr_id.raw() as usize, ty);
    ty
}

/// Check an expression against an expected type.
///
/// This is the "check" direction of bidirectional type checking.
/// It handles cases where the expected type can guide literal typing:
///
/// - Integer literals in range 0-255 are coerced to `byte` when expected type is `byte`
///
/// For all other expressions, this infers the type and then checks against expected.
#[tracing::instrument(level = "trace", skip(engine, arena, expected))]
pub fn check_expr(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    expected: &Expected,
    span: Span,
) -> Idx {
    let expr = arena.get_expr(expr_id);

    // Resolve the expected type to see what we're checking against
    let expected_ty = engine.resolve(expected.ty);
    let expected_tag = engine.pool().tag(expected_ty);

    // Special case: integer literals can coerce to byte when in range
    if let ExprKind::Int(value) = &expr.kind {
        if expected_tag == Tag::Byte {
            // Check if the literal is in the valid byte range (0-255)
            if *value >= 0 && *value <= 255 {
                // Coerce the literal to byte
                engine.store_type(expr_id.raw() as usize, Idx::BYTE);
                return Idx::BYTE;
            }
            // Out of range - infer as int and let check_type report the mismatch
        }
    }

    // Default: infer the type and check against expected
    let inferred = infer_expr(engine, arena, expr_id);
    let _ = engine.check_type(inferred, expected, span);
    inferred
}

// ============================================================================
// Identifier Inference
// ============================================================================

/// Infer the type of an identifier reference.
fn infer_ident(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    // 1. Environment lookup (functions, parameters, let bindings)
    if let Some(scheme) = engine.env().lookup(name) {
        return engine.instantiate(scheme);
    }

    // 2. Resolve name to string for constructor/builtin matching
    let name_str = engine.lookup_name(name);

    // 2a. Special case for "self" - if not in env, check for recursive self_type
    // This handles `self()` calls inside recursive patterns like `recurse`.
    if name_str == Some("self") {
        if let Some(self_ty) = engine.self_type() {
            return self_ty;
        }
    }

    if let Some(s) = name_str {
        // 3. Built-in variant constructors (Option/Result are primitive types)
        match s {
            "Some" => {
                let t = engine.pool_mut().fresh_var();
                let opt_t = engine.pool_mut().option(t);
                return engine.pool_mut().function(&[t], opt_t);
            }
            "None" => {
                let t = engine.pool_mut().fresh_var();
                return engine.pool_mut().option(t);
            }
            "Ok" => {
                let t = engine.pool_mut().fresh_var();
                let e = engine.pool_mut().fresh_var();
                let res = engine.pool_mut().result(t, e);
                return engine.pool_mut().function(&[t], res);
            }
            "Err" => {
                let t = engine.pool_mut().fresh_var();
                let e = engine.pool_mut().fresh_var();
                let res = engine.pool_mut().result(t, e);
                return engine.pool_mut().function(&[e], res);
            }
            _ => {}
        }

        // 4. Built-in conversion functions
        let conversion_target = match s {
            "int" => Some(Idx::INT),
            "float" => Some(Idx::FLOAT),
            "str" => Some(Idx::STR),
            "byte" => Some(Idx::BYTE),
            "bool" => Some(Idx::BOOL),
            "char" => Some(Idx::CHAR),
            _ => None,
        };
        if let Some(target) = conversion_target {
            let t = engine.pool_mut().fresh_var();
            return engine.pool_mut().function(&[t], target);
        }

        // 5. Type names used as expression-level receivers for associated functions
        //    e.g., Duration.from_seconds(s: 5), Size.from_bytes(b: 100)
        match s {
            "Duration" | "duration" => return Idx::DURATION,
            "Size" | "size" => return Idx::SIZE,
            "Ordering" | "ordering" => return Idx::ORDERING,
            _ => {}
        }
    }

    // 5. TypeRegistry: newtype constructors, enum variant constructors
    //    Extract data with immutable borrow, then release before pool_mut
    if let Some(ctor) = resolve_type_constructor_info(engine, name) {
        return match ctor {
            ConstructorInfo::Newtype {
                underlying,
                type_idx,
            } => engine.pool_mut().function(&[underlying], type_idx),
            ConstructorInfo::UnitVariant {
                enum_idx,
                enum_name,
                type_params,
            } => {
                if type_params.is_empty() {
                    // Non-generic enum: return bare idx
                    enum_idx
                } else {
                    // Generic enum unit variant: instantiate fresh vars
                    // e.g., `MyNone` becomes `MyOption<$fresh>`
                    let fresh_vars: Vec<Idx> = type_params
                        .iter()
                        .map(|_| engine.pool_mut().fresh_var())
                        .collect();
                    engine.pool_mut().applied(enum_name, &fresh_vars)
                }
            }
            ConstructorInfo::TupleVariant {
                field_types,
                enum_idx,
                enum_name,
                type_params,
            } => {
                if type_params.is_empty() {
                    // Non-generic enum: use field types directly
                    engine.pool_mut().function(&field_types, enum_idx)
                } else {
                    // Generic enum: instantiate fresh type variables for type parameters
                    // Create fresh vars for each type parameter
                    let fresh_vars: Vec<Idx> = type_params
                        .iter()
                        .map(|_| engine.pool_mut().fresh_var())
                        .collect();

                    // Build substitution map: type_param_name -> fresh_var
                    let subst_map: Vec<(Name, Idx)> = type_params
                        .into_iter()
                        .zip(fresh_vars.iter().copied())
                        .collect();

                    // Substitute type params in field types
                    let substituted_fields: Vec<Idx> = field_types
                        .iter()
                        .map(|&ft| substitute_type_params_with_map(engine, ft, &subst_map))
                        .collect();

                    // Build the return type: Applied(enum_name, fresh_vars) for generics
                    // This creates e.g. MyResult<$0, $1> for a generic MyResult<T, E>
                    let ret_type = engine.pool_mut().applied(enum_name, &fresh_vars);

                    engine.pool_mut().function(&substituted_fields, ret_type)
                }
            }
        };
    }

    // 7. Unknown identifier — find similar names for typo suggestions
    let similar = engine
        .env()
        .find_similar(name, 3, |n| engine.lookup_name(n));
    engine.push_error(TypeCheckError::unknown_ident(span, name, similar));
    Idx::ERROR
}

/// Constructor info extracted from `TypeRegistry` (avoids borrow conflicts).
enum ConstructorInfo {
    Newtype {
        underlying: Idx,
        type_idx: Idx,
    },
    /// Unit variant (no fields).
    /// For generic enums (e.g., `MyNone` from `MyOption<T>`), we need the type params
    /// to instantiate fresh variables so that `MyNone` becomes `MyOption<$fresh>`.
    UnitVariant {
        enum_idx: Idx,
        enum_name: Name,
        type_params: Vec<Name>,
    },
    /// Tuple variant constructor with field types, base enum idx/name, and type parameter names.
    /// For generic enums (e.g., `MyOk(value: T)` from `MyResult<T, E>`), the field types
    /// may contain `Named(param_name)` indices that need substitution with fresh variables.
    TupleVariant {
        field_types: Vec<Idx>,
        enum_idx: Idx,
        enum_name: Name,
        type_params: Vec<Name>,
    },
}

/// Look up a name in the `TypeRegistry` to find constructor info.
///
/// Returns constructor info that can be used to build the appropriate type
/// after the registry borrow is released.
fn resolve_type_constructor_info(engine: &InferEngine<'_>, name: Name) -> Option<ConstructorInfo> {
    let registry = engine.type_registry()?;

    // Check if name is a type name
    if let Some(entry) = registry.get_by_name(name) {
        return match &entry.kind {
            TypeKind::Newtype { underlying } => Some(ConstructorInfo::Newtype {
                underlying: *underlying,
                type_idx: entry.idx,
            }),
            // Struct/Enum type names used as expressions: return as unit variant
            // (enables associated function calls like Type.new(...))
            TypeKind::Struct(_) | TypeKind::Enum { .. } => Some(ConstructorInfo::UnitVariant {
                enum_idx: entry.idx,
                enum_name: entry.name,
                type_params: entry.type_params.clone(),
            }),
            TypeKind::Alias { target } => Some(ConstructorInfo::UnitVariant {
                enum_idx: *target,
                enum_name: entry.name,
                type_params: entry.type_params.clone(),
            }),
        };
    }

    // Check if name is an enum variant constructor
    let (type_entry, variant_def) = registry.lookup_variant_def(name)?;
    let enum_idx = type_entry.idx;
    let enum_name = type_entry.name;
    let type_params = type_entry.type_params.clone();

    Some(match &variant_def.fields {
        VariantFields::Unit => ConstructorInfo::UnitVariant {
            enum_idx,
            enum_name,
            type_params,
        },
        VariantFields::Tuple(types) => ConstructorInfo::TupleVariant {
            field_types: types.clone(),
            enum_idx,
            enum_name,
            type_params,
        },
        VariantFields::Record(fields) => {
            // Record variants can be constructed with positional args
            let field_types: Vec<Idx> = fields.iter().map(|f| f.ty).collect();
            ConstructorInfo::TupleVariant {
                field_types,
                enum_idx,
                enum_name,
                type_params,
            }
        }
    })
}

/// Infer the type of a function reference (@name).
fn infer_function_ref(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    // Function references are looked up the same way as identifiers
    // but may have special handling for capability tracking
    infer_ident(engine, name, span)
}

/// Infer the type of self reference.
///
/// `self` can refer to:
/// - The current function type (for recursive calls in patterns like `recurse`)
/// - The impl `Self` type (in method bodies)
fn infer_self_ref(engine: &mut InferEngine<'_>, span: Span) -> Idx {
    if let Some(self_ty) = engine.self_type() {
        return self_ty;
    }
    engine.push_error(TypeCheckError::self_outside_impl(span));
    Idx::ERROR
}

/// Infer the type of a constant reference (`$name`).
///
/// Looks up the constant's registered type from the module-level `const_types` map.
/// If not found, emits an "undefined constant" error.
fn infer_const(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    if let Some(ty) = engine.const_type(name) {
        return ty;
    }
    engine.push_error(TypeCheckError::undefined_const(name, span));
    Idx::ERROR
}

// ============================================================================
// Operator Inference
// ============================================================================

/// Infer the type of a binary operation.
fn infer_binary(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
    span: Span,
) -> Idx {
    let left_ty = infer_expr(engine, arena, left);
    let right_ty = infer_expr(engine, arena, right);
    let op_str = op.as_symbol();

    // Never propagation: if the left operand is Never (e.g. panic()), the right
    // operand is unreachable and the whole expression is Never.
    let resolved_left_top = engine.resolve(left_ty);
    if engine.pool().tag(resolved_left_top) == Tag::Never {
        return Idx::NEVER;
    }

    match op {
        // Arithmetic: same type in, same type out (with Duration/Size mixed support)
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::FloorDiv => {
            let resolved_left = engine.resolve(left_ty);
            let resolved_right = engine.resolve(right_ty);
            let left_tag = engine.pool().tag(resolved_left);
            let right_tag = engine.pool().tag(resolved_right);

            // Special case: Duration/Size * Int, Int * Duration/Size, Duration/Size / Int
            let mixed_result = match (left_tag, right_tag, op) {
                // Duration + Duration, Duration * int, Duration / int, int * Duration = Duration
                (Tag::Duration, Tag::Duration, _)
                | (Tag::Duration, Tag::Int, BinaryOp::Mul | BinaryOp::Div | BinaryOp::FloorDiv)
                | (Tag::Int, Tag::Duration, BinaryOp::Mul) => Some(Idx::DURATION),
                // Size + Size, Size * int, Size / int, int * Size = Size
                (Tag::Size, Tag::Size, _)
                | (Tag::Size, Tag::Int, BinaryOp::Mul | BinaryOp::Div | BinaryOp::FloorDiv)
                | (Tag::Int, Tag::Size, BinaryOp::Mul) => Some(Idx::SIZE),
                // String concatenation
                (Tag::Str, Tag::Str, BinaryOp::Add) => Some(Idx::STR),
                // Never propagation: right operand diverges
                (_, Tag::Never, _) => Some(Idx::NEVER),
                // Error propagation
                (_, Tag::Error, _) | (Tag::Error, _, _) => Some(Idx::ERROR),
                _ => None,
            };

            if let Some(result) = mixed_result {
                return result;
            }

            // Default: unify left and right operands
            engine.push_context(ContextKind::BinaryOpRight { op: op_str });
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::BinaryOpLeft { op: op_str },
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            // Result type is the left operand type (after unification)
            engine.resolve(left_ty)
        }

        // Comparison: same type in, bool out
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq => {
            // Unify left and right operands
            engine.push_context(ContextKind::ComparisonRight);
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::ComparisonLeft,
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            Idx::BOOL
        }

        // Boolean: bool in, bool out
        BinaryOp::And | BinaryOp::Or => {
            let left_span = arena.get_expr(left).span;
            let right_span = arena.get_expr(right).span;

            // Check left is bool — produce operator-specific message on failure
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);
            match left_tag {
                Tag::Bool | Tag::Error | Tag::Var | Tag::Never => {
                    // Bool is correct, Error/Never propagate silently, Var defers
                    if left_tag != Tag::Never {
                        let bool_expected = Expected {
                            ty: Idx::BOOL,
                            origin: ExpectedOrigin::NoExpectation,
                        };
                        let _ = engine.check_type(left_ty, &bool_expected, left_span);
                    }
                }
                _ => {
                    engine.push_error(TypeCheckError::bad_binary_operand(
                        left_span,
                        "logical",
                        "bool",
                        resolved_left,
                    ));
                }
            }

            // Check right is bool (Never accepted: e.g. `false && panic()`)
            let resolved_right = engine.resolve(right_ty);
            let right_tag = engine.pool().tag(resolved_right);
            match right_tag {
                Tag::Bool | Tag::Error | Tag::Var | Tag::Never => {
                    if right_tag != Tag::Never {
                        let bool_expected = Expected {
                            ty: Idx::BOOL,
                            origin: ExpectedOrigin::NoExpectation,
                        };
                        let _ = engine.check_type(right_ty, &bool_expected, right_span);
                    }
                }
                _ => {
                    engine.push_error(TypeCheckError::bad_binary_operand(
                        right_span,
                        "logical",
                        "bool",
                        resolved_right,
                    ));
                }
            }

            Idx::BOOL
        }

        // Bitwise operations: int operands only
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            let left_span = arena.get_expr(left).span;

            // Check left operand is int (skip Error/Never to prevent cascading)
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);
            match left_tag {
                Tag::Int | Tag::Var => {}
                Tag::Error => return Idx::ERROR,
                Tag::Never => return Idx::NEVER,
                _ => {
                    engine.push_error(TypeCheckError::bad_binary_operand(
                        left_span,
                        "bitwise",
                        "int",
                        resolved_left,
                    ));
                    return Idx::ERROR;
                }
            }

            // Check right operand (also skip Error/Never)
            let resolved_right = engine.resolve(right_ty);
            match engine.pool().tag(resolved_right) {
                Tag::Error => return Idx::ERROR,
                Tag::Never => return Idx::NEVER,
                _ => {}
            }

            // Unify left and right as int
            engine.push_context(ContextKind::BinaryOpRight { op: op_str });
            let expected = Expected {
                ty: Idx::INT,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::BinaryOpLeft { op: op_str },
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            Idx::INT
        }

        // Range creation
        BinaryOp::Range | BinaryOp::RangeInclusive => {
            // Both operands should be the same type (typically int)
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::RangeStart,
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);

            // Return Range<T>
            let elem_ty = engine.resolve(left_ty);
            engine.pool_mut().range(elem_ty)
        }

        // Coalesce: Option<T> ?? T -> T  or  Result<T, E> ?? T -> T
        BinaryOp::Coalesce => {
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);
            match left_tag {
                Tag::Option => {
                    let inner = engine.pool().option_inner(resolved_left);
                    let _ = engine.unify_types(inner, right_ty);
                    engine.resolve(inner)
                }
                Tag::Result => {
                    let ok_ty = engine.pool().result_ok(resolved_left);
                    let _ = engine.unify_types(ok_ty, right_ty);
                    engine.resolve(ok_ty)
                }
                // Unresolved variable — defer via fresh var
                Tag::Var => engine.fresh_var(),
                Tag::Error => Idx::ERROR,
                // Never is the bottom type — expression diverges before coalesce
                Tag::Never => Idx::NEVER,
                _ => {
                    engine.push_error(TypeCheckError::coalesce_requires_option(span));
                    Idx::ERROR
                }
            }
        }
    }
}

/// Infer the type of a unary operation.
fn infer_unary(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    op: UnaryOp,
    operand: ExprId,
    span: Span,
) -> Idx {
    let operand_ty = infer_expr(engine, arena, operand);
    let operand_span = arena.get_expr(operand).span;

    match op {
        // Negation: numeric/duration/size -> same type
        UnaryOp::Neg => {
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);
            match tag {
                Tag::Int | Tag::Float | Tag::Duration => resolved,
                // Propagate errors and defer type variables
                Tag::Error => Idx::ERROR,
                Tag::Var => {
                    // Type variable not yet resolved — unify with int as default
                    let _ = engine.unify_types(operand_ty, Idx::INT);
                    engine.resolve(operand_ty)
                }
                _ => {
                    engine.push_error(TypeCheckError::bad_unary_operand(
                        operand_span,
                        "-",
                        resolved,
                    ));
                    Idx::ERROR
                }
            }
        }

        // Logical not: bool -> bool
        UnaryOp::Not => {
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);
            match tag {
                Tag::Bool => Idx::BOOL,
                // Propagate errors and defer type variables
                Tag::Error => Idx::ERROR,
                Tag::Var => {
                    let _ = engine.unify_types(operand_ty, Idx::BOOL);
                    Idx::BOOL
                }
                _ => {
                    engine.push_error(TypeCheckError::bad_unary_operand(
                        operand_span,
                        "!",
                        resolved,
                    ));
                    Idx::ERROR
                }
            }
        }

        // Bitwise not: int -> int
        UnaryOp::BitNot => {
            engine.push_context(ContextKind::UnaryOpOperand { op: "~" });
            let expected = Expected {
                ty: Idx::INT,
                origin: ExpectedOrigin::NoExpectation,
            };
            let _ = engine.check_type(operand_ty, &expected, operand_span);
            engine.pop_context();
            Idx::INT
        }

        // Try operator: Option<T> -> T or Result<T, E> -> T
        UnaryOp::Try => {
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);

            match tag {
                Tag::Option => engine.pool().option_inner(resolved),
                Tag::Result => engine.pool().result_ok(resolved),
                Tag::Error => Idx::ERROR,
                _ => {
                    engine.push_error(TypeCheckError::try_requires_option_or_result(
                        span, resolved,
                    ));
                    Idx::ERROR
                }
            }
        }
    }
}

// ============================================================================
// Control Flow Inference
// ============================================================================

/// Infer the type of an if expression.
fn infer_if(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    cond: ExprId,
    then_branch: ExprId,
    else_branch: ExprId,
    _span: Span,
) -> Idx {
    // Condition must be bool
    let cond_ty = infer_expr(engine, arena, cond);
    engine.push_context(ContextKind::IfCondition);
    let expected = Expected {
        ty: Idx::BOOL,
        origin: ExpectedOrigin::NoExpectation,
    };
    let _ = engine.check_type(cond_ty, &expected, arena.get_expr(cond).span);
    engine.pop_context();

    // Infer then branch
    engine.push_context(ContextKind::IfThenBranch);
    let then_ty = infer_expr(engine, arena, then_branch);
    engine.pop_context();

    if else_branch.is_present() {
        // Else branch must match then branch
        engine.push_context(ContextKind::IfElseBranch { branch_index: 0 });
        let then_span = arena.get_expr(then_branch).span;
        let expected = Expected {
            ty: then_ty,
            origin: ExpectedOrigin::PreviousInSequence {
                previous_span: then_span,
                current_index: 1,
                sequence_kind: SequenceKind::IfBranches,
            },
        };
        let else_ty = infer_expr(engine, arena, else_branch);
        let _ = engine.check_type(else_ty, &expected, arena.get_expr(else_branch).span);
        engine.pop_context();

        engine.resolve(then_ty)
    } else {
        // No else: if without else has type unit
        // (unless then_branch has type unit or never)
        let resolved_then = engine.resolve(then_ty);
        if resolved_then == Idx::UNIT || resolved_then == Idx::NEVER {
            Idx::UNIT
        } else {
            // Warning: if without else where then is not unit
            // For now, just return unit
            Idx::UNIT
        }
    }
}

// ============================================================================
// Match Expression Inference
// ============================================================================

/// Infer the type of a match expression.
///
/// Match inference follows these steps:
/// 1. Infer the scrutinee type
/// 2. For each arm: check pattern against scrutinee, check guard is bool, infer body
/// 3. Unify all arm body types
/// 4. Return the unified type (or never if no arms)
fn infer_match(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    scrutinee: ExprId,
    arms: ori_ir::ArmRange,
    span: Span,
) -> Idx {
    // Step 1: Infer scrutinee type
    engine.push_context(ContextKind::MatchScrutinee);
    let scrutinee_ty = infer_expr(engine, arena, scrutinee);
    engine.pop_context();

    let arms_slice = arena.get_arms(arms);

    // Empty match returns never (vacuously true that all branches agree)
    if arms_slice.is_empty() {
        return Idx::NEVER;
    }

    // Step 2 & 3: Process arms and unify body types
    let mut result_ty: Option<Idx> = None;
    let scrutinee_span = arena.get_expr(scrutinee).span;

    for (i, arm) in arms_slice.iter().enumerate() {
        // Check pattern against scrutinee type (and bind variables)
        engine.push_context(ContextKind::MatchArmPattern { arm_index: i });
        #[expect(clippy::cast_possible_truncation, reason = "arm index fits in u32")]
        let arm_key = PatternKey::Arm(arms.start + i as u32);
        check_match_pattern(engine, arena, &arm.pattern, scrutinee_ty, arm_key, arm.span);
        engine.pop_context();

        // Check guard is bool (if present)
        if let Some(guard_id) = arm.guard {
            engine.push_context(ContextKind::MatchArmGuard { arm_index: i });
            let guard_ty = infer_expr(engine, arena, guard_id);
            let expected = Expected {
                ty: Idx::BOOL,
                origin: ExpectedOrigin::Context {
                    span: arena.get_expr(guard_id).span,
                    kind: ContextKind::MatchArmGuard { arm_index: i },
                },
            };
            let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard_id).span);
            engine.pop_context();
        }

        // Infer body type
        engine.push_context(ContextKind::MatchArm { arm_index: i });
        let body_ty = infer_expr(engine, arena, arm.body);
        engine.pop_context();

        // Unify with previous arms
        match result_ty {
            None => {
                // First arm establishes the result type
                result_ty = Some(body_ty);
            }
            Some(prev_ty) => {
                // Subsequent arms must match the first
                let expected = Expected {
                    ty: prev_ty,
                    origin: ExpectedOrigin::PreviousInSequence {
                        previous_span: scrutinee_span,
                        current_index: i,
                        sequence_kind: SequenceKind::MatchArms,
                    },
                };
                let _ = engine.check_type(body_ty, &expected, arena.get_expr(arm.body).span);
            }
        }

        // Exit pattern bindings scope (patterns introduce local bindings)
        // Note: Variables bound in patterns are only visible in that arm's body
        // This is handled by enter/exit scope around pattern checking
    }

    // Return the unified type, or error if something went wrong
    if let Some(ty) = result_ty {
        engine.resolve(ty)
    } else {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            1,
            0,
            crate::ArityMismatchKind::Pattern,
        ));
        Idx::ERROR
    }
}

/// Check a match pattern against an expected type, binding variables in the environment.
///
/// This function validates that a pattern can match values of the given type,
/// and binds any variable names introduced by the pattern.
///
/// The `pattern_key` identifies this pattern for resolution lookup. For top-level
/// arm patterns it's `PatternKey::Arm(arms.start + i)`, for nested patterns it's
/// `PatternKey::Nested(match_pattern_id.raw())`.
fn check_match_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::MatchPattern,
    expected_ty: Idx,
    pattern_key: PatternKey,
    span: Span,
) {
    use ori_ir::MatchPattern;

    match pattern {
        // Wildcard matches anything
        MatchPattern::Wildcard => {}

        // Binding: either a variable binding or an ambiguous unit variant.
        //
        // The parser can't distinguish `Pending` (unit variant) from `x` (binding)
        // without type context. We resolve this here by checking if the name is a
        // unit variant of the scrutinee's enum type.
        MatchPattern::Binding(name) => {
            let resolved = engine.resolve(expected_ty);
            let tag = engine.pool().tag(resolved);

            // Check if this name is a unit variant of the scrutinee's enum type
            let is_unit_variant = if matches!(tag, Tag::Named | Tag::Applied) {
                let scrutinee_name = if tag == Tag::Named {
                    engine.pool().named_name(resolved)
                } else {
                    engine.pool().applied_name(resolved)
                };
                engine.type_registry().and_then(|reg| {
                    let (type_entry, variant_def) = reg.lookup_variant_def(*name)?;
                    // CRITICAL: variant must belong to the scrutinee's type, not any enum
                    if type_entry.name != scrutinee_name {
                        return None;
                    }
                    if !variant_def.fields.is_unit() {
                        return None;
                    }
                    let (_, variant_idx) = reg.lookup_variant(*name)?;
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "enums are limited to 256 variants"
                    )]
                    Some((type_entry.name, variant_idx as u8))
                })
            } else {
                None
            };

            if let Some((type_name, variant_index)) = is_unit_variant {
                engine.record_pattern_resolution(
                    pattern_key,
                    PatternResolution::UnitVariant {
                        type_name,
                        variant_index,
                    },
                );
                // Do NOT bind name — it's a constructor, not a variable
            } else {
                engine.env_mut().bind(*name, expected_ty);
            }
        }

        // Literal must have compatible type
        MatchPattern::Literal(expr_id) => {
            let lit_ty = infer_expr(engine, arena, *expr_id);
            let _ = engine.unify_types(lit_ty, expected_ty);
        }

        // Variant pattern: extract inner type if it's an enum/Option/Result
        MatchPattern::Variant { name, inner } => {
            let resolved = engine.resolve(expected_ty);
            let tag = engine.pool().tag(resolved);

            // Handle known container types
            let inner_types = match tag {
                Tag::Option => {
                    // Some(x) pattern - inner has one element with inner type
                    vec![engine.pool().option_inner(resolved)]
                }
                Tag::Result => {
                    // Ok(x) or Err(e) pattern - use variant name to select inner type
                    let variant_str = engine.lookup_name(*name);
                    match variant_str {
                        Some("Err") => vec![engine.pool().result_err(resolved)],
                        _ => vec![engine.pool().result_ok(resolved)],
                    }
                }
                Tag::Named | Tag::Applied => {
                    // User-defined enum: look up variant field types from TypeRegistry,
                    // substituting any generic type parameters with concrete types from
                    // the scrutinee's type arguments.
                    let result = engine.type_registry().and_then(|reg| {
                        let (type_entry, variant_def) = reg.lookup_variant_def(*name)?;
                        let field_types: Vec<Idx> = match &variant_def.fields {
                            VariantFields::Unit => vec![],
                            VariantFields::Tuple(types) => types.clone(),
                            VariantFields::Record(fields) => fields.iter().map(|f| f.ty).collect(),
                        };
                        Some((type_entry.type_params.clone(), field_types))
                    });

                    match result {
                        Some((type_params, field_types)) if type_params.is_empty() => {
                            // Non-generic enum: field types are concrete, use directly
                            field_types
                        }
                        Some((type_params, field_types)) => {
                            // Generic enum: substitute type parameters with concrete
                            // type arguments from the scrutinee.
                            // e.g., scrutinee `MyResult<int, str>` → T=int, E=str
                            let type_args = if tag == Tag::Applied {
                                engine.pool().applied_args(resolved)
                            } else {
                                vec![]
                            };

                            if type_args.len() == type_params.len() {
                                // Build param→arg mapping and substitute
                                let substituted: Vec<Idx> = field_types
                                    .iter()
                                    .map(|&ft| {
                                        substitute_type_params(engine, ft, &type_params, &type_args)
                                    })
                                    .collect();
                                substituted
                            } else {
                                // Mismatch between expected and actual type args — use
                                // fresh variables as fallback
                                let inner_ids = arena.get_match_pattern_list(*inner);
                                inner_ids.iter().map(|_| engine.fresh_var()).collect()
                            }
                        }
                        None => {
                            // Variant not found — fall back to fresh variables
                            let inner_ids = arena.get_match_pattern_list(*inner);
                            inner_ids.iter().map(|_| engine.fresh_var()).collect()
                        }
                    }
                }
                _ => {
                    // Unknown tag — fall back to fresh variables
                    let inner_ids = arena.get_match_pattern_list(*inner);
                    inner_ids.iter().map(|_| engine.fresh_var()).collect()
                }
            };

            // Check inner patterns
            let inner_ids = arena.get_match_pattern_list(*inner);
            for (inner_id, inner_ty) in inner_ids.iter().zip(inner_types.iter()) {
                let inner_pattern = arena.get_match_pattern(*inner_id);
                let nested_key = PatternKey::Nested(inner_id.raw());
                check_match_pattern(engine, arena, inner_pattern, *inner_ty, nested_key, span);
            }
        }

        // Tuple pattern: check each element
        MatchPattern::Tuple(inner) => {
            let resolved = engine.resolve(expected_ty);

            if engine.pool().tag(resolved) == Tag::Tuple {
                let elem_types = engine.pool().tuple_elems(resolved);
                let inner_ids = arena.get_match_pattern_list(*inner);

                // Check arity
                if inner_ids.len() != elem_types.len() {
                    engine.push_error(TypeCheckError::arity_mismatch(
                        span,
                        elem_types.len(),
                        inner_ids.len(),
                        crate::ArityMismatchKind::Pattern,
                    ));
                    return;
                }

                // Check each element
                for (inner_id, elem_ty) in inner_ids.iter().zip(elem_types.iter()) {
                    let inner_pattern = arena.get_match_pattern(*inner_id);
                    let nested_key = PatternKey::Nested(inner_id.raw());
                    check_match_pattern(engine, arena, inner_pattern, *elem_ty, nested_key, span);
                }
            } else if resolved != Idx::ERROR {
                // Not a tuple type
                engine.push_error(TypeCheckError::mismatch(
                    span,
                    expected_ty,
                    resolved,
                    vec![],
                    crate::ErrorContext::new(ContextKind::PatternMatch {
                        pattern_kind: "tuple",
                    }),
                ));
            }
        }

        // List pattern: check elements and rest
        MatchPattern::List { elements, rest } => {
            let resolved = engine.resolve(expected_ty);

            if engine.pool().tag(resolved) == Tag::List {
                let elem_ty = engine.pool().list_elem(resolved);
                let elem_ids = arena.get_match_pattern_list(*elements);

                // Check each element pattern
                for inner_id in elem_ids {
                    let inner_pattern = arena.get_match_pattern(*inner_id);
                    let nested_key = PatternKey::Nested(inner_id.raw());
                    check_match_pattern(engine, arena, inner_pattern, elem_ty, nested_key, span);
                }

                // Bind rest pattern to list type
                if let Some(rest_name) = rest {
                    engine.env_mut().bind(*rest_name, resolved);
                }
            } else if resolved != Idx::ERROR {
                // Not a list type
                engine.push_error(TypeCheckError::mismatch(
                    span,
                    expected_ty,
                    resolved,
                    vec![],
                    crate::ErrorContext::new(ContextKind::PatternMatch {
                        pattern_kind: "list",
                    }),
                ));
            }
        }

        // Struct pattern: check field types against registry
        MatchPattern::Struct { fields, .. } => {
            let resolved = engine.resolve(expected_ty);
            let field_type_map = match engine.pool().tag(resolved) {
                Tag::Named => {
                    let type_name = engine.pool().named_name(resolved);
                    lookup_struct_field_types(engine, type_name, None)
                }
                Tag::Applied => {
                    let type_name = engine.pool().applied_name(resolved);
                    let type_args = engine.pool().applied_args(resolved);
                    lookup_struct_field_types(engine, type_name, Some(&type_args))
                }
                _ => None,
            };

            for (name, inner_pattern) in fields {
                let field_ty = field_type_map
                    .as_ref()
                    .and_then(|m| m.get(name).copied())
                    .unwrap_or_else(|| engine.fresh_var());
                if let Some(inner_id) = inner_pattern {
                    let inner = arena.get_match_pattern(*inner_id);
                    let nested_key = PatternKey::Nested(inner_id.raw());
                    check_match_pattern(engine, arena, inner, field_ty, nested_key, span);
                } else {
                    // Shorthand: `{ x }` binds x to the field value
                    engine.env_mut().bind(*name, field_ty);
                }
            }
        }

        // Range pattern: check bounds
        MatchPattern::Range { start, end, .. } => {
            if let Some(start_id) = start {
                let start_ty = infer_expr(engine, arena, *start_id);
                let _ = engine.unify_types(start_ty, expected_ty);
            }
            if let Some(end_id) = end {
                let end_ty = infer_expr(engine, arena, *end_id);
                let _ = engine.unify_types(end_ty, expected_ty);
            }
        }

        // Or pattern: all alternatives must match the same type
        MatchPattern::Or(alternatives) => {
            let alt_ids = arena.get_match_pattern_list(*alternatives);
            for alt_id in alt_ids {
                let alt_pattern = arena.get_match_pattern(*alt_id);
                let nested_key = PatternKey::Nested(alt_id.raw());
                check_match_pattern(engine, arena, alt_pattern, expected_ty, nested_key, span);
            }
        }

        // At pattern: bind name and check inner pattern
        MatchPattern::At {
            name,
            pattern: inner_id,
        } => {
            engine.env_mut().bind(*name, expected_ty);
            let inner_pattern = arena.get_match_pattern(*inner_id);
            let nested_key = PatternKey::Nested(inner_id.raw());
            check_match_pattern(engine, arena, inner_pattern, expected_ty, nested_key, span);
        }
    }
}

/// Substitute generic type parameters in a field type with concrete type arguments.
///
/// Given a field type like `Named("T")` and a mapping `[T] → [int]`, returns `int`.
/// For compound types (lists, tuples, functions, applied types), recurses into children.
/// Non-parameterized types (primitives, error, etc.) are returned unchanged.
fn substitute_type_params(
    engine: &mut InferEngine<'_>,
    field_ty: Idx,
    type_params: &[ori_ir::Name],
    type_args: &[Idx],
) -> Idx {
    let resolved = engine.resolve(field_ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Named => {
            // Check if this named type is one of the type parameters
            let name = engine.pool().named_name(resolved);
            for (i, &param_name) in type_params.iter().enumerate() {
                if name == param_name {
                    return type_args[i];
                }
            }
            // Not a type parameter — return as-is (concrete named type)
            resolved
        }
        Tag::Applied => {
            // Recurse into applied type arguments: e.g., List<T> → List<int>
            let app_name = engine.pool().applied_name(resolved);
            let args = engine.pool().applied_args(resolved);
            let substituted_args: Vec<Idx> = args
                .iter()
                .map(|&arg| substitute_type_params(engine, arg, type_params, type_args))
                .collect();
            engine.pool_mut().applied(app_name, &substituted_args)
        }
        Tag::List => {
            let elem = engine.pool().list_elem(resolved);
            let sub_elem = substitute_type_params(engine, elem, type_params, type_args);
            engine.pool_mut().list(sub_elem)
        }
        Tag::Tuple => {
            let elems = engine.pool().tuple_elems(resolved);
            let sub_elems: Vec<Idx> = elems
                .iter()
                .map(|&e| substitute_type_params(engine, e, type_params, type_args))
                .collect();
            engine.pool_mut().tuple(&sub_elems)
        }
        Tag::Function => {
            let params = engine.pool().function_params(resolved);
            let ret = engine.pool().function_return(resolved);
            let sub_params: Vec<Idx> = params
                .iter()
                .map(|&p| substitute_type_params(engine, p, type_params, type_args))
                .collect();
            let sub_ret = substitute_type_params(engine, ret, type_params, type_args);
            engine.pool_mut().function(&sub_params, sub_ret)
        }
        Tag::Option => {
            let inner = engine.pool().option_inner(resolved);
            let sub_inner = substitute_type_params(engine, inner, type_params, type_args);
            engine.pool_mut().option(sub_inner)
        }
        Tag::Result => {
            let ok = engine.pool().result_ok(resolved);
            let err = engine.pool().result_err(resolved);
            let sub_ok = substitute_type_params(engine, ok, type_params, type_args);
            let sub_err = substitute_type_params(engine, err, type_params, type_args);
            engine.pool_mut().result(sub_ok, sub_err)
        }
        Tag::Map => {
            let key = engine.pool().map_key(resolved);
            let val = engine.pool().map_value(resolved);
            let sub_key = substitute_type_params(engine, key, type_params, type_args);
            let sub_val = substitute_type_params(engine, val, type_params, type_args);
            engine.pool_mut().map(sub_key, sub_val)
        }
        // Primitives and other leaf types — no substitution needed
        _ => resolved,
    }
}

/// Substitute type parameters using a pre-built map of (Name, Idx) pairs.
///
/// This is a convenience wrapper around `substitute_type_params` that accepts
/// a map representation rather than parallel arrays.
fn substitute_type_params_with_map(
    engine: &mut InferEngine<'_>,
    field_ty: Idx,
    subst_map: &[(Name, Idx)],
) -> Idx {
    if subst_map.is_empty() {
        return field_ty;
    }
    let type_params: Vec<Name> = subst_map.iter().map(|(n, _)| *n).collect();
    let type_args: Vec<Idx> = subst_map.iter().map(|(_, i)| *i).collect();
    substitute_type_params(engine, field_ty, &type_params, &type_args)
}

// ============================================================================
// Loop Inference
// ============================================================================

/// Infer the type of a for loop.
///
/// For loops in Ori can be used in two forms:
/// - `for x in iter do body` - returns unit, iterates for side effects
/// - `for x in iter yield body` - returns a list, collects body results
///
/// The iterator must be iterable (list, range, etc.), and the binding
/// receives each element type.
// TODO(inference): Refactor with a ForLoopParams struct when implementing
#[expect(clippy::too_many_arguments, reason = "matches ExprKind::For structure")]
fn infer_for(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    binding: Name,
    iter: ExprId,
    guard: ExprId,
    body: ExprId,
    is_yield: bool,
    _span: Span,
) -> Idx {
    // Enter scope for loop binding
    engine.enter_scope();

    // Infer iterator type
    engine.push_context(ContextKind::ForIterator);
    let iter_ty = infer_expr(engine, arena, iter);
    engine.pop_context();

    // Extract element type from iterator
    let resolved_iter = engine.resolve(iter_ty);
    let tag = engine.pool().tag(resolved_iter);

    let elem_ty = match tag {
        Tag::List => engine.pool().list_elem(resolved_iter),
        Tag::Range => engine.pool().range_elem(resolved_iter),
        Tag::Map => {
            // Iterating over a map yields (key, value) tuples
            let key_ty = engine.pool().map_key(resolved_iter);
            let value_ty = engine.pool().map_value(resolved_iter);
            engine.pool_mut().tuple(&[key_ty, value_ty])
        }
        Tag::Set => {
            // Sets store elements similarly to lists (single type parameter)
            engine.pool().set_elem(resolved_iter)
        }
        _ => {
            // Not a known iterable - still allow iteration with fresh element type
            // The type checker will catch concrete type mismatches later
            engine.fresh_var()
        }
    };

    // Bind the loop variable
    engine.push_context(ContextKind::ForBinding);
    engine.env_mut().bind(binding, elem_ty);
    engine.pop_context();

    // Check guard if present (must be bool)
    if guard.is_present() {
        let guard_ty = infer_expr(engine, arena, guard);
        let expected = Expected {
            ty: Idx::BOOL,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(guard).span,
                kind: ContextKind::LoopCondition,
            },
        };
        let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard).span);
    }

    // Infer body type
    engine.push_context(ContextKind::LoopBody);
    let body_ty = infer_expr(engine, arena, body);
    engine.pop_context();

    // Exit loop scope
    engine.exit_scope();

    // Return type depends on do vs yield
    if is_yield {
        // yield: collect results into a list
        let resolved_body = engine.resolve(body_ty);
        engine.pool_mut().list(resolved_body)
    } else {
        // do: iterate for side effects, return unit
        Idx::UNIT
    }
}

/// Infer the type of an infinite loop.
///
/// `loop { body }` runs the body repeatedly until a `break` is encountered.
/// The loop type is determined by break expressions within the body:
/// - If breaks have values, the loop returns that type
/// - If no breaks, the loop returns `never` (runs forever)
fn infer_loop(engine: &mut InferEngine<'_>, arena: &ExprArena, body: ExprId, _span: Span) -> Idx {
    // Create a fresh type variable for the loop's result (determined by break values)
    let break_ty = engine.fresh_var();
    engine.push_loop_break_type(break_ty);

    // Enter scope for loop
    engine.enter_scope();

    // Infer body type (break expressions unify their value with break_ty)
    engine.push_context(ContextKind::LoopBody);
    let _body_ty = infer_expr(engine, arena, body);
    engine.pop_context();

    // Exit loop scope
    engine.exit_scope();
    engine.pop_loop_break_type();

    // Resolve the break type — if no break was encountered, the variable
    // stays unresolved (infinite loop returns Never). If breaks exist,
    // it unifies to their value type.
    let resolved = engine.resolve(break_ty);
    if engine.pool().tag(resolved) == Tag::Var {
        // No break was encountered — this is an infinite loop (returns Never).
        // Note: `break` without a value unifies break_ty with Unit, so
        // Tag::Var here means truly no break exists in the loop body.
        Idx::NEVER
    } else {
        resolved
    }
}

fn infer_block(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmts: ori_ir::StmtRange,
    result: ExprId,
    _span: Span,
) -> Idx {
    // Enter binding scope for the block.
    // All let bindings within this block will be isolated from parent scope.
    engine.enter_scope();

    // Process statements
    for stmt in arena.get_stmt_range(stmts) {
        match &stmt.kind {
            ori_ir::StmtKind::Expr(expr_id) => {
                let _ = infer_expr(engine, arena, *expr_id);
            }
            ori_ir::StmtKind::Let {
                pattern,
                ty,
                init,
                mutable: _,
            } => {
                let pat = arena.get_binding_pattern(*pattern);

                // Enter rank scope for let-polymorphism (not binding scope).
                // This allows type variables in the initializer to be generalized.
                engine.enter_rank_scope();

                // Check/infer the initializer type based on presence of annotation
                let final_ty = if ty.is_valid() {
                    // With type annotation: use bidirectional checking
                    let parsed_ty = arena.get_parsed_type(*ty);
                    let expected_ty = resolve_parsed_type(engine, arena, parsed_ty);
                    let expected = Expected {
                        ty: expected_ty,
                        origin: ExpectedOrigin::Annotation {
                            name: pattern_first_name(pat).unwrap_or(Name::EMPTY),
                            span: stmt.span,
                        },
                    };
                    let _init_ty = check_expr(engine, arena, *init, &expected, stmt.span);
                    expected_ty
                } else {
                    // No annotation: infer and generalize for let-polymorphism
                    let init_ty = infer_expr(engine, arena, *init);
                    engine.generalize(init_ty)
                };

                // Exit rank scope (but stay in block's binding scope)
                engine.exit_rank_scope();

                // Bind pattern to the block's scope.
                // The binding is visible to subsequent statements and the result.
                bind_pattern(engine, arena, pat, final_ty);
            }
        }
    }

    // Block type is the result expression type, or unit
    let block_ty = if result.is_present() {
        infer_expr(engine, arena, result)
    } else {
        Idx::UNIT
    };

    // Exit block scope - bindings are no longer visible
    engine.exit_scope();

    block_ty
}

fn infer_let(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::BindingPattern,
    ty_annotation: Option<&ori_ir::ParsedType>,
    init: ExprId,
    // Mutability is an effect, not a type property in Ori's HM inference system.
    // Enforcement happens in the evaluator (`bind_can_pattern`) and codegen backends,
    // not here. Kept as a parameter for future "cannot assign to immutable binding"
    // diagnostics (like Rust's type checker emits).
    _mutable: bool,
    span: Span,
) -> Idx {
    // Enter scope for let-polymorphism.
    // This increases the rank so that type variables created during
    // initializer inference can be generalized.
    engine.enter_scope();

    let binding_name = pattern_first_name(pattern);
    let errors_before = engine.error_count();

    // Check/infer the initializer type based on presence of annotation
    let final_ty = if let Some(parsed_ty) = ty_annotation {
        // With type annotation: use bidirectional checking (allows literal coercion)
        let expected_ty = resolve_parsed_type(engine, arena, parsed_ty);
        let expected = Expected {
            ty: expected_ty,
            origin: ExpectedOrigin::Annotation {
                name: pattern_first_name(pattern).unwrap_or(Name::EMPTY),
                span,
            },
        };
        // Use check_expr for bidirectional type checking (literal coercion)
        let _init_ty = check_expr(engine, arena, init, &expected, span);
        expected_ty
    } else {
        // No annotation: infer the initializer type
        let init_ty = infer_expr(engine, arena, init);

        // Detect closure self-capture: if the init is a lambda and any new errors
        // are UnknownIdent matching the binding name, it's a self-capture attempt.
        // Example: `let f = () -> f` — the closure body references `f`, which isn't
        // yet in scope. This would create a reference cycle under ARC.
        if let Some(name) = binding_name {
            if matches!(arena.get_expr(init).kind, ExprKind::Lambda { .. }) {
                engine.rewrite_self_capture_errors(name, errors_before);
            }
        }

        // Generalize free type variables for let-polymorphism.
        // Variables created at the current (elevated) rank will be quantified.
        engine.generalize(init_ty)
    };

    // Exit scope (rank goes back down).
    // The binding will be added to the outer environment.
    engine.exit_scope();

    // Bind the pattern to the (possibly generalized) type
    bind_pattern(engine, arena, pattern, final_ty);

    // Let expression returns unit
    Idx::UNIT
}

/// Get the first name from a binding pattern (for error messages).
fn pattern_first_name(pattern: &ori_ir::BindingPattern) -> Option<Name> {
    match pattern {
        ori_ir::BindingPattern::Name { name, .. } => Some(*name),
        ori_ir::BindingPattern::Tuple(pats) => pats.first().and_then(pattern_first_name),
        ori_ir::BindingPattern::Struct { fields } => fields.first().map(|field| field.name),
        ori_ir::BindingPattern::List { elements, .. } => {
            elements.first().and_then(pattern_first_name)
        }
        ori_ir::BindingPattern::Wildcard => None,
    }
}

fn infer_lambda(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    params: ori_ir::ParamRange,
    ret_ty: Option<&ori_ir::ParsedType>,
    body: ExprId,
    span: Span,
) -> Idx {
    // Enter a new scope for the lambda
    engine.enter_scope();

    // Create types for parameters
    let mut param_types = Vec::new();
    for param in arena.get_params(params) {
        let param_ty = if let Some(ref parsed_ty) = param.ty {
            resolve_parsed_type(engine, arena, parsed_ty)
        } else {
            engine.fresh_var()
        };
        engine.env_mut().bind(param.name, param_ty);
        param_types.push(param_ty);
    }

    // Infer body type, checking against return annotation if present
    let body_ty = if let Some(ret_parsed) = ret_ty {
        let expected_ty = resolve_parsed_type(engine, arena, ret_parsed);
        let inferred = infer_expr(engine, arena, body);
        let expected = Expected {
            ty: expected_ty,
            origin: ExpectedOrigin::Context {
                span,
                kind: ContextKind::FunctionReturn { func_name: None },
            },
        };
        let _ = engine.check_type(inferred, &expected, arena.get_expr(body).span);
        expected_ty
    } else {
        infer_expr(engine, arena, body)
    };

    // Exit scope
    engine.exit_scope();

    // Create function type
    engine.infer_function(&param_types, body_ty)
}

// Collection inference stubs
fn infer_list(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::ExprRange,
    _span: Span,
) -> Idx {
    let elem_ids: Vec<_> = arena.get_expr_list(elements).to_vec();

    if elem_ids.is_empty() {
        return engine.infer_empty_list();
    }

    // Infer first element
    let first_ty = infer_expr(engine, arena, elem_ids[0]);
    let first_span = arena.get_expr(elem_ids[0]).span;

    // Check remaining elements
    for (i, &elem_id) in elem_ids.iter().skip(1).enumerate() {
        let expected = Expected {
            ty: first_ty,
            origin: ExpectedOrigin::PreviousInSequence {
                previous_span: first_span,
                current_index: i + 1,
                sequence_kind: SequenceKind::ListLiteral,
            },
        };
        let elem_ty = infer_expr(engine, arena, elem_id);
        let _ = engine.check_type(elem_ty, &expected, arena.get_expr(elem_id).span);
    }

    let resolved_elem = engine.resolve(first_ty);
    engine.infer_list(resolved_elem)
}

fn infer_list_spread(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::ListElementRange,
    _span: Span,
) -> Idx {
    use ori_ir::ListElement;

    let elems = arena.get_list_elements(elements);
    if elems.is_empty() {
        return engine.infer_empty_list();
    }

    // Unified element type — start with a fresh variable
    let elem_ty = engine.fresh_var();

    for element in elems {
        match element {
            ListElement::Expr {
                expr,
                span: el_span,
            } => {
                let ty = infer_expr(engine, arena, *expr);
                if engine.unify_types(ty, elem_ty).is_err() {
                    engine.push_error(TypeCheckError::mismatch(
                        *el_span,
                        elem_ty,
                        ty,
                        vec![],
                        crate::ErrorContext::new(ContextKind::ListElement { index: 0 }),
                    ));
                }
            }
            ListElement::Spread {
                expr,
                span: sp_span,
            } => {
                let spread_ty = infer_expr(engine, arena, *expr);
                let resolved = engine.resolve(spread_ty);
                if engine.pool().tag(resolved) == Tag::List {
                    let inner = engine.pool().list_elem(resolved);
                    if engine.unify_types(inner, elem_ty).is_err() {
                        engine.push_error(TypeCheckError::mismatch(
                            *sp_span,
                            elem_ty,
                            inner,
                            vec![],
                            crate::ErrorContext::new(ContextKind::ListElement { index: 0 }),
                        ));
                    }
                } else if resolved != Idx::ERROR {
                    // Spread target must be a list
                    let expected_list = engine.infer_list(elem_ty);
                    engine.push_error(TypeCheckError::mismatch(
                        *sp_span,
                        expected_list,
                        resolved,
                        vec![],
                        crate::ErrorContext::new(ContextKind::PatternMatch {
                            pattern_kind: "list spread",
                        }),
                    ));
                }
            }
        }
    }

    let resolved_elem = engine.resolve(elem_ty);
    engine.infer_list(resolved_elem)
}

fn infer_tuple(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::ExprRange,
    _span: Span,
) -> Idx {
    let elem_ids: Vec<_> = arena.get_expr_list(elements).to_vec();
    let elem_types: Vec<_> = elem_ids
        .iter()
        .map(|&id| infer_expr(engine, arena, id))
        .collect();
    engine.infer_tuple(&elem_types)
}

fn infer_map_literal(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    entries: ori_ir::MapEntryRange,
    _span: Span,
) -> Idx {
    let entries_slice = arena.get_map_entries(entries);

    if entries_slice.is_empty() {
        return engine.infer_empty_map();
    }

    // Infer first entry
    let first_entry = &entries_slice[0];
    let first_key_ty = infer_expr(engine, arena, first_entry.key);
    let first_val_ty = infer_expr(engine, arena, first_entry.value);

    // Check remaining entries
    for entry in entries_slice.iter().skip(1) {
        let key_ty = infer_expr(engine, arena, entry.key);
        let val_ty = infer_expr(engine, arena, entry.value);
        let _ = engine.unify_types(key_ty, first_key_ty);
        let _ = engine.unify_types(val_ty, first_val_ty);
    }

    let resolved_key = engine.resolve(first_key_ty);
    let resolved_val = engine.resolve(first_val_ty);
    engine.infer_map(resolved_key, resolved_val)
}

fn infer_map_spread(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::MapElementRange,
    _span: Span,
) -> Idx {
    use ori_ir::MapElement;

    let elems = arena.get_map_elements(elements);
    if elems.is_empty() {
        return engine.infer_empty_map();
    }

    // Unified key and value types — start with fresh variables
    let key_ty = engine.fresh_var();
    let val_ty = engine.fresh_var();

    for element in elems {
        match element {
            MapElement::Entry(entry) => {
                let k = infer_expr(engine, arena, entry.key);
                let v = infer_expr(engine, arena, entry.value);
                let _ = engine.unify_types(k, key_ty);
                let _ = engine.unify_types(v, val_ty);
            }
            MapElement::Spread {
                expr,
                span: sp_span,
            } => {
                let spread_ty = infer_expr(engine, arena, *expr);
                let resolved = engine.resolve(spread_ty);
                if engine.pool().tag(resolved) == Tag::Map {
                    let k = engine.pool().map_key(resolved);
                    let v = engine.pool().map_value(resolved);
                    let _ = engine.unify_types(k, key_ty);
                    let _ = engine.unify_types(v, val_ty);
                } else if resolved != Idx::ERROR {
                    // Spread target must be a map
                    let expected_map = engine.infer_map(key_ty, val_ty);
                    engine.push_error(TypeCheckError::mismatch(
                        *sp_span,
                        expected_map,
                        resolved,
                        vec![],
                        crate::ErrorContext::new(ContextKind::PatternMatch {
                            pattern_kind: "map spread",
                        }),
                    ));
                }
            }
        }
    }

    let resolved_key = engine.resolve(key_ty);
    let resolved_val = engine.resolve(val_ty);
    engine.infer_map(resolved_key, resolved_val)
}

fn infer_range(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    start: ExprId,
    end: ExprId,
    step: ExprId,
    _inclusive: bool,
    _span: Span,
) -> Idx {
    // Determine element type from provided bounds
    let elem_ty = if start.is_present() {
        infer_expr(engine, arena, start)
    } else if end.is_present() {
        infer_expr(engine, arena, end)
    } else {
        Idx::INT // Default to int for open ranges
    };

    // Unify all provided bounds
    if start.is_present() {
        let ty = infer_expr(engine, arena, start);
        let _ = engine.unify_types(ty, elem_ty);
    }
    if end.is_present() {
        let ty = infer_expr(engine, arena, end);
        let _ = engine.unify_types(ty, elem_ty);
    }
    if step.is_present() {
        let ty = infer_expr(engine, arena, step);
        let _ = engine.unify_types(ty, elem_ty);
    }

    let resolved = engine.resolve(elem_ty);
    engine.pool_mut().range(resolved)
}

// ============================================================================
// Struct Inference
// ============================================================================

/// Find type names similar to `target` in the type registry (for typo suggestions).
fn find_similar_type_names(
    engine: &InferEngine<'_>,
    type_registry: &TypeRegistry,
    target: Name,
) -> Vec<Name> {
    let Some(target_str) = engine.lookup_name(target) else {
        return Vec::new();
    };

    if target_str.is_empty() {
        return Vec::new();
    }

    let threshold = match target_str.len() {
        0 => return Vec::new(),
        1..=2 => 1,
        3..=5 => 2,
        _ => 3,
    };

    let mut matches: Vec<(Name, usize)> = type_registry
        .names()
        .filter(|&n| n != target)
        .filter_map(|candidate_name| {
            let candidate_str = engine.lookup_name(candidate_name)?;
            let len_diff = target_str.len().abs_diff(candidate_str.len());
            if len_diff > threshold {
                return None;
            }
            let distance = crate::edit_distance(target_str, candidate_str);
            (distance <= threshold).then_some((candidate_name, distance))
        })
        .collect();

    matches.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    matches.into_iter().take(3).map(|(n, _)| n).collect()
}

/// Infer type for a struct literal: `Point { x: 1, y: 2 }`.
///
/// Performs:
/// 1. Type registry lookup to find the struct definition
/// 2. Fresh type variable creation for generic type parameters
/// 3. Type parameter substitution in field types
/// 4. Field validation (unknown fields, duplicate fields, missing fields)
/// 5. Unification of provided field values with expected field types
fn infer_struct(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    name: Name,
    fields: ori_ir::FieldInitRange,
    span: Span,
) -> Idx {
    // Step 1: Look up the struct type in the registry
    let Some(type_registry) = engine.type_registry() else {
        // No type registry — infer field values but can't validate
        let field_inits = arena.get_field_inits(fields);
        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
        return Idx::ERROR;
    };

    let Some(entry) = type_registry.get_by_name(name).cloned() else {
        // Unknown type name — find similar type names for suggestions
        let similar = find_similar_type_names(engine, type_registry, name);
        engine.push_error(TypeCheckError::unknown_ident(span, name, similar));
        let field_inits = arena.get_field_inits(fields);
        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
        return Idx::ERROR;
    };

    // Step 2: Verify it's a struct — move struct_def out of the already-owned entry
    let entry_idx = entry.idx;
    let type_params = entry.type_params;
    let TypeKind::Struct(struct_def) = entry.kind else {
        engine.push_error(TypeCheckError::not_a_struct(span, name));
        let field_inits = arena.get_field_inits(fields);
        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
        return Idx::ERROR;
    };

    // Step 3: Create fresh type variables for generic params
    let type_param_subst: FxHashMap<Name, Idx> = type_params
        .iter()
        .map(|&param_name| (param_name, engine.fresh_var()))
        .collect();

    // Step 4: Build expected field types with substitution
    let expected_fields: Vec<(Name, Idx)> = struct_def
        .fields
        .iter()
        .map(|f| {
            let ty = if type_param_subst.is_empty() {
                f.ty
            } else {
                substitute_named_types(engine.pool_mut(), f.ty, &type_param_subst)
            };
            (f.name, ty)
        })
        .collect();

    let expected_map: FxHashMap<Name, Idx> = expected_fields.iter().copied().collect();

    // Step 5: Check provided fields
    let field_inits = arena.get_field_inits(fields);
    let mut provided_fields: FxHashSet<Name> =
        FxHashSet::with_capacity_and_hasher(field_inits.len(), rustc_hash::FxBuildHasher);

    for init in field_inits {
        // Check for duplicate fields
        if !provided_fields.insert(init.name) {
            engine.push_error(TypeCheckError::duplicate_field(init.span, name, init.name));
            continue;
        }

        if let Some(&expected_ty) = expected_map.get(&init.name) {
            // Known field — infer value and unify with expected type
            let actual_ty = if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id)
            } else {
                // Shorthand: `Point { x }` means `Point { x: x }`
                infer_ident(engine, init.name, init.span)
            };
            let _ = engine.unify_types(actual_ty, expected_ty);
        } else {
            // Unknown field — report error, still infer value
            let available: Vec<Name> = expected_fields.iter().map(|(n, _)| *n).collect();
            engine.push_error(TypeCheckError::undefined_field(
                init.span, entry_idx, init.name, available,
            ));
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
    }

    // Step 6: Check for missing fields
    let missing: Vec<Name> = expected_fields
        .iter()
        .filter(|(field_name, _)| !provided_fields.contains(field_name))
        .map(|(field_name, _)| *field_name)
        .collect();

    if !missing.is_empty() {
        engine.push_error(TypeCheckError::missing_fields(span, name, missing));
    }

    // Step 7: Return the struct type
    if type_param_subst.is_empty() {
        engine.pool_mut().named(name)
    } else {
        let type_args: Vec<Idx> = type_params
            .iter()
            .map(|param_name| type_param_subst[param_name])
            .collect();
        engine.pool_mut().applied(name, &type_args)
    }
}

/// Infer type for a struct literal with spread syntax: `Point { ...base, x: 10 }`.
fn infer_struct_spread(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    name: Name,
    fields: ori_ir::StructLitFieldRange,
    span: Span,
) -> Idx {
    let struct_lit_fields = arena.get_struct_lit_fields(fields);

    // Step 1: Look up the struct type in the registry
    let Some(type_registry) = engine.type_registry() else {
        for field in struct_lit_fields {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id);
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    infer_expr(engine, arena, *expr);
                }
            }
        }
        return Idx::ERROR;
    };

    let Some(entry) = type_registry.get_by_name(name).cloned() else {
        // Unknown type name — find similar type names for suggestions
        let similar = find_similar_type_names(engine, type_registry, name);
        engine.push_error(TypeCheckError::unknown_ident(span, name, similar));
        for field in struct_lit_fields {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id);
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    infer_expr(engine, arena, *expr);
                }
            }
        }
        return Idx::ERROR;
    };

    // Extract scalar fields before moving kind out of the owned entry
    let entry_idx = entry.idx;
    let type_params = entry.type_params;
    let TypeKind::Struct(struct_def) = entry.kind else {
        engine.push_error(TypeCheckError::not_a_struct(span, name));
        for field in struct_lit_fields {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id);
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    infer_expr(engine, arena, *expr);
                }
            }
        }
        return Idx::ERROR;
    };

    // Step 2: Create fresh type variables for generic params
    let type_param_subst: FxHashMap<Name, Idx> = type_params
        .iter()
        .map(|&param_name| (param_name, engine.fresh_var()))
        .collect();

    // Step 3: Build expected field types with substitution
    let expected_fields: Vec<(Name, Idx)> = struct_def
        .fields
        .iter()
        .map(|f| {
            let ty = if type_param_subst.is_empty() {
                f.ty
            } else {
                substitute_named_types(engine.pool_mut(), f.ty, &type_param_subst)
            };
            (f.name, ty)
        })
        .collect();

    let expected_map: FxHashMap<Name, Idx> = expected_fields.iter().copied().collect();

    // Build the target type for spread unification
    let target_type = if type_param_subst.is_empty() {
        engine.pool_mut().named(name)
    } else {
        let type_args: Vec<Idx> = type_params
            .iter()
            .map(|param_name| type_param_subst[param_name])
            .collect();
        engine.pool_mut().applied(name, &type_args)
    };

    // Step 4: Check provided fields
    let mut provided_fields: FxHashSet<Name> =
        FxHashSet::with_capacity_and_hasher(struct_lit_fields.len(), rustc_hash::FxBuildHasher);
    let mut has_spread = false;

    for field in struct_lit_fields {
        match field {
            ori_ir::StructLitField::Field(init) => {
                if !provided_fields.insert(init.name) {
                    engine.push_error(TypeCheckError::duplicate_field(init.span, name, init.name));
                    continue;
                }

                if let Some(&expected_ty) = expected_map.get(&init.name) {
                    let actual_ty = if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id)
                    } else {
                        infer_ident(engine, init.name, init.span)
                    };
                    let _ = engine.unify_types(actual_ty, expected_ty);
                } else {
                    let available: Vec<Name> = expected_fields.iter().map(|(n, _)| *n).collect();
                    engine.push_error(TypeCheckError::undefined_field(
                        init.span, entry_idx, init.name, available,
                    ));
                    if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id);
                    }
                }
            }
            ori_ir::StructLitField::Spread { expr, .. } => {
                has_spread = true;
                let spread_ty = infer_expr(engine, arena, *expr);
                // Spread expression must be the same struct type
                let _ = engine.unify_types(spread_ty, target_type);
            }
        }
    }

    // Step 5: Check for missing fields (only if no spread)
    if !has_spread {
        let missing: Vec<Name> = expected_fields
            .iter()
            .filter(|(field_name, _)| !provided_fields.contains(field_name))
            .map(|(field_name, _)| *field_name)
            .collect();

        if !missing.is_empty() {
            engine.push_error(TypeCheckError::missing_fields(span, name, missing));
        }
    }

    target_type
}

/// Substitute Named types that match type parameter names with replacement types.
///
/// Walks the pool type structure recursively. For a generic struct `type Box<T> = { value: T }`,
/// field type `Named(T)` is replaced with the fresh type variable allocated for T.
fn substitute_named_types(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    match pool.tag(ty) {
        Tag::Named => {
            let name = pool.named_name(ty);
            if let Some(&replacement) = subst.get(&name) {
                replacement
            } else {
                ty
            }
        }

        Tag::List => {
            let elem = pool.list_elem(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.list(new_elem)
            }
        }

        Tag::Option => {
            let elem = pool.option_inner(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.option(new_elem)
            }
        }

        Tag::Set => {
            let elem = pool.set_elem(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.set(new_elem)
            }
        }

        Tag::Channel => {
            let elem = pool.channel_elem(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.channel(new_elem)
            }
        }

        Tag::Range => {
            let elem = pool.range_elem(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.range(new_elem)
            }
        }

        Tag::Map => {
            let key = pool.map_key(ty);
            let value = pool.map_value(ty);
            let new_key = substitute_named_types(pool, key, subst);
            let new_value = substitute_named_types(pool, value, subst);
            if new_key == key && new_value == value {
                ty
            } else {
                pool.map(new_key, new_value)
            }
        }

        Tag::Result => {
            let ok = pool.result_ok(ty);
            let err = pool.result_err(ty);
            let new_ok = substitute_named_types(pool, ok, subst);
            let new_err = substitute_named_types(pool, err, subst);
            if new_ok == ok && new_err == err {
                ty
            } else {
                pool.result(new_ok, new_err)
            }
        }

        Tag::Function => {
            let params = pool.function_params(ty);
            let ret = pool.function_return(ty);

            let mut changed = false;
            let new_params: Vec<Idx> = params
                .iter()
                .map(|&p| {
                    let new_p = substitute_named_types(pool, p, subst);
                    if new_p != p {
                        changed = true;
                    }
                    new_p
                })
                .collect();

            let new_ret = substitute_named_types(pool, ret, subst);
            if new_ret != ret {
                changed = true;
            }

            if changed {
                pool.function(&new_params, new_ret)
            } else {
                ty
            }
        }

        Tag::Tuple => {
            let elems = pool.tuple_elems(ty);

            let mut changed = false;
            let new_elems: Vec<Idx> = elems
                .iter()
                .map(|&e| {
                    let new_e = substitute_named_types(pool, e, subst);
                    if new_e != e {
                        changed = true;
                    }
                    new_e
                })
                .collect();

            if changed {
                pool.tuple(&new_elems)
            } else {
                ty
            }
        }

        Tag::Applied => {
            let app_name = pool.applied_name(ty);
            let args = pool.applied_args(ty);

            let mut changed = false;
            let new_args: Vec<Idx> = args
                .iter()
                .map(|&a| {
                    let new_a = substitute_named_types(pool, a, subst);
                    if new_a != a {
                        changed = true;
                    }
                    new_a
                })
                .collect();

            if changed {
                pool.applied(app_name, &new_args)
            } else {
                ty
            }
        }

        // Primitives, Error, Var, BoundVar, RigidVar, etc. — no substitution needed
        _ => ty,
    }
}

// Option/Result constructors
fn infer_ok(engine: &mut InferEngine<'_>, arena: &ExprArena, inner: ExprId, _span: Span) -> Idx {
    let ok_ty = if inner.is_present() {
        infer_expr(engine, arena, inner)
    } else {
        Idx::UNIT
    };
    let err_ty = engine.fresh_var();
    engine.infer_result(ok_ty, err_ty)
}

fn infer_err(engine: &mut InferEngine<'_>, arena: &ExprArena, inner: ExprId, _span: Span) -> Idx {
    let err_ty = if inner.is_present() {
        infer_expr(engine, arena, inner)
    } else {
        Idx::UNIT
    };
    let ok_ty = engine.fresh_var();
    engine.infer_result(ok_ty, err_ty)
}

fn infer_some(engine: &mut InferEngine<'_>, arena: &ExprArena, inner: ExprId, _span: Span) -> Idx {
    let inner_ty = infer_expr(engine, arena, inner);
    engine.infer_option(inner_ty)
}

fn infer_none(engine: &mut InferEngine<'_>) -> Idx {
    let inner_ty = engine.fresh_var();
    engine.infer_option(inner_ty)
}

// Control flow expression stubs
fn infer_break(engine: &mut InferEngine<'_>, arena: &ExprArena, value: ExprId, _span: Span) -> Idx {
    // Infer the break value's type (unit if no value)
    let value_ty = if value.is_present() {
        infer_expr(engine, arena, value)
    } else {
        Idx::UNIT
    };

    // Unify with the enclosing loop's break type variable
    if let Some(loop_break_ty) = engine.current_loop_break_type() {
        let _ = engine.unify_types(value_ty, loop_break_ty);
    }

    // Break itself is a diverging expression (control transfers to loop exit)
    Idx::NEVER
}

fn infer_continue(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _value: ExprId,
    _span: Span,
) -> Idx {
    Idx::NEVER
}

fn infer_try(engine: &mut InferEngine<'_>, arena: &ExprArena, inner: ExprId, span: Span) -> Idx {
    let inner_ty = infer_expr(engine, arena, inner);
    let resolved = engine.resolve(inner_ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Option => {
            // Option<T>? -> T (propagates None)
            engine.pool().option_inner(resolved)
        }
        Tag::Result => {
            // Result<T, E>? -> T (propagates Err)
            engine.pool().result_ok(resolved)
        }
        _ => {
            engine.push_error(TypeCheckError::try_requires_option_or_result(
                span, resolved,
            ));
            Idx::ERROR
        }
    }
}

fn infer_await(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _inner: ExprId,
    _span: Span,
) -> Idx {
    // TODO: Implement await inference
    Idx::ERROR
}

fn infer_cast(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
    ty: &ori_ir::ParsedType,
    fallible: bool,
    _span: Span,
) -> Idx {
    // Infer the expression type (for validation, though we don't check cast validity here)
    let _expr_ty = infer_expr(engine, arena, expr);

    // Resolve the target type
    let target_ty = resolve_parsed_type(engine, arena, ty);

    // Fallible casts return Option<T>, infallible return T directly
    if fallible {
        engine.pool_mut().option(target_ty)
    } else {
        target_ty
    }
}

fn infer_assign(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    target: ExprId,
    value: ExprId,
    _span: Span,
) -> Idx {
    let target_ty = infer_expr(engine, arena, target);
    let value_ty = infer_expr(engine, arena, value);

    let expected = Expected {
        ty: target_ty,
        origin: ExpectedOrigin::Context {
            span: arena.get_expr(target).span,
            kind: ContextKind::Assignment,
        },
    };
    let _ = engine.check_type(value_ty, &expected, arena.get_expr(value).span);

    Idx::UNIT
}

fn infer_with_capability(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    capability: Name,
    provider: ExprId,
    body: ExprId,
    _span: Span,
) -> Idx {
    // Infer provider type (validates the provider expression)
    let provider_ty = infer_expr(engine, arena, provider);

    // Bind the capability name in a child scope so the body can
    // reference it as an identifier (e.g., `with Http = mock in Http`).
    engine.enter_scope();
    engine.env_mut().bind(capability, provider_ty);

    // Provide the capability for the duration of the body.
    // This makes calls to functions `uses <capability>` valid within.
    let body_ty =
        engine.with_provided_capability(capability, |engine| infer_expr(engine, arena, body));

    engine.exit_scope();
    body_ty
}

// Call inference stubs
fn infer_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: ExprId,
    args: ori_ir::ExprRange,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let arg_ids = arena.get_expr_list(args);

    // Extract function name for signature lookup
    let func_name_id = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(name) | ExprKind::Ident(name) => Some(*name),
        _ => None,
    };

    // Look up required_params from function signature if available
    let required_params = func_name_id
        .and_then(|n| engine.get_signature(n))
        .map_or(params.len(), |sig| sig.required_params);

    // Check arity: allow fewer args if defaults fill the gap
    if arg_ids.len() < required_params || arg_ids.len() > params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            params.len(),
            arg_ids.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Idx::ERROR;
    }

    // Validate capability requirements
    check_call_capabilities(engine, func_name_id, span);

    // Check each provided argument
    for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
                kind: ContextKind::FunctionArgument {
                    func_name: None,
                    arg_index: i,
                    param_name: None,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg_id);
        let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg_id).span);
    }

    ret
}

fn infer_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: ExprId,
    args: ori_ir::CallArgRange,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let call_args = arena.get_call_args(args);

    // Extract function name for error messages and signature lookup
    let func_name_id = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(name) | ExprKind::Ident(name) => Some(*name),
        _ => None,
    };

    // Look up required_params from function signature if available
    let required_params = func_name_id
        .and_then(|n| engine.get_signature(n))
        .map_or(params.len(), |sig| sig.required_params);

    // Check arity: allow fewer args if defaults fill the gap
    if call_args.len() < required_params || call_args.len() > params.len() {
        // Allocate func name string only on the error path
        let func_name = func_name_id.and_then(|n| engine.lookup_name(n).map(String::from));
        if let Some(name) = func_name {
            engine.push_error(TypeCheckError::arity_mismatch_named(
                span,
                name,
                params.len(),
                call_args.len(),
            ));
        } else {
            engine.push_error(TypeCheckError::arity_mismatch(
                span,
                params.len(),
                call_args.len(),
                crate::ArityMismatchKind::Function,
            ));
        }
        return Idx::ERROR;
    }

    // Validate capability requirements
    check_call_capabilities(engine, func_name_id, span);

    // Check each argument type by position
    for (i, (arg, &param_ty)) in call_args.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
                kind: ContextKind::FunctionArgument {
                    func_name: func_name_id,
                    arg_index: i,
                    param_name: arg.name,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg.value);
        let _ = engine.check_type(arg_ty, &expected, arg.span);
    }

    // Validate where-clause constraints after argument type-checking.
    // At this point, generic type variables have been unified with concrete types.
    if let Some(func_name) = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(n) | ExprKind::Ident(n) => Some(*n),
        _ => None,
    } {
        check_where_clauses(engine, func_name, &params, span);
    }

    ret
}

/// Validate that required capabilities are available at a call site.
///
/// Looks up the callee's signature to find its `uses` capabilities,
/// then checks each one against the caller's declared + provided capabilities.
/// Emits `E2014 MissingCapability` for each missing capability.
fn check_call_capabilities(engine: &mut InferEngine<'_>, func_name: Option<Name>, span: Span) {
    let Some(name) = func_name else { return };
    let Some(sig) = engine.get_signature(name) else {
        return;
    };

    // Collect missing capabilities during immutable borrow
    let missing: Vec<Name> = sig
        .capabilities
        .iter()
        .copied()
        .filter(|&cap| !engine.has_capability(cap))
        .collect();

    if missing.is_empty() {
        return;
    }

    // Push errors in a separate mutable pass
    let available = engine.available_capabilities();
    for cap in missing {
        tracing::debug!(?cap, "missing capability at call site");
        engine.push_error(TypeCheckError::missing_capability(span, cap, &available));
    }
}

/// Validate where-clause constraints for a generic function call.
///
/// After argument type-checking has unified generic type variables with concrete
/// types, this checks constraints like `where C.Item: Eq` by:
/// 1. Resolving the concrete type for the generic param
/// 2. Finding the trait impl that defines the associated type
/// 3. Looking up the projected type
/// 4. Checking the projected type satisfies the required trait bound
///
/// Uses a three-phase approach to satisfy the borrow checker:
/// 1. Mutable phase: resolve types and create pool entries
/// 2. Immutable phase: check trait registry and collect violations
/// 3. Mutable phase: push collected errors
fn check_where_clauses(
    engine: &mut InferEngine<'_>,
    func_name: Name,
    params: &[Idx],
    call_span: Span,
) {
    struct PreparedCheck {
        concrete_type: Idx,
        projection: Option<Name>,
        bound_entries: Vec<(Name, Idx)>,
        trait_bound_entries: Vec<Idx>,
    }

    let Some(sig) = engine.get_signature(func_name) else {
        return;
    };

    if sig.where_clauses.is_empty() {
        return;
    }

    // Extract only the fields we need, avoiding a full FunctionSig clone
    let where_clauses = sig.where_clauses.clone();
    let type_params = sig.type_params.clone();
    let type_param_bounds = sig.type_param_bounds.clone();
    let generic_param_mapping = sig.generic_param_mapping.clone();

    // Phase 1 (mutable): Resolve concrete types and create named Idx entries

    let mut prepared = Vec::new();

    for wc in &where_clauses {
        let Some(tp_idx) = type_params.iter().position(|&n| n == wc.param) else {
            continue;
        };
        let Some(Some(param_idx)) = generic_param_mapping.get(tp_idx) else {
            continue;
        };
        let Some(&instantiated_param) = params.get(*param_idx) else {
            continue;
        };
        let concrete_type = engine.resolve(instantiated_param);
        if concrete_type == Idx::ERROR {
            continue;
        }

        // Pre-create named Idx for each bound (needs &mut pool)
        let bound_entries: Vec<(Name, Idx)> = wc
            .bounds
            .iter()
            .map(|&name| (name, engine.pool_mut().named(name)))
            .collect();

        // Pre-create named Idx for type param bounds (for projection lookup)
        let tp_bounds = type_param_bounds.get(tp_idx).cloned().unwrap_or_default();
        let trait_bound_entries: Vec<Idx> = tp_bounds
            .iter()
            .map(|&name| engine.pool_mut().named(name))
            .collect();

        prepared.push(PreparedCheck {
            concrete_type,
            projection: wc.projection,
            bound_entries,
            trait_bound_entries,
        });
    }

    // Phase 2 (immutable): Check trait registry and collect error messages
    let errors = {
        let Some(trait_registry) = engine.trait_registry() else {
            return;
        };
        let pool = engine.pool();

        let mut errors: Vec<String> = Vec::new();

        for check in &prepared {
            if let Some(projection) = check.projection {
                // Where-clause with projection: `where C.Item: Eq`
                for &trait_idx in &check.trait_bound_entries {
                    let Some((_, impl_entry)) =
                        trait_registry.find_impl(trait_idx, check.concrete_type)
                    else {
                        continue;
                    };
                    let Some(&projected_type) = impl_entry.assoc_types.get(&projection) else {
                        continue;
                    };
                    for &(bound_name, bound_idx) in &check.bound_entries {
                        let bound_str = engine.lookup_name(bound_name).unwrap_or("");
                        if !trait_registry.has_impl(bound_idx, projected_type)
                            && !type_satisfies_trait(projected_type, bound_str, pool)
                        {
                            errors.push(format!("does not satisfy trait bound `{bound_str}`",));
                        }
                    }
                }
            } else {
                // Direct bound: `where T: Clone`
                for &(bound_name, bound_idx) in &check.bound_entries {
                    let bound_str = engine.lookup_name(bound_name).unwrap_or("");
                    if !trait_registry.has_impl(bound_idx, check.concrete_type)
                        && !type_satisfies_trait(check.concrete_type, bound_str, pool)
                    {
                        errors.push(format!("does not satisfy trait bound `{bound_str}`",));
                    }
                }
            }
        }

        errors
    };

    // Phase 3 (mutable): Push collected errors
    for msg in errors {
        engine.push_error(TypeCheckError::unsatisfied_bound(call_span, msg));
    }
}

/// Check if a type inherently satisfies a trait without needing an explicit impl.
///
/// Mirrors V1's `primitive_implements_trait()` from `bound_checking.rs`.
/// Primitive and built-in types have known trait implementations that don't
/// require explicit `impl` blocks in the trait registry.
fn primitive_satisfies_trait(ty: Idx, trait_name: &str) -> bool {
    // Trait sets for each primitive type, matching V1's const arrays.
    const INT_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "FloorDiv",
        "Rem",
        "Neg",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    const FLOAT_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Default",
        "Printable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Neg",
    ];
    const BOOL_TRAITS: &[&str] = &["Eq", "Clone", "Hashable", "Default", "Printable", "Not"];
    const STR_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Len",
        "IsEmpty",
        "Add",
    ];
    const CHAR_TRAITS: &[&str] = &["Eq", "Comparable", "Clone", "Hashable", "Printable"];
    const BYTE_TRAITS: &[&str] = &[
        "Eq",
        "Clone",
        "Hashable",
        "Printable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    const UNIT_TRAITS: &[&str] = &["Eq", "Clone", "Default"];
    const DURATION_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "Neg",
    ];
    const SIZE_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
    ];
    const ORDERING_TRAITS: &[&str] = &["Eq", "Clone", "Printable"];

    // Check primitive types by Idx constant
    if ty == Idx::INT {
        return INT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::FLOAT {
        return FLOAT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::BOOL {
        return BOOL_TRAITS.contains(&trait_name);
    }
    if ty == Idx::STR {
        return STR_TRAITS.contains(&trait_name);
    }
    if ty == Idx::CHAR {
        return CHAR_TRAITS.contains(&trait_name);
    }
    if ty == Idx::BYTE {
        return BYTE_TRAITS.contains(&trait_name);
    }
    if ty == Idx::UNIT {
        return UNIT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::DURATION {
        return DURATION_TRAITS.contains(&trait_name);
    }
    if ty == Idx::SIZE {
        return SIZE_TRAITS.contains(&trait_name);
    }
    if ty == Idx::ORDERING {
        return ORDERING_TRAITS.contains(&trait_name);
    }

    false
}

/// Extended trait satisfaction check that also handles compound types via Pool tags.
///
/// This extends `primitive_satisfies_trait` to handle List, Map, Option, Result,
/// Tuple, Set, and Range — types that aren't simple Idx constants but can be
/// identified by their Pool tag.
fn type_satisfies_trait(ty: Idx, trait_name: &str, pool: &Pool) -> bool {
    const COLLECTION_TRAITS: &[&str] = &["Clone", "Eq", "Len", "IsEmpty"];
    const WRAPPER_TRAITS: &[&str] = &["Clone", "Eq", "Default"];
    const RESULT_TRAITS: &[&str] = &["Clone", "Eq"];

    // First check primitives (no pool access needed)
    if primitive_satisfies_trait(ty, trait_name) {
        return true;
    }

    // Then check compound types by tag

    match pool.tag(ty) {
        Tag::List | Tag::Map | Tag::Set => COLLECTION_TRAITS.contains(&trait_name),
        Tag::Option => WRAPPER_TRAITS.contains(&trait_name),
        Tag::Result | Tag::Tuple => RESULT_TRAITS.contains(&trait_name),
        Tag::Range => trait_name == "Len",
        _ => false,
    }
}

/// Infer the type of a method call expression: `receiver.method(args)`.
///
/// Resolution priority:
/// 1. Built-in methods on primitives/collections (len, `is_empty`, first, etc.)
/// 2. User-defined inherent methods (from `impl Type { ... }`)
/// 3. User-defined trait methods (from `impl Trait for Type { ... }`)
///
/// For unresolved type variables, returns a fresh variable to defer resolution.
fn infer_method_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    args: ori_ir::ExprRange,
    span: Span,
) -> Idx {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(receiver_ty);

    // Propagate errors silently, but still infer args
    if resolved == Idx::ERROR {
        for &arg_id in arena.get_expr_list(args) {
            infer_expr(engine, arena, arg_id);
        }
        return Idx::ERROR;
    }

    // If receiver is a scheme, instantiate it to get the concrete type
    let resolved = if engine.pool().tag(resolved) == Tag::Scheme {
        engine.instantiate(resolved)
    } else {
        resolved
    };

    // For unresolved type variables, infer args and return fresh var
    let tag = engine.pool().tag(resolved);
    if tag == Tag::Var {
        for &arg_id in arena.get_expr_list(args) {
            infer_expr(engine, arena, arg_id);
        }
        return engine.pool_mut().fresh_var();
    }

    // Resolve method name to an owned string for built-in lookup.
    // String::from is needed to end the immutable engine borrow before the
    // mutable resolve_builtin_method call.
    let method_str = engine.lookup_name(method).map(String::from);

    // 1. Try built-in method resolution
    if let Some(ref name_str) = method_str {
        if let Some(ret) = resolve_builtin_method(engine, resolved, tag, name_str) {
            // Infer arguments (built-in methods don't have formal param types yet)
            for &arg_id in arena.get_expr_list(args) {
                infer_expr(engine, arena, arg_id);
            }
            return ret;
        }
    }

    // 2. Try user-defined method resolution via TraitRegistry
    if let Some(ret) = resolve_impl_method(engine, arena, resolved, method, args, span) {
        return ret;
    }

    // 3. No method found — silently return ERROR to preserve backward compatibility.
    // Once impl method registration is complete, this should report an error instead.
    for &arg_id in arena.get_expr_list(args) {
        infer_expr(engine, arena, arg_id);
    }
    Idx::ERROR
}

/// Infer the type of a named-argument method call: `receiver.method(name: value)`.
fn infer_method_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    args: ori_ir::CallArgRange,
    span: Span,
) -> Idx {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(receiver_ty);

    // Propagate errors silently, but still infer args
    if resolved == Idx::ERROR {
        for arg in arena.get_call_args(args) {
            infer_expr(engine, arena, arg.value);
        }
        return Idx::ERROR;
    }

    // If receiver is a scheme, instantiate it
    let resolved = if engine.pool().tag(resolved) == Tag::Scheme {
        engine.instantiate(resolved)
    } else {
        resolved
    };

    // For unresolved type variables, infer args and return fresh var
    let tag = engine.pool().tag(resolved);
    if tag == Tag::Var {
        for arg in arena.get_call_args(args) {
            infer_expr(engine, arena, arg.value);
        }
        return engine.pool_mut().fresh_var();
    }

    // Resolve method name to an owned string for built-in lookup.
    // String::from is needed to end the immutable engine borrow before the
    // mutable resolve_builtin_method call.
    let method_str = engine.lookup_name(method).map(String::from);

    // 1. Try built-in method resolution
    if let Some(ref name_str) = method_str {
        if let Some(ret) = resolve_builtin_method(engine, resolved, tag, name_str) {
            for arg in arena.get_call_args(args) {
                infer_expr(engine, arena, arg.value);
            }
            return ret;
        }
    }

    // 2. Try user-defined method resolution via TraitRegistry
    if let Some(ret) = resolve_impl_method_named(engine, arena, resolved, method, args, span) {
        return ret;
    }

    // 3. No method found — silently return ERROR to preserve backward compatibility.
    for arg in arena.get_call_args(args) {
        infer_expr(engine, arena, arg.value);
    }
    Idx::ERROR
}

/// Resolve a built-in method call on a known type tag.
///
/// Returns `Some(return_type)` if the method is a known built-in,
/// `None` if the method is not recognized for this type tag.
fn resolve_builtin_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,
) -> Option<Idx> {
    match tag {
        Tag::List => resolve_list_method(engine, receiver_ty, method_name),
        Tag::Option => resolve_option_method(engine, receiver_ty, method_name),
        Tag::Result => resolve_result_method(engine, receiver_ty, method_name),
        Tag::Map => resolve_map_method(engine, receiver_ty, method_name),
        Tag::Set => resolve_set_method(engine, receiver_ty, method_name),
        Tag::Str => resolve_str_method(engine, method_name),
        Tag::Int => resolve_int_method(method_name),
        Tag::Float => resolve_float_method(method_name),
        Tag::Duration => resolve_duration_method(method_name),
        Tag::Size => resolve_size_method(method_name),
        Tag::Channel => resolve_channel_method(engine, receiver_ty, method_name),
        Tag::Range => resolve_range_method(engine, receiver_ty, method_name),
        Tag::Named | Tag::Applied => resolve_named_type_method(engine, receiver_ty, method_name),
        Tag::Bool => resolve_bool_method(method_name),
        Tag::Byte => resolve_byte_method(method_name),
        Tag::Char => resolve_char_method(method_name),
        Tag::Ordering => resolve_ordering_method(method_name),
        Tag::Tuple => resolve_tuple_method(engine, receiver_ty, method_name),
        _ => None,
    }
}

fn resolve_list_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().list_elem(receiver_ty);
    match method {
        "len" | "count" => Some(Idx::INT),
        "is_empty" | "contains" => Some(Idx::BOOL),
        "first" | "last" | "pop" | "get" => Some(engine.pool_mut().option(elem)),
        "reverse" | "sort" | "sorted" | "unique" | "flatten" | "push" | "append" | "prepend" => {
            Some(receiver_ty)
        }
        "join" => Some(Idx::STR),
        "enumerate" => {
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            Some(engine.pool_mut().list(pair))
        }
        "zip" => {
            // zip takes another list and returns list of tuples
            // Without knowing the other list's element type, return fresh var
            let other_elem = engine.pool_mut().fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            Some(engine.pool_mut().list(pair))
        }
        "map" | "filter" | "flat_map" | "find" | "any" | "all" | "fold" | "reduce" | "for_each"
        | "take" | "skip" | "take_while" | "skip_while" | "chunk" | "window" | "min" | "max"
        | "sum" | "product" | "min_by" | "max_by" | "sort_by" | "group_by" | "partition" => {
            // Higher-order methods — return type depends on closure argument.
            // For now return fresh var; proper HO method inference is a follow-up.
            Some(engine.pool_mut().fresh_var())
        }
        _ => None,
    }
}

fn resolve_option_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let inner = engine.pool().option_inner(receiver_ty);
    match method {
        "is_some" | "is_none" => Some(Idx::BOOL),
        "unwrap" | "expect" | "unwrap_or" => Some(inner),
        "map" | "and_then" | "flat_map" | "filter" | "or_else" => {
            Some(engine.pool_mut().fresh_var())
        }
        "or" => Some(receiver_ty),
        _ => None,
    }
}

fn resolve_result_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let ok_ty = engine.pool().result_ok(receiver_ty);
    let err_ty = engine.pool().result_err(receiver_ty);
    match method {
        "is_ok" | "is_err" => Some(Idx::BOOL),
        "unwrap" | "expect" | "unwrap_or" => Some(ok_ty),
        "unwrap_err" | "expect_err" => Some(err_ty),
        "ok" => Some(engine.pool_mut().option(ok_ty)),
        "err" => Some(engine.pool_mut().option(err_ty)),
        "map" | "map_err" | "and_then" | "or_else" => Some(engine.pool_mut().fresh_var()),
        _ => None,
    }
}

fn resolve_map_method(engine: &mut InferEngine<'_>, receiver_ty: Idx, method: &str) -> Option<Idx> {
    let key_ty = engine.pool().map_key(receiver_ty);
    let value_ty = engine.pool().map_value(receiver_ty);
    match method {
        "len" => Some(Idx::INT),
        "is_empty" | "contains_key" | "contains" => Some(Idx::BOOL),
        "get" => Some(engine.pool_mut().option(value_ty)),
        "keys" => Some(engine.pool_mut().list(key_ty)),
        "values" => Some(engine.pool_mut().list(value_ty)),
        "entries" => {
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            Some(engine.pool_mut().list(pair))
        }
        "insert" | "remove" | "update" | "merge" => Some(receiver_ty),
        _ => None,
    }
}

fn resolve_set_method(engine: &mut InferEngine<'_>, receiver_ty: Idx, method: &str) -> Option<Idx> {
    let elem = engine.pool().set_elem(receiver_ty);
    match method {
        "len" => Some(Idx::INT),
        "is_empty" | "contains" => Some(Idx::BOOL),
        "insert" | "remove" | "union" | "intersection" | "difference" => Some(receiver_ty),
        "to_list" => Some(engine.pool_mut().list(elem)),
        _ => None,
    }
}

fn resolve_str_method(engine: &mut InferEngine<'_>, method: &str) -> Option<Idx> {
    match method {
        "len" | "byte_len" => Some(Idx::INT),
        "is_empty" | "starts_with" | "ends_with" | "contains" => Some(Idx::BOOL),
        "to_upper" | "to_lower" | "trim" | "trim_start" | "trim_end" | "replace" | "repeat"
        | "pad_start" | "pad_end" | "slice" | "substring" => Some(Idx::STR),
        "chars" => Some(engine.pool_mut().list(Idx::CHAR)),
        "bytes" => Some(engine.pool_mut().list(Idx::BYTE)),
        "split" | "lines" => Some(engine.pool_mut().list(Idx::STR)),
        "index_of" | "last_index_of" | "to_int" | "parse_int" => {
            Some(engine.pool_mut().option(Idx::INT))
        }
        "to_float" | "parse_float" => Some(engine.pool_mut().option(Idx::FLOAT)),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_int_method(method: &str) -> Option<Idx> {
    match method {
        "abs" | "min" | "max" | "clamp" | "pow" | "signum" => Some(Idx::INT),
        "to_float" => Some(Idx::FLOAT),
        "to_str" => Some(Idx::STR),
        "to_byte" => Some(Idx::BYTE),
        "is_positive" | "is_negative" | "is_zero" | "is_even" | "is_odd" => Some(Idx::BOOL),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_float_method(method: &str) -> Option<Idx> {
    match method {
        "abs" | "sqrt" | "cbrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
        | "ln" | "log2" | "log10" | "exp" | "pow" | "min" | "max" | "clamp" | "signum" => {
            Some(Idx::FLOAT)
        }
        "floor" | "ceil" | "round" | "trunc" | "to_int" => Some(Idx::INT),
        "to_str" => Some(Idx::STR),
        "is_nan" | "is_infinite" | "is_finite" | "is_normal" | "is_positive" | "is_negative"
        | "is_zero" => Some(Idx::BOOL),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_duration_method(method: &str) -> Option<Idx> {
    match method {
        // Instance methods
        "to_seconds" | "to_millis" | "to_micros" | "to_nanos" | "as_seconds" | "as_millis"
        | "as_micros" | "as_nanos" => Some(Idx::FLOAT),
        "to_str" | "format" => Some(Idx::STR),
        "abs" | "from_nanoseconds" | "from_microseconds" | "from_milliseconds" | "from_seconds"
        | "from_minutes" | "from_hours" | "from_nanos" | "from_micros" | "from_millis" | "zero" => {
            Some(Idx::DURATION)
        }
        "is_zero" | "is_negative" | "is_positive" => Some(Idx::BOOL),
        "nanoseconds" | "microseconds" | "milliseconds" | "seconds" | "minutes" | "hours" => {
            Some(Idx::INT)
        }
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_size_method(method: &str) -> Option<Idx> {
    match method {
        // Instance methods
        "to_bytes" | "as_bytes" | "to_kb" | "to_mb" | "to_gb" | "to_tb" => Some(Idx::INT),
        "to_str" | "format" => Some(Idx::STR),
        "is_zero" => Some(Idx::BOOL),
        // Associated functions (static constructors): Size.from_bytes(b: 100)
        "from_bytes" | "from_kilobytes" | "from_megabytes" | "from_gigabytes"
        | "from_terabytes" | "from_kb" | "from_mb" | "from_gb" | "from_tb" | "zero" => {
            Some(Idx::SIZE)
        }
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_channel_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().channel_elem(receiver_ty);
    match method {
        "send" | "close" => Some(Idx::UNIT),
        "recv" | "receive" | "try_recv" | "try_receive" => Some(engine.pool_mut().option(elem)),
        "is_closed" | "is_empty" => Some(Idx::BOOL),
        "len" => Some(Idx::INT),
        _ => None,
    }
}

fn resolve_range_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().range_elem(receiver_ty);
    match method {
        "len" | "count" => Some(Idx::INT),
        "is_empty" | "contains" => Some(Idx::BOOL),
        "to_list" | "collect" => Some(engine.pool_mut().list(elem)),
        "step_by" => Some(receiver_ty),
        _ => None,
    }
}

/// Resolve methods on Named/Applied types (user-defined structs, enums, newtypes).
///
/// For newtypes, supports `.unwrap()` to extract the inner value.
fn resolve_named_type_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method_name: &str,
) -> Option<Idx> {
    // Check type registry for newtype unwrap
    if method_name == "unwrap" || method_name == "inner" || method_name == "value" {
        if let Some(type_registry) = engine.type_registry() {
            if let Some(entry) = type_registry.get_by_idx(receiver_ty) {
                if let crate::TypeKind::Newtype { underlying } = &entry.kind {
                    return Some(*underlying);
                }
            }
        }
    }

    // Common methods on any user-defined type
    match method_name {
        "to_str" => Some(Idx::STR),
        _ => None,
    }
}

/// Ordering methods: predicates, reverse, equality, and trait methods.
fn resolve_ordering_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "is_less"
        | "is_equal"
        | "is_greater"
        | "is_less_or_equal"
        | "is_greater_or_equal"
        | "equals" => Some(Idx::BOOL),
        "reverse" | "clone" | "compare" => Some(Idx::ORDERING),
        "hash" => Some(Idx::INT),
        "to_str" | "debug" => Some(Idx::STR),
        _ => None,
    }
}

fn resolve_bool_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_str" => Some(Idx::STR),
        "to_int" => Some(Idx::INT),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_byte_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_int" => Some(Idx::INT),
        "to_char" => Some(Idx::CHAR),
        "to_str" => Some(Idx::STR),
        "is_ascii" | "is_ascii_digit" | "is_ascii_alpha" | "is_ascii_whitespace" => Some(Idx::BOOL),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_char_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_str" => Some(Idx::STR),
        "to_int" | "to_byte" => Some(Idx::INT),
        "is_digit" | "is_alpha" | "is_whitespace" | "is_uppercase" | "is_lowercase"
        | "is_ascii" => Some(Idx::BOOL),
        "to_upper" | "to_lower" => Some(Idx::CHAR),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_tuple_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method_name: &str,
) -> Option<Idx> {
    match method_name {
        "len" => Some(Idx::INT),
        "to_list" => {
            // Only works if all elements are the same type
            let count = engine.pool().tuple_elem_count(receiver_ty);
            if count > 0 {
                let first = engine.pool().tuple_elem(receiver_ty, 0);
                Some(engine.pool_mut().list(first))
            } else {
                Some(engine.pool_mut().list(Idx::UNIT))
            }
        }
        _ => None,
    }
}

/// Try to resolve a method call through the `TraitRegistry` (user-defined impls).
///
/// Returns `Some(return_type)` if the method was found, `None` otherwise.
fn resolve_impl_method(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver_ty: Idx,
    method: Name,
    args: ori_ir::ExprRange,
    span: Span,
) -> Option<Idx> {
    // Look up the method signature and self-ness from user-defined impls
    let (sig_ty, has_self) = {
        let trait_registry = engine.trait_registry()?;
        let lookup = trait_registry.lookup_method(receiver_ty, method)?;
        (lookup.method().signature, lookup.method().has_self)
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        // Signature exists but isn't a proper function type
        for &arg_id in arena.get_expr_list(args) {
            infer_expr(engine, arena, arg_id);
        }
        return Some(Idx::ERROR);
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    // For instance methods (has_self), skip the first `self` param.
    // For associated functions, use all params.
    let skip = usize::from(has_self);
    let method_params = &params[skip..];

    let arg_ids = arena.get_expr_list(args);

    // Check arity
    if arg_ids.len() != method_params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            method_params.len(),
            arg_ids.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Some(Idx::ERROR);
    }

    // Check each argument
    for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(method_params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span,
                kind: ContextKind::FunctionArgument {
                    func_name: None,
                    arg_index: i,
                    param_name: None,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg_id);
        let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg_id).span);
    }

    Some(ret)
}

/// Try to resolve a named-argument method call through the `TraitRegistry`.
fn resolve_impl_method_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver_ty: Idx,
    method: Name,
    args: ori_ir::CallArgRange,
    span: Span,
) -> Option<Idx> {
    let (sig_ty, has_self) = {
        let trait_registry = engine.trait_registry()?;
        let lookup = trait_registry.lookup_method(receiver_ty, method)?;
        (lookup.method().signature, lookup.method().has_self)
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        for arg in arena.get_call_args(args) {
            infer_expr(engine, arena, arg.value);
        }
        return Some(Idx::ERROR);
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    // For instance methods (has_self), skip the first `self` param.
    // For associated functions, use all params.
    let skip = usize::from(has_self);
    let method_params = &params[skip..];

    let call_args = arena.get_call_args(args);

    if call_args.len() != method_params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            method_params.len(),
            call_args.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Some(Idx::ERROR);
    }

    for (i, (arg, &param_ty)) in call_args.iter().zip(method_params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span,
                kind: ContextKind::FunctionArgument {
                    func_name: None,
                    arg_index: i,
                    param_name: arg.name,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg.value);
        let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg.value).span);
    }

    Some(ret)
}

/// Infer the type of a field access expression: `receiver.field`.
///
/// Handles:
/// - Tuple field access by numeric index (`.0`, `.1`, etc.)
/// - Struct field access by name (`.x`, `.name`)
/// - Generic struct field access with type parameter substitution
/// - Module namespace access (`Counter.new`)
///
/// For unresolved type variables, returns a fresh variable to defer resolution.
/// For error types, propagates ERROR silently. For types where field access
/// is genuinely unsupported (primitives, functions, etc.), returns ERROR
/// without reporting an error — method resolution may handle these separately.
fn infer_field(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    field: Name,
    span: Span,
) -> Idx {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(receiver_ty);

    match engine.pool().tag(resolved) {
        Tag::Tuple => {
            // Tuple field access: `.0`, `.1`, etc.
            let Some(field_str) = engine.lookup_name(field) else {
                return Idx::ERROR;
            };
            if let Ok(index) = field_str.parse::<usize>() {
                let elems = engine.pool().tuple_elems(resolved);
                if index < elems.len() {
                    elems[index]
                } else {
                    engine.push_error(TypeCheckError::undefined_field(
                        span,
                        resolved,
                        field,
                        vec![],
                    ));
                    Idx::ERROR
                }
            } else {
                engine.push_error(TypeCheckError::undefined_field(
                    span,
                    resolved,
                    field,
                    vec![],
                ));
                Idx::ERROR
            }
        }

        Tag::Named => {
            let type_name = engine.pool().named_name(resolved);
            infer_struct_field(engine, type_name, None, field, span)
        }

        Tag::Applied => {
            let type_name = engine.pool().applied_name(resolved);
            let type_args = engine.pool().applied_args(resolved);
            infer_struct_field(engine, type_name, Some(type_args), field, span)
        }

        // Unresolved type variable — return fresh var to defer resolution
        // (following V1 pattern: the actual field type will be resolved later)
        Tag::Var => engine.fresh_var(),

        // Error, or unsupported types for field access — return ERROR silently.
        // Don't report errors here since module namespace access
        // (e.g., `Counter.new`) and other patterns may reach this point
        // and would require method/namespace resolution to diagnose properly.
        _ => Idx::ERROR,
    }
}

/// Look up a field on a struct type, with optional type argument substitution.
///
/// For types not in the registry or non-struct types, returns ERROR silently.
/// This avoids false positives for imported types or types that aren't yet
/// fully registered (e.g., from other modules).
///
/// Only reports errors when the struct is known but the field doesn't exist —
/// a case where we can give a definitive, useful error message.
fn infer_struct_field(
    engine: &mut InferEngine<'_>,
    type_name: Name,
    type_args: Option<Vec<Idx>>,
    field: Name,
    span: Span,
) -> Idx {
    let Some(type_registry) = engine.type_registry() else {
        return Idx::ERROR;
    };

    let Some(entry) = type_registry.get_by_name(type_name).cloned() else {
        return Idx::ERROR; // Not registered — likely imported
    };

    let TypeKind::Struct(struct_def) = &entry.kind else {
        return Idx::ERROR; // Enum/newtype/alias — not a struct
    };

    // Find the field
    let Some(field_def) = struct_def.fields.iter().find(|f| f.name == field).cloned() else {
        let available: Vec<Name> = struct_def.fields.iter().map(|f| f.name).collect();
        let receiver_idx = engine.pool_mut().named(type_name);
        engine.push_error(TypeCheckError::undefined_field(
            span,
            receiver_idx,
            field,
            available,
        ));
        return Idx::ERROR;
    };

    // Substitute type parameters for generic structs
    if let Some(args) = type_args {
        if !entry.type_params.is_empty() && args.len() == entry.type_params.len() {
            let subst: FxHashMap<Name, Idx> = entry
                .type_params
                .iter()
                .zip(args.iter())
                .map(|(&param, &arg)| (param, arg))
                .collect();
            return substitute_named_types(engine.pool_mut(), field_def.ty, &subst);
        }
    }

    field_def.ty
}

/// Look up all field types for a struct, with optional generic substitution.
///
/// Returns a `Name → Idx` map of field types if the type is a known struct
/// in the registry. Returns `None` for unknown or non-struct types.
fn lookup_struct_field_types(
    engine: &mut InferEngine<'_>,
    type_name: Name,
    type_args: Option<&[Idx]>,
) -> Option<FxHashMap<Name, Idx>> {
    let type_registry = engine.type_registry()?;
    let entry = type_registry.get_by_name(type_name)?.clone();

    let TypeKind::Struct(struct_def) = &entry.kind else {
        return None;
    };

    let subst: Option<FxHashMap<Name, Idx>> = type_args.and_then(|args| {
        if !entry.type_params.is_empty() && args.len() == entry.type_params.len() {
            Some(
                entry
                    .type_params
                    .iter()
                    .zip(args.iter())
                    .map(|(&param, &arg)| (param, arg))
                    .collect(),
            )
        } else {
            None
        }
    });

    let mut field_types = FxHashMap::default();
    for field in &struct_def.fields {
        let ty = if let Some(ref subst) = subst {
            substitute_named_types(engine.pool_mut(), field.ty, subst)
        } else {
            field.ty
        };
        field_types.insert(field.name, ty);
    }
    Some(field_types)
}

/// Infer the type of an index access expression (e.g., `list[0]`, `map["key"]`).
///
/// Validates that the receiver is indexable and the index type matches:
/// - `[T]` indexed by `int` returns `T`
/// - `Map<K, V>` indexed by `K` returns `Option<V>`
/// - `str` indexed by `int` returns `str`
///
/// Returns ERROR silently for non-indexable types to avoid false positives
/// when the receiver type is unknown or not yet fully resolved.
fn infer_index(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    index: ExprId,
    _span: Span,
) -> Idx {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let index_ty = infer_expr(engine, arena, index);
    let resolved = engine.resolve(receiver_ty);

    match engine.pool().tag(resolved) {
        Tag::List => {
            let elem_ty = engine.pool().list_elem(resolved);
            let _ = engine.unify_types(index_ty, Idx::INT);
            elem_ty
        }

        Tag::Map => {
            let key_ty = engine.pool().map_key(resolved);
            let value_ty = engine.pool().map_value(resolved);
            let _ = engine.unify_types(index_ty, key_ty);
            // Map indexing returns Option<V>
            engine.pool_mut().option(value_ty)
        }

        Tag::Str => {
            let _ = engine.unify_types(index_ty, Idx::INT);
            Idx::STR
        }

        // Unresolved type variable — return fresh var
        Tag::Var => engine.fresh_var(),

        // Error, non-indexable, or unknown types — return ERROR silently.
        // Avoids false positives for types that may support custom indexing
        // or types not yet fully resolved in inference.
        _ => Idx::ERROR,
    }
}

/// Infer type for a `function_seq` expression (run, try, match, for).
///
/// `FunctionSeq` represents sequential expressions where order matters:
/// - **Run**: `run(let x = a, let y = b, result)` - sequential bindings
/// - **Try**: `try(let x = fallible()?, result)` - auto-unwrap `Result`/`Option`
/// - **Match**: `match(scrutinee, Pattern -> expr, ...)` - pattern matching
/// - **`ForPattern`**: `for(over: items, match: Pattern -> expr, default: fallback)`
fn infer_function_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func_seq: &ori_ir::FunctionSeq,
    span: Span,
) -> Idx {
    use ori_ir::FunctionSeq;

    match func_seq {
        FunctionSeq::Run {
            pre_checks,
            bindings,
            result,
            post_checks,
            ..
        } => infer_run_seq(engine, arena, *pre_checks, *bindings, *result, *post_checks),

        FunctionSeq::Try {
            bindings, result, ..
        } => infer_try_seq(engine, arena, *bindings, *result, span),

        FunctionSeq::Match {
            scrutinee,
            arms,
            span: match_span,
        } => {
            // Delegate to existing match inference
            infer_match(engine, arena, *scrutinee, *arms, *match_span)
        }

        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => infer_for_pattern(engine, arena, *over, *map, arm, *default, span),
    }
}

/// Infer type for `run(pre_check: ..., let x = a, result, post_check: ...)`.
///
/// Creates a new scope, validates pre-checks, processes bindings sequentially,
/// infers the result type, then validates post-checks against the result type.
fn infer_run_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pre_checks: ori_ir::CheckRange,
    bindings: ori_ir::SeqBindingRange,
    result: ExprId,
    post_checks: ori_ir::CheckRange,
) -> Idx {
    engine.enter_scope();

    // Pre-checks: each condition must be bool, each message must be str.
    // Pre-checks execute before any bindings, so they only see the enclosing scope.
    infer_pre_checks(engine, arena, pre_checks);

    // Process each binding in sequence
    let seq_bindings = arena.get_seq_bindings(bindings);
    for binding in seq_bindings {
        infer_seq_binding(engine, arena, binding, false);
    }

    // Infer the result expression
    let result_ty = infer_expr(engine, arena, result);

    // Post-checks: each must be a lambda `(result_type) -> bool`, message must be str.
    // Post-checks can see bindings from the run body.
    infer_post_checks(engine, arena, post_checks, result_ty);

    engine.exit_scope();

    result_ty
}

/// Type-check pre-check expressions in a `run()` block.
///
/// Each `pre_check: condition` must have type `bool`.
/// Each optional message (`| "msg"`) must have type `str`.
fn infer_pre_checks(engine: &mut InferEngine<'_>, arena: &ExprArena, checks: ori_ir::CheckRange) {
    let checks = arena.get_checks(checks);
    for check in checks {
        // Condition must be bool
        let cond_ty = infer_expr(engine, arena, check.expr);
        engine.push_context(ContextKind::PreCheck);
        let expected = Expected {
            ty: Idx::BOOL,
            origin: ExpectedOrigin::NoExpectation,
        };
        let _ = engine.check_type(cond_ty, &expected, arena.get_expr(check.expr).span);
        engine.pop_context();

        // Message must be str (if present)
        if let Some(msg) = check.message {
            let msg_ty = infer_expr(engine, arena, msg);
            let expected = Expected {
                ty: Idx::STR,
                origin: ExpectedOrigin::NoExpectation,
            };
            let _ = engine.check_type(msg_ty, &expected, arena.get_expr(msg).span);
        }
    }
}

/// Type-check post-check expressions in a `run()` block.
///
/// Each `post_check: r -> condition` must be a lambda from `result_type` to `bool`.
/// Each optional message (`| "msg"`) must have type `str`.
fn infer_post_checks(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    checks: ori_ir::CheckRange,
    result_ty: Idx,
) {
    let checks = arena.get_checks(checks);
    for check in checks {
        // Post-check expression must be fn(result_ty) -> bool
        let check_ty = infer_expr(engine, arena, check.expr);
        engine.push_context(ContextKind::PostCheck);
        let expected_fn = engine.pool_mut().function1(result_ty, Idx::BOOL);
        let expected = Expected {
            ty: expected_fn,
            origin: ExpectedOrigin::NoExpectation,
        };
        let _ = engine.check_type(check_ty, &expected, arena.get_expr(check.expr).span);
        engine.pop_context();

        // Message must be str (if present)
        if let Some(msg) = check.message {
            let msg_ty = infer_expr(engine, arena, msg);
            let expected = Expected {
                ty: Idx::STR,
                origin: ExpectedOrigin::NoExpectation,
            };
            let _ = engine.check_type(msg_ty, &expected, arena.get_expr(msg).span);
        }
    }
}

/// Infer type for `try(let x = fallible()?, result)`.
///
/// Like run, but auto-unwraps Result/Option types in let bindings.
/// The entire expression returns a Result or Option wrapping the result.
fn infer_try_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    bindings: ori_ir::SeqBindingRange,
    result: ExprId,
    span: Span,
) -> Idx {
    // Enter a new scope for the try block
    engine.enter_scope();

    // Track the error type for Result propagation
    let mut error_ty: Option<Idx> = None;

    // Process each binding in sequence (with unwrapping)
    let seq_bindings = arena.get_seq_bindings(bindings);
    for binding in seq_bindings {
        if let ori_ir::SeqBinding::Let { value, .. } = binding {
            // Infer the value type first
            let value_ty = infer_expr(engine, arena, *value);
            let resolved = engine.resolve(value_ty);
            let tag = engine.pool().tag(resolved);

            // Track error type from Result
            if tag == Tag::Result && error_ty.is_none() {
                error_ty = Some(engine.pool().result_err(resolved));
            }
        }
        // Process binding with try-unwrapping enabled
        infer_seq_binding(engine, arena, binding, true);
    }

    // Infer the result expression
    let result_ty = infer_expr(engine, arena, result);

    // Exit scope
    engine.exit_scope();

    // The result type depends on what was in the bindings
    // If we saw Results, wrap the result in Result<T, E>
    // If we saw Options, wrap in Option<T>
    // Otherwise, return as-is (though this shouldn't happen in valid try blocks)
    if let Some(err_ty) = error_ty {
        engine.pool_mut().result(result_ty, err_ty)
    } else {
        // Check if result is already wrapped
        let resolved = engine.resolve(result_ty);
        let tag = engine.pool().tag(resolved);
        if tag == Tag::Result || tag == Tag::Option {
            result_ty
        } else {
            // Default to Result with a fresh error type for proper try semantics
            let _ = span; // Available for future error reporting
            let err_var = engine.fresh_var();
            engine.pool_mut().result(result_ty, err_var)
        }
    }
}

/// Infer type for `for(over: items, [map: transform,] match: Pattern -> expr, default: fallback)`.
///
/// Iterates over a collection, applies optional map, finds first matching pattern,
/// or returns default.
fn infer_for_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    over: ExprId,
    map: Option<ExprId>,
    arm: &ori_ir::MatchArm,
    default: ExprId,
    _span: Span,
) -> Idx {
    // Infer the iterable type
    let over_ty = infer_expr(engine, arena, over);
    let resolved_over = engine.resolve(over_ty);

    // Extract element type from collection
    let elem_ty = match engine.pool().tag(resolved_over) {
        Tag::List => engine.pool().list_elem(resolved_over),
        Tag::Set => engine.pool().set_elem(resolved_over),
        Tag::Range => engine.pool().range_elem(resolved_over),
        Tag::Map => engine.pool().map_key(resolved_over),
        _ => engine.fresh_var(), // Unknown iterable, create type var
    };

    // Apply optional map function
    let scrutinee_ty = if let Some(map_fn) = map {
        let map_fn_ty = infer_expr(engine, arena, map_fn);
        let resolved_map = engine.resolve(map_fn_ty);

        if engine.pool().tag(resolved_map) == Tag::Function {
            // Map function return type becomes the new element type
            engine.pool().function_return(resolved_map)
        } else {
            // Not a function, just use elem_ty
            elem_ty
        }
    } else {
        elem_ty
    };

    // Enter scope for pattern bindings
    engine.enter_scope();

    // Check pattern against scrutinee type.
    // for-pattern arms don't have an ArmRange, use a sentinel key.
    check_match_pattern(
        engine,
        arena,
        &arm.pattern,
        scrutinee_ty,
        PatternKey::Arm(u32::MAX),
        arm.span,
    );

    // Check guard if present
    if let Some(guard_id) = arm.guard {
        engine.push_context(ContextKind::MatchArmGuard { arm_index: 0 });
        let guard_ty = infer_expr(engine, arena, guard_id);
        let _ = engine.unify_types(guard_ty, Idx::BOOL);
        engine.pop_context();
    }

    // Infer arm body
    let arm_ty = infer_expr(engine, arena, arm.body);

    // Exit scope
    engine.exit_scope();

    // Infer default expression
    let default_ty = infer_expr(engine, arena, default);

    // Arm and default must have same type
    let _ = engine.unify_types(arm_ty, default_ty);

    arm_ty
}

/// Process a sequential binding (let or statement).
///
/// If `try_unwrap` is true, auto-unwrap Result/Option in let bindings.
fn infer_seq_binding(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    binding: &ori_ir::SeqBinding,
    try_unwrap: bool,
) {
    use ori_ir::SeqBinding;

    match binding {
        SeqBinding::Let {
            pattern,
            ty,
            value,
            span,
            ..
        } => {
            let pat = arena.get_binding_pattern(*pattern);

            // Track error count for closure self-capture detection
            let binding_name = pattern_first_name(pat);
            let errors_before = engine.error_count();

            // Enter scope for let-polymorphism (allows generalization of lambdas)
            engine.enter_scope();

            // Handle type annotation if present, or generalize for let-polymorphism
            let final_ty = if ty.is_valid() {
                // With type annotation
                let parsed_ty = arena.get_parsed_type(*ty);
                let expected_ty = resolve_parsed_type(engine, arena, parsed_ty);

                if try_unwrap {
                    // For try blocks: infer, unwrap, then check against annotation
                    // e.g., `let x: int = succeed(42)` where succeed returns Result<int>
                    let init_ty = infer_expr(engine, arena, *value);
                    let unwrapped = unwrap_result_or_option(engine, init_ty);

                    let expected = Expected {
                        ty: expected_ty,
                        origin: ExpectedOrigin::Annotation {
                            name: pattern_first_name(pat).unwrap_or(Name::EMPTY),
                            span: *span,
                        },
                    };
                    let _ = engine.check_type(unwrapped, &expected, *span);
                    expected_ty
                } else {
                    // For run blocks: use bidirectional checking (allows literal coercion)
                    // e.g., `let x: byte = 65` coerces int literal to byte
                    let expected = Expected {
                        ty: expected_ty,
                        origin: ExpectedOrigin::Annotation {
                            name: pattern_first_name(pat).unwrap_or(Name::EMPTY),
                            span: *span,
                        },
                    };
                    let _init_ty = check_expr(engine, arena, *value, &expected, *span);
                    expected_ty
                }
            } else {
                // No annotation: infer the initializer type
                let init_ty = infer_expr(engine, arena, *value);

                // Detect closure self-capture: if the init is a lambda and any new
                // errors are UnknownIdent matching the binding name, rewrite them.
                // Example: `run(let f = () -> f, ...)` — f isn't yet in scope.
                if let Some(name) = binding_name {
                    if matches!(arena.get_expr(*value).kind, ExprKind::Lambda { .. }) {
                        engine.rewrite_self_capture_errors(name, errors_before);
                    }
                }

                // For try blocks, unwrap Result/Option
                let bound_ty = if try_unwrap {
                    unwrap_result_or_option(engine, init_ty)
                } else {
                    init_ty
                };

                // Generalize free type variables for let-polymorphism
                // This enables: `let id = x -> x, id(42), id("hello")`
                engine.generalize(bound_ty)
            };

            // Exit scope before binding (generalization happens at current rank)
            engine.exit_scope();

            // Bind pattern to type
            bind_pattern(engine, arena, pat, final_ty);
        }

        SeqBinding::Stmt { expr, .. } => {
            // Statement expression - evaluate for side effects
            infer_expr(engine, arena, *expr);
        }
    }
}

/// Unwrap Result<T, E> → T or Option<T> → T.
fn unwrap_result_or_option(engine: &mut InferEngine<'_>, ty: Idx) -> Idx {
    let resolved = engine.resolve(ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Result => engine.pool().result_ok(resolved),
        Tag::Option => engine.pool().option_inner(resolved),
        _ => ty, // Not wrapped, return as-is
    }
}

/// Bind a binding pattern to a type, introducing variables into scope.
#[expect(
    clippy::only_used_in_recursion,
    reason = "Arena is threaded through for recursive sub-pattern binding"
)]
fn bind_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::BindingPattern,
    ty: Idx,
) {
    use ori_ir::BindingPattern;

    match pattern {
        BindingPattern::Name { name, .. } => {
            engine.env_mut().bind(*name, ty);
        }

        BindingPattern::Tuple(patterns) => {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) == Tag::Tuple {
                let elem_types = engine.pool().tuple_elems(resolved);
                for (pat, elem_ty) in patterns.iter().zip(elem_types.iter()) {
                    bind_pattern(engine, arena, pat, *elem_ty);
                }
            } else {
                // Type mismatch - bind each to fresh var
                for pat in patterns {
                    let var = engine.fresh_var();
                    bind_pattern(engine, arena, pat, var);
                }
            }
        }

        BindingPattern::Struct { fields } => {
            let resolved = engine.resolve(ty);
            let field_type_map = match engine.pool().tag(resolved) {
                Tag::Named => {
                    let type_name = engine.pool().named_name(resolved);
                    lookup_struct_field_types(engine, type_name, None)
                }
                Tag::Applied => {
                    let type_name = engine.pool().applied_name(resolved);
                    let type_args = engine.pool().applied_args(resolved);
                    lookup_struct_field_types(engine, type_name, Some(&type_args))
                }
                _ => None,
            };

            for field in fields {
                let field_ty = field_type_map
                    .as_ref()
                    .and_then(|m| m.get(&field.name).copied())
                    .unwrap_or_else(|| engine.fresh_var());
                if let Some(sub_pat) = &field.pattern {
                    bind_pattern(engine, arena, sub_pat, field_ty);
                } else {
                    // Shorthand: { x } means { x: x }
                    engine.env_mut().bind(field.name, field_ty);
                }
            }
        }

        BindingPattern::List { elements, rest } => {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) == Tag::List {
                let elem_ty = engine.pool().list_elem(resolved);
                for pat in elements {
                    bind_pattern(engine, arena, pat, elem_ty);
                }
                if let Some(rest_name) = rest {
                    // Rest binding gets the full list type
                    engine.env_mut().bind(*rest_name, ty);
                }
            } else {
                // Type mismatch - bind each to fresh var
                for pat in elements {
                    let var = engine.fresh_var();
                    bind_pattern(engine, arena, pat, var);
                }
                if let Some(rest_name) = rest {
                    engine.env_mut().bind(*rest_name, ty);
                }
            }
        }

        BindingPattern::Wildcard => {
            // Wildcard binds nothing
        }
    }
}

/// Infer type for a `function_exp` expression (recurse, parallel, print, etc.).
///
/// `FunctionExp` represents named property expressions:
/// - **Print**: `print(value: expr)` → unit
/// - **Panic**: `panic(message: expr)` → never
/// - **Todo/Unreachable**: `todo(message?: expr)` → never
/// - **Catch**: `catch(try: expr, catch: expr)` → T
/// - **Recurse**: `recurse(condition: expr, base: expr, step: expr)` → T
/// - **Parallel/Spawn/Timeout/Cache/With**: Concurrency patterns
fn infer_function_exp(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func_exp: &ori_ir::FunctionExp,
) -> Idx {
    use ori_ir::FunctionExpKind;

    let props = arena.get_named_exprs(func_exp.props);

    match func_exp.kind {
        // === Simple built-ins ===
        FunctionExpKind::Print => {
            // print(value: expr) → unit
            // Evaluate the value (if present) for type checking
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::UNIT
        }

        FunctionExpKind::Panic => {
            // panic(message: expr) → never
            // Evaluate message for type checking
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        FunctionExpKind::Todo => {
            // todo(message?: expr) → never
            // Optional message
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        FunctionExpKind::Unreachable => {
            // unreachable(message?: expr) → never
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        // === Error handling ===
        FunctionExpKind::Catch => {
            // catch(expr: expression) → Result<T, str>
            infer_catch(engine, arena, props)
        }

        // === Recursion ===
        FunctionExpKind::Recurse => {
            // recurse(condition: expr, base: expr, step: expr)
            // Complex: step can reference `self` (the recursive function)
            infer_recurse(engine, arena, props)
        }

        // === Concurrency patterns ===
        FunctionExpKind::Parallel => {
            // parallel(tasks: [expr]) → [T]
            // Returns list of results from parallel execution
            infer_parallel(engine, arena, props)
        }

        FunctionExpKind::Spawn => {
            // spawn(task: expr) → Task<T>
            // Returns a handle to the spawned task
            infer_spawn(engine, arena, props)
        }

        FunctionExpKind::Timeout => {
            // timeout(duration: Duration, task: expr) → Option<T>
            // Returns Some(result) or None if timeout
            infer_timeout(engine, arena, props)
        }

        FunctionExpKind::Cache => {
            // cache(key: expr, op: expr, ttl: Duration) → T
            infer_cache(engine, arena, props)
        }

        FunctionExpKind::With => {
            // with(acquire: expr, action: expr, release: expr) → T
            infer_with(engine, arena, props)
        }

        // Channel constructors — stub: infer props, return fresh type var
        FunctionExpKind::Channel
        | FunctionExpKind::ChannelIn
        | FunctionExpKind::ChannelOut
        | FunctionExpKind::ChannelAll => {
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            engine.fresh_var()
        }
    }
}

/// Infer type for `catch(expr: expression)`.
///
/// Returns `Result<T, str>` where `T` is the type of the `expr` property.
fn infer_catch(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    let mut expr_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if engine.lookup_name(prop.name) == Some("expr") {
            expr_ty = Some(ty);
        }
    }

    let inner = expr_ty.unwrap_or_else(|| engine.fresh_var());
    engine.pool_mut().result(inner, Idx::STR)
}

/// Infer type for `recurse(condition: expr, base: expr, step: expr)`.
fn infer_recurse(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // The step expression needs access to `self` (the recursive function)
    // For now, we'll infer base and use that as the result type
    // Full implementation needs Section 07 (scoped bindings)

    let mut condition_ty = None;
    let mut base_ty = None;
    let mut step_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if condition_ty.is_none() {
            // condition should be bool
            condition_ty = Some(ty);
        } else if base_ty.is_none() {
            base_ty = Some(ty);
        } else if step_ty.is_none() {
            step_ty = Some(ty);
        }
    }

    // Condition must be bool
    if let Some(cond) = condition_ty {
        let _ = engine.unify_types(cond, Idx::BOOL);
    }

    // Base and step must have same type
    if let (Some(b), Some(s)) = (base_ty, step_ty) {
        let _ = engine.unify_types(b, s);
    }

    base_ty.unwrap_or_else(|| engine.fresh_var())
}

/// Infer type for `parallel(tasks: [expr])`.
fn infer_parallel(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Parallel takes a list of tasks and returns a list of results
    // For now, return [?a] with fresh variable
    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        // If it's a list, extract element type and wrap result
        let resolved = engine.resolve(ty);
        if engine.pool().tag(resolved) == Tag::List {
            let elem_ty = engine.pool().list_elem(resolved);
            return engine.pool_mut().list(elem_ty);
        }
    }

    let result_ty = engine.fresh_var();
    engine.pool_mut().list(result_ty)
}

/// Infer type for `spawn(task: expr)`.
fn infer_spawn(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Spawn returns a handle to the task
    // For now, return the task's result type wrapped in a fresh type
    // (Would need a Task<T> type in the pool)
    for prop in props {
        let _ = infer_expr(engine, arena, prop.value);
    }
    // TODO: Return proper Task<T> type when Task is added
    engine.fresh_var()
}

/// Infer type for `timeout(duration: Duration, task: expr)`.
fn infer_timeout(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Returns Option<T> where T is the task result
    let mut task_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        // Skip duration, capture task type (first non-duration property)
        if task_ty.is_none() {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) != Tag::Duration {
                task_ty = Some(ty);
            }
        }
        // If we already have a task type, just evaluate for type checking
    }

    let inner = task_ty.unwrap_or_else(|| engine.fresh_var());
    engine.pool_mut().option(inner)
}

/// Infer type for `cache(key: expr, op: expr, ttl: Duration)`.
fn infer_cache(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Returns the `op` expression's type.
    // Match on prop names to avoid positional fragility.
    let mut op_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if engine.lookup_name(prop.name) == Some("op") {
            op_ty = Some(ty);
        }
    }

    op_ty.unwrap_or_else(|| engine.fresh_var())
}

/// Infer type for `with(acquire: expr, action: expr, release: expr)`.
///
/// Returns the `action` expression's type.
fn infer_with(engine: &mut InferEngine<'_>, arena: &ExprArena, props: &[ori_ir::NamedExpr]) -> Idx {
    let mut action_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if engine.lookup_name(prop.name) == Some("action") {
            action_ty = Some(ty);
        }
    }

    action_ty.unwrap_or_else(|| engine.fresh_var())
}

// =============================================================================
// ParsedType → Idx Resolution
// =============================================================================

/// Resolve a `ParsedType` from the AST into a pool `Idx`.
///
/// This converts parsed type annotations into the pool representation.
/// The conversion is recursive for compound types (functions, containers, etc.).
///
/// # Type Mapping
///
/// | `ParsedType` | `Idx` |
/// |--------------|-------|
/// | `Primitive(TypeId::INT)` | `Idx::INT` |
/// | `Primitive(TypeId::UNIT)` | `Idx::UNIT` |
/// | `List(elem)` | `pool.list(resolve(elem))` |
/// | `Function { params, ret }` | `pool.function(...)` |
/// | `Named { name, args }` | lookup or fresh var |
/// | `Infer` | fresh variable |
/// | `SelfType` | fresh variable (TODO: context lookup) |
///
/// # Future Work
///
/// - Named type lookup requires `TypeRegistry` integration (section 07)
/// - `SelfType` requires trait/impl context
/// - `AssociatedType` requires projection support
pub fn resolve_parsed_type(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    parsed: &ParsedType,
) -> Idx {
    match parsed {
        // === Primitive Types ===
        ParsedType::Primitive(type_id) => resolve_type_id(engine, *type_id),

        // === Container Types ===
        ParsedType::List(elem_id) => {
            let elem = arena.get_parsed_type(*elem_id);
            let elem_ty = resolve_parsed_type(engine, arena, elem);
            engine.pool_mut().list(elem_ty)
        }

        ParsedType::FixedList { elem, capacity: _ } => {
            // Fixed lists are treated as regular lists for now
            // TODO: Add fixed list support when needed
            let elem_parsed = arena.get_parsed_type(*elem);
            let elem_ty = resolve_parsed_type(engine, arena, elem_parsed);
            engine.pool_mut().list(elem_ty)
        }

        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_ty = resolve_parsed_type(engine, arena, key_parsed);
            let value_ty = resolve_parsed_type(engine, arena, value_parsed);
            engine.pool_mut().map(key_ty, value_ty)
        }

        // === Tuple Types ===
        ParsedType::Tuple(elems) => {
            if elems.is_empty() {
                Idx::UNIT
            } else {
                let elem_types = resolve_parsed_type_list(engine, arena, *elems);
                engine.pool_mut().tuple(&elem_types)
            }
        }

        // === Function Types ===
        ParsedType::Function { params, ret } => {
            let param_types = resolve_parsed_type_list(engine, arena, *params);
            let ret_parsed = arena.get_parsed_type(*ret);
            let ret_ty = resolve_parsed_type(engine, arena, ret_parsed);
            engine.pool_mut().function(&param_types, ret_ty)
        }

        // === Named Types ===
        ParsedType::Named { name, type_args } => {
            // Resolve type arguments if present
            let resolved_args: Vec<Idx> = if type_args.is_empty() {
                Vec::new()
            } else {
                resolve_parsed_type_list(engine, arena, *type_args)
            };

            // Check for well-known generic types that have dedicated Pool tags.
            // Must use the correct Pool constructors to match types created during inference.
            if !resolved_args.is_empty() {
                if let Some(name_str) = engine.lookup_name(*name) {
                    match (name_str, resolved_args.len()) {
                        ("Option", 1) => return engine.pool_mut().option(resolved_args[0]),
                        ("Result", 2) => {
                            return engine.pool_mut().result(resolved_args[0], resolved_args[1]);
                        }
                        ("Set", 1) => return engine.pool_mut().set(resolved_args[0]),
                        ("Channel" | "Chan", 1) => {
                            return engine.pool_mut().channel(resolved_args[0]);
                        }
                        ("Range", 1) => return engine.pool_mut().range(resolved_args[0]),
                        _ => {
                            // User-defined generic: Applied type
                            return engine.pool_mut().applied(*name, &resolved_args);
                        }
                    }
                }
                // No interner — create Applied type with name and args
                return engine.pool_mut().applied(*name, &resolved_args);
            }

            // No type args — check for builtin primitive names
            if let Some(name_str) = engine.lookup_name(*name) {
                match name_str {
                    "int" => return Idx::INT,
                    "float" => return Idx::FLOAT,
                    "bool" => return Idx::BOOL,
                    "str" => return Idx::STR,
                    "char" => return Idx::CHAR,
                    "byte" => return Idx::BYTE,
                    "void" | "()" => return Idx::UNIT,
                    "never" | "Never" => return Idx::NEVER,
                    "duration" => return Idx::DURATION,
                    "size" => return Idx::SIZE,
                    "ordering" | "Ordering" => return Idx::ORDERING,
                    _ => {}
                }
            }

            // Check if it's a known user-defined type in the TypeRegistry
            if let Some(registry) = engine.type_registry() {
                if registry.get_by_name(*name).is_some() {
                    return engine.pool_mut().named(*name);
                }
            }

            // Check if it's bound in the current environment (type parameter or local)
            if let Some(ty) = engine.env().lookup(*name) {
                return engine.instantiate(ty);
            }

            // Unknown type — create a named var for inference
            engine.fresh_named_var(*name)
        }

        // === Inference Markers ===
        // Infer and ConstExpr both produce fresh variables (const eval not yet implemented).
        // Note: registration (check/registration.rs) uses Idx::ERROR for ConstExpr because
        // registration needs deterministic types. Inference can defer via fresh vars.
        ParsedType::Infer | ParsedType::ConstExpr(_) => engine.fresh_var(),

        ParsedType::SelfType => engine
            .impl_self_type()
            .unwrap_or_else(|| engine.fresh_var()),

        ParsedType::AssociatedType { base, assoc_name } => {
            let base_parsed = arena.get_parsed_type(*base);
            let base_ty = resolve_parsed_type(engine, arena, base_parsed);
            let resolved_base = engine.resolve(base_ty);

            // Search trait impls for the associated type
            if let Some(trait_registry) = engine.trait_registry() {
                for impl_entry in trait_registry.impls_for_type(resolved_base) {
                    if let Some(&assoc_ty) = impl_entry.assoc_types.get(assoc_name) {
                        return assoc_ty;
                    }
                }
            }

            // Not found — return fresh variable for deferred resolution
            engine.fresh_var()
        }

        ParsedType::TraitBounds(bounds) => {
            // Bounded trait object: Printable + Hashable
            // Resolve the first bound as the primary type for now;
            // full trait object dispatch will refine this later.
            let bound_ids = arena.get_parsed_type_list(*bounds);
            if let Some(&first_id) = bound_ids.first() {
                let first = arena.get_parsed_type(first_id);
                resolve_parsed_type(engine, arena, first)
            } else {
                engine.fresh_var()
            }
        }
    }
}

/// Resolve a list of parsed types into a vector of pool indices.
fn resolve_parsed_type_list(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    range: ParsedTypeRange,
) -> Vec<Idx> {
    let ids = arena.get_parsed_type_list(range);
    ids.iter()
        .map(|id| {
            let parsed = arena.get_parsed_type(*id);
            resolve_parsed_type(engine, arena, parsed)
        })
        .collect()
}

/// Resolve a `TypeId` primitive to an `Idx`.
///
/// Handles the mapping between `TypeId` constants (from `ori_ir`) and `Idx` constants.
///
/// # `TypeId` Overlap
///
/// `TypeId` and `Idx` now share the same index layout for primitives (0-11),
/// so this is an identity mapping. INFER (12) and `SELF_TYPE` (13) are markers
/// that become fresh inference variables.
fn resolve_type_id(engine: &mut InferEngine<'_>, type_id: TypeId) -> Idx {
    let raw = type_id.raw();
    if raw < TypeId::PRIMITIVE_COUNT {
        // Primitives 0-11 map by identity (TypeId and Idx share the same layout)
        Idx::from_raw(raw)
    } else {
        // INFER (12), SELF_TYPE (13), or unknown — create a fresh variable
        engine.fresh_var()
    }
}

#[cfg(test)]
mod tests;

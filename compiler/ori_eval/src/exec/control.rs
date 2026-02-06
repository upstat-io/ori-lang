//! Control flow evaluation (if/else, match, loops, break/continue).
//!
//! This module handles control flow constructs including:
//! - Conditionals (if/else)
//! - Match expressions and pattern matching
//! - For loops (imperative and yield)
//! - Loop expressions
//! - Break and continue
//!
//! # RAII Scope Safety
//!
//! Functions in this module that modify environment scope use `EnvScopeGuard`
//! to ensure the scope is popped even if evaluation panics. This provides
//! true panic safety through the RAII pattern.

use crate::{
    // Error factories
    cannot_assign_immutable,
    expected_list,
    expected_struct,
    expected_tuple,
    field_assignment_not_implemented,
    index_assignment_not_implemented,
    invalid_assignment_target,
    invalid_literal_pattern,
    list_pattern_too_long,
    missing_struct_field,
    non_exhaustive_match,
    tuple_pattern_mismatch,
    Environment,
    EvalError,
    EvalResult,
    Mutability,
    Value,
};
use ori_ir::{
    ArmRange, BindingPattern, ExprArena, ExprId, ExprKind, MatchPattern, Name, StmtKind, StmtRange,
    StringInterner,
};
use ori_types::{PatternKey, PatternResolution};

/// Look up a pattern resolution by key using binary search.
fn lookup_resolution(
    resolutions: &[(PatternKey, PatternResolution)],
    key: PatternKey,
) -> Option<PatternResolution> {
    resolutions
        .binary_search_by_key(&key, |(k, _)| *k)
        .ok()
        .map(|idx| resolutions[idx].1)
}

/// RAII guard for environment scope management.
///
/// Ensures `pop_scope()` is called when dropped, even during panic unwinding.
/// This provides panic-safe scope management for control flow constructs.
struct EnvScopeGuard<'a>(&'a mut Environment);

impl Drop for EnvScopeGuard<'_> {
    fn drop(&mut self) {
        self.0.pop_scope();
    }
}

impl<'a> EnvScopeGuard<'a> {
    /// Create a new scope guard, pushing a scope immediately.
    fn new(env: &'a mut Environment) -> Self {
        env.push_scope();
        EnvScopeGuard(env)
    }
}

impl std::ops::Deref for EnvScopeGuard<'_> {
    type Target = Environment;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::ops::DerefMut for EnvScopeGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

/// Evaluate an if/else expression.
pub fn eval_if<F>(
    cond: ExprId,
    then_branch: ExprId,
    else_branch: ExprId,
    mut eval_fn: F,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
{
    let cond_val = eval_fn(cond)?;
    if cond_val.is_truthy() {
        eval_fn(then_branch)
    } else if else_branch.is_present() {
        eval_fn(else_branch)
    } else {
        Ok(Value::Void)
    }
}

/// Bind a pattern to a value in the environment.
pub fn bind_pattern(
    pattern: &BindingPattern,
    value: Value,
    mutability: Mutability,
    env: &mut Environment,
) -> EvalResult {
    match pattern {
        BindingPattern::Name(name) => {
            env.define(*name, value, mutability);
            Ok(Value::Void)
        }
        BindingPattern::Wildcard => Ok(Value::Void),
        BindingPattern::Tuple(patterns) => {
            if let Value::Tuple(values) = value {
                if patterns.len() != values.len() {
                    return Err(tuple_pattern_mismatch());
                }
                for (pat, val) in patterns.iter().zip(values.iter()) {
                    bind_pattern(pat, val.clone(), mutability, env)?;
                }
                Ok(Value::Void)
            } else {
                Err(expected_tuple())
            }
        }
        BindingPattern::Struct { fields } => {
            if let Value::Struct(s) = value {
                for (field_name, binding) in fields {
                    if let Some(val) = s.get_field(*field_name) {
                        if let Some(nested_pattern) = binding {
                            bind_pattern(nested_pattern, val.clone(), mutability, env)?;
                        } else {
                            // Shorthand: let { x } = s -> binds x to s.x
                            env.define(*field_name, val.clone(), mutability);
                        }
                    } else {
                        return Err(missing_struct_field());
                    }
                }
                Ok(Value::Void)
            } else {
                Err(expected_struct())
            }
        }
        BindingPattern::List { elements, rest } => {
            if let Value::List(values) = value {
                if values.len() < elements.len() {
                    return Err(list_pattern_too_long());
                }
                for (pat, val) in elements.iter().zip(values.iter()) {
                    bind_pattern(pat, val.clone(), mutability, env)?;
                }
                if let Some(rest_name) = rest {
                    let rest_values: Vec<_> = values[elements.len()..].to_vec();
                    env.define(*rest_name, Value::list(rest_values), mutability);
                }
                Ok(Value::Void)
            } else {
                Err(expected_list())
            }
        }
    }
}

/// Try to match a pattern against a value, returning bindings if successful.
///
/// `arm_key` identifies this pattern for looking up type-checker resolutions.
/// `pattern_resolutions` contains the resolved disambiguation data from the type checker.
pub fn try_match(
    pattern: &MatchPattern,
    value: &Value,
    arena: &ExprArena,
    interner: &StringInterner,
    arm_key: Option<PatternKey>,
    pattern_resolutions: &[(PatternKey, PatternResolution)],
) -> Result<Option<Vec<(Name, Value)>>, EvalError> {
    match pattern {
        MatchPattern::Wildcard => Ok(Some(vec![])),

        MatchPattern::Binding(name) => {
            // Type-checker resolution is the primary authority for Binding disambiguation.
            // If resolved as UnitVariant: compare against scrutinee's variant name.
            if let Some(key) = arm_key {
                if let Some(PatternResolution::UnitVariant { .. }) =
                    lookup_resolution(pattern_resolutions, key)
                {
                    // This Binding was resolved as a unit variant constructor
                    if let Value::Variant {
                        variant_name: val_variant,
                        fields,
                        ..
                    } = value
                    {
                        if *name == *val_variant && fields.is_empty() {
                            return Ok(Some(vec![])); // Match, no bindings
                        }
                    }
                    return Ok(None); // Not this variant
                }
            }

            // Fallback: value-based variant disambiguation for cases where the type
            // checker lacks resolution (e.g., lambda parameters in higher-order methods
            // where the element type isn't propagated into the closure).
            if let Value::Variant {
                variant_name: val_variant,
                fields,
                ..
            } = value
            {
                if *name == *val_variant && fields.is_empty() {
                    return Ok(Some(vec![])); // Unit variant match
                }
                // Uppercase name that doesn't match → likely a different variant
                let pattern_name = interner.lookup(*name);
                if pattern_name.starts_with(char::is_uppercase) {
                    return Ok(None);
                }
            }

            // Normal binding — unconditionally binds the scrutinee value
            Ok(Some(vec![(*name, value.clone())]))
        }

        MatchPattern::Literal(expr_id) => {
            let lit_val = arena.get_expr(*expr_id);
            let lit = match &lit_val.kind {
                ExprKind::Int(n) => Value::int(*n),
                ExprKind::Float(bits) => Value::Float(f64::from_bits(*bits)),
                ExprKind::Bool(b) => Value::Bool(*b),
                ExprKind::String(s) => Value::string_static(interner.lookup_static(*s)),
                ExprKind::Char(c) => Value::Char(*c),
                _ => return Err(invalid_literal_pattern()),
            };
            if &lit == value {
                Ok(Some(vec![]))
            } else {
                Ok(None)
            }
        }

        MatchPattern::Variant { name, inner } => {
            let variant_name = interner.lookup(*name);
            let inner_patterns = arena.get_match_pattern_list(*inner);

            // Built-in Option/Result variants
            match (variant_name, value) {
                ("Some", Value::Some(v)) | ("Ok", Value::Ok(v)) | ("Err", Value::Err(v)) => {
                    return match inner_patterns.len() {
                        0 => Ok(Some(vec![])),
                        1 => try_match(
                            arena.get_match_pattern(inner_patterns[0]),
                            v.as_ref(),
                            arena,
                            interner,
                            None,
                            pattern_resolutions,
                        ),
                        _ => Ok(None), // These variants have only one field
                    };
                }
                ("None", Value::None) => {
                    return if inner_patterns.is_empty() {
                        Ok(Some(vec![]))
                    } else {
                        Ok(None)
                    };
                }
                _ => {}
            }

            // User-defined variants
            if let Value::Variant {
                variant_name: val_variant,
                fields,
                ..
            } = value
            {
                // Check if variant name matches
                if interner.lookup(*val_variant) != variant_name {
                    return Ok(None);
                }

                match (inner_patterns.len(), fields.len()) {
                    // No inner patterns: matches unit variants or acts as wildcard
                    (0, _) => Ok(Some(vec![])),
                    // Single pattern for single-field variant
                    (1, 1) => try_match(
                        arena.get_match_pattern(inner_patterns[0]),
                        &fields[0],
                        arena,
                        interner,
                        None,
                        pattern_resolutions,
                    ),
                    // Multiple patterns for multi-field variant
                    (n, m) if n == m => {
                        let mut all_bindings = Vec::with_capacity(inner_patterns.len());
                        for (pat_id, val) in inner_patterns.iter().zip(fields.iter()) {
                            match try_match(
                                arena.get_match_pattern(*pat_id),
                                val,
                                arena,
                                interner,
                                None,
                                pattern_resolutions,
                            )? {
                                Some(bindings) => all_bindings.extend(bindings),
                                None => return Ok(None),
                            }
                        }
                        Ok(Some(all_bindings))
                    }
                    // Pattern count doesn't match field count
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        }

        MatchPattern::Tuple(patterns) => {
            if let Value::Tuple(values) = value {
                let pattern_ids = arena.get_match_pattern_list(*patterns);
                if pattern_ids.len() != values.len() {
                    return Ok(None);
                }
                let mut all_bindings = Vec::with_capacity(pattern_ids.len());
                for (pat_id, val) in pattern_ids.iter().zip(values.iter()) {
                    match try_match(
                        arena.get_match_pattern(*pat_id),
                        val,
                        arena,
                        interner,
                        None,
                        pattern_resolutions,
                    )? {
                        Some(bindings) => all_bindings.extend(bindings),
                        None => return Ok(None),
                    }
                }
                Ok(Some(all_bindings))
            } else {
                Ok(None)
            }
        }

        MatchPattern::List { elements, rest } => {
            if let Value::List(values) = value {
                let element_ids = arena.get_match_pattern_list(*elements);
                if values.len() < element_ids.len() {
                    return Ok(None);
                }
                if rest.is_none() && values.len() != element_ids.len() {
                    return Ok(None);
                }
                // Pre-allocate for element bindings plus optional rest binding
                // Use saturating_add since overflow is impossible in practice (pattern lists
                // are bounded by source code size), but we need to satisfy arithmetic lint
                let capacity = element_ids
                    .len()
                    .saturating_add(usize::from(rest.is_some()));
                let mut all_bindings = Vec::with_capacity(capacity);
                for (pat_id, val) in element_ids.iter().zip(values.iter()) {
                    match try_match(
                        arena.get_match_pattern(*pat_id),
                        val,
                        arena,
                        interner,
                        None,
                        pattern_resolutions,
                    )? {
                        Some(bindings) => all_bindings.extend(bindings),
                        None => return Ok(None),
                    }
                }
                if let Some(rest_name) = rest {
                    let rest_values: Vec<_> = values[element_ids.len()..].to_vec();
                    all_bindings.push((*rest_name, Value::list(rest_values)));
                }
                Ok(Some(all_bindings))
            } else {
                Ok(None)
            }
        }

        MatchPattern::Or(patterns) => {
            for pat_id in arena.get_match_pattern_list(*patterns) {
                if let Some(bindings) = try_match(
                    arena.get_match_pattern(*pat_id),
                    value,
                    arena,
                    interner,
                    None,
                    pattern_resolutions,
                )? {
                    return Ok(Some(bindings));
                }
            }
            Ok(None)
        }

        MatchPattern::At { name, pattern } => {
            if let Some(mut bindings) = try_match(
                arena.get_match_pattern(*pattern),
                value,
                arena,
                interner,
                None,
                pattern_resolutions,
            )? {
                bindings.push((*name, value.clone()));
                Ok(Some(bindings))
            } else {
                Ok(None)
            }
        }

        MatchPattern::Struct { fields } => {
            if let Value::Struct(s) = value {
                let mut all_bindings = Vec::with_capacity(fields.len());
                for (field_name, inner_pat_id) in fields {
                    if let Some(field_val) = s.get_field(*field_name) {
                        if let Some(pat_id) = inner_pat_id {
                            match try_match(
                                arena.get_match_pattern(*pat_id),
                                field_val,
                                arena,
                                interner,
                                None,
                                pattern_resolutions,
                            )? {
                                Some(bindings) => all_bindings.extend(bindings),
                                None => return Ok(None),
                            }
                        } else {
                            // Shorthand: { x } binds x to the field value
                            all_bindings.push((*field_name, field_val.clone()));
                        }
                    } else {
                        return Ok(None);
                    }
                }
                Ok(Some(all_bindings))
            } else {
                Ok(None)
            }
        }

        MatchPattern::Range {
            start,
            end,
            inclusive,
        } => {
            if let Value::Int(n) = value {
                let n_raw = n.raw();
                let start_val = if let Some(s) = start {
                    let expr = arena.get_expr(*s);
                    if let ExprKind::Int(i) = expr.kind {
                        i
                    } else {
                        return Ok(None);
                    }
                } else {
                    i64::MIN
                };
                let end_val = if let Some(e) = end {
                    let expr = arena.get_expr(*e);
                    if let ExprKind::Int(i) = expr.kind {
                        i
                    } else {
                        return Ok(None);
                    }
                } else {
                    i64::MAX
                };

                let in_range = if *inclusive {
                    n_raw >= start_val && n_raw <= end_val
                } else {
                    n_raw >= start_val && n_raw < end_val
                };

                if in_range {
                    Ok(Some(vec![]))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
    }
}

/// Evaluate a match expression.
///
/// Uses RAII scope guard to ensure scope is popped even on panic.
#[expect(
    clippy::too_many_arguments,
    reason = "closure params (eval_fn, guard_fn) resist bundling into a struct"
)]
pub fn eval_match<EvalFn, GuardFn>(
    value: &Value,
    arms: ArmRange,
    arena: &ExprArena,
    interner: &StringInterner,
    env: &mut Environment,
    pattern_resolutions: &[(PatternKey, PatternResolution)],
    mut eval_fn: EvalFn,
    guard_fn: GuardFn,
) -> EvalResult
where
    EvalFn: FnMut(ExprId) -> EvalResult,
    GuardFn: Fn(ExprId, &mut Environment) -> EvalResult,
{
    let arm_range_start = arms.start;
    let arm_list = arena.get_arms(arms);

    for (i, arm) in arm_list.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::arithmetic_side_effects,
            reason = "arm count bounded by AST size, cannot overflow u32"
        )]
        let arm_key = PatternKey::Arm(arm_range_start + i as u32);

        // Try to match the pattern first
        if let Some(bindings) = try_match(
            &arm.pattern,
            value,
            arena,
            interner,
            Some(arm_key),
            pattern_resolutions,
        )? {
            // Use RAII guard for scope safety - scope is popped when guard drops
            let mut guard = EnvScopeGuard::new(env);
            for (name, val) in bindings {
                guard.define(name, val, Mutability::Immutable);
            }

            // Check if guard passes (if present) - bindings are now available
            if let Some(arm_guard) = arm.guard {
                let guard_result = guard_fn(arm_guard, &mut guard)?;
                if !guard_result.is_truthy() {
                    // Guard failed, scope will be popped when guard drops
                    drop(guard);
                    continue;
                }
            }

            // Evaluate body - scope popped when guard drops
            return eval_fn(arm.body);
        }
    }

    Err(non_exhaustive_match())
}

/// Result of a for loop iteration.
#[derive(Debug)]
pub enum LoopAction {
    /// Skip current iteration (continue without value)
    Continue,
    /// Substitute yielded value (continue with value in for...yield)
    ContinueWith(Value),
    /// Exit loop with value
    Break(Value),
    /// Propagate error
    Error(EvalError),
}

/// Evaluate a loop expression.
pub fn eval_loop<F>(body: ExprId, mut eval_fn: F) -> EvalResult
where
    F: FnMut(ExprId) -> Result<LoopAction, EvalError>,
{
    loop {
        match eval_fn(body)? {
            LoopAction::Continue | LoopAction::ContinueWith(_) => {}
            LoopAction::Break(val) => {
                return Ok(val);
            }
            LoopAction::Error(e) => {
                return Err(e);
            }
        }
    }
}

/// Convert an `EvalError` to a `LoopAction` using the `ControlFlow` enum.
///
/// Control flow signals (break/continue) are indicated by the `control_flow`
/// field on `EvalError`. Regular errors (where `control_flow` is `None`) are
/// propagated as `LoopAction::Error`.
pub fn to_loop_action(error: EvalError) -> LoopAction {
    use ori_patterns::ControlFlow;

    match error.control_flow {
        Some(ControlFlow::Continue(v)) if !matches!(v, Value::Void) => LoopAction::ContinueWith(v),
        Some(ControlFlow::Continue(_)) => LoopAction::Continue,
        Some(ControlFlow::Break(v)) => LoopAction::Break(v),
        None => LoopAction::Error(error),
    }
}

/// Evaluate an assignment target.
pub fn eval_assign(
    target: ExprId,
    value: Value,
    arena: &ExprArena,
    interner: &StringInterner,
    env: &mut Environment,
) -> EvalResult {
    let target_expr = arena.get_expr(target);
    match &target_expr.kind {
        ExprKind::Ident(name) => {
            env.assign(*name, value.clone()).map_err(|_| {
                let name_str = interner.lookup(*name);
                cannot_assign_immutable(name_str)
            })?;
            Ok(value)
        }
        ExprKind::Index { .. } => {
            // Assignment to index would require mutable values
            Err(index_assignment_not_implemented())
        }
        ExprKind::Field { .. } => {
            // Assignment to field would require mutable structs
            Err(field_assignment_not_implemented())
        }
        _ => Err(invalid_assignment_target()),
    }
}

/// Evaluate a block of statements.
///
/// Uses RAII scope guard to ensure scope is popped even on panic.
pub fn eval_block<F, G>(
    stmts: StmtRange,
    result: ExprId,
    arena: &ExprArena,
    env: &mut Environment,
    mut eval_fn: F,
    mut bind_fn: G,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
    G: FnMut(&BindingPattern, Value, bool) -> EvalResult,
{
    // Use RAII guard for scope safety - scope is popped when guard drops
    let _scope_guard = EnvScopeGuard::new(env);

    let stmt_list = arena.get_stmt_range(stmts);

    for stmt in stmt_list {
        match &stmt.kind {
            StmtKind::Expr(expr) => {
                eval_fn(*expr)?;
            }
            StmtKind::Let {
                pattern,
                init,
                mutable,
                ..
            } => {
                let pat = arena.get_binding_pattern(*pattern);
                let value = eval_fn(*init)?;
                bind_fn(pat, value, *mutable)?;
            }
        }
    }

    if result.is_present() {
        eval_fn(result)
    } else {
        Ok(Value::Void)
    }
    // scope popped when _scope_guard drops
}

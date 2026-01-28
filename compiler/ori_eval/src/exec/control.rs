//! Control flow evaluation (if/else, match, loops, break/continue).
//!
//! This module handles control flow constructs including:
//! - Conditionals (if/else)
//! - Match expressions and pattern matching
//! - For loops (imperative and yield)
//! - Loop expressions
//! - Break and continue

use crate::{
    // Error factories
    cannot_assign_immutable,
    expected_list,
    expected_struct,
    expected_tuple,
    field_assignment_not_implemented,
    for_requires_iterable,
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
    Value,
};
use ori_ir::{
    ArmRange, BindingPattern, ExprArena, ExprId, ExprKind, MatchPattern, Name, StmtKind, StmtRange,
    StringInterner,
};

/// Evaluate an if/else expression.
pub fn eval_if<F>(
    cond: ExprId,
    then_branch: ExprId,
    else_branch: Option<ExprId>,
    mut eval_fn: F,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
{
    let cond_val = eval_fn(cond)?;
    if cond_val.is_truthy() {
        eval_fn(then_branch)
    } else if let Some(else_expr) = else_branch {
        eval_fn(else_expr)
    } else {
        Ok(Value::Void)
    }
}

/// Bind a pattern to a value in the environment.
pub fn bind_pattern(
    pattern: &BindingPattern,
    value: Value,
    mutable: bool,
    env: &mut Environment,
) -> EvalResult {
    match pattern {
        BindingPattern::Name(name) => {
            env.define(*name, value, mutable);
            Ok(Value::Void)
        }
        BindingPattern::Wildcard => Ok(Value::Void),
        BindingPattern::Tuple(patterns) => {
            if let Value::Tuple(values) = value {
                if patterns.len() != values.len() {
                    return Err(tuple_pattern_mismatch());
                }
                for (pat, val) in patterns.iter().zip(values.iter()) {
                    bind_pattern(pat, val.clone(), mutable, env)?;
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
                            bind_pattern(nested_pattern, val.clone(), mutable, env)?;
                        } else {
                            // Shorthand: let { x } = s -> binds x to s.x
                            env.define(*field_name, val.clone(), mutable);
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
                    bind_pattern(pat, val.clone(), mutable, env)?;
                }
                if let Some(rest_name) = rest {
                    let rest_values: Vec<_> = values[elements.len()..].to_vec();
                    env.define(*rest_name, Value::list(rest_values), mutable);
                }
                Ok(Value::Void)
            } else {
                Err(expected_list())
            }
        }
    }
}

/// Try to match a pattern against a value, returning bindings if successful.
pub fn try_match(
    pattern: &MatchPattern,
    value: &Value,
    arena: &ExprArena,
    interner: &StringInterner,
) -> Result<Option<Vec<(Name, Value)>>, EvalError> {
    match pattern {
        MatchPattern::Wildcard => Ok(Some(vec![])),

        MatchPattern::Binding(name) => {
            // Check if this might be a unit variant pattern.
            // The parser can't distinguish `Pending` (variant) from `x` (binding)
            // without type context, so we check at match time.
            if let Value::Variant {
                variant_name: val_variant,
                fields,
                ..
            } = value
            {
                let pattern_name = interner.lookup(*name);
                let value_variant_name = interner.lookup(*val_variant);

                // Check if the pattern name is a known variant name by seeing if
                // it matches the type's variants. If the pattern name matches any
                // variant name of this type, treat it as a variant pattern.
                if pattern_name == value_variant_name {
                    // Pattern name matches variant name - treat as variant pattern
                    if fields.is_empty() {
                        // Unit variant match
                        return Ok(Some(vec![]));
                    }
                    // Variant has fields but pattern doesn't - no match
                    return Ok(None);
                }

                // Pattern name doesn't match this variant - check if it looks
                // like a variant name (starts with uppercase). If so, it's a
                // non-matching variant pattern.
                let first_char = pattern_name.chars().next().unwrap_or('a');
                if first_char.is_uppercase() {
                    // Likely a variant pattern that doesn't match - no match
                    return Ok(None);
                }
                // Lowercase name - treat as a regular binding
            }
            // Regular binding pattern
            Ok(Some(vec![(*name, value.clone())]))
        }

        MatchPattern::Literal(expr_id) => {
            let lit_val = arena.get_expr(*expr_id);
            let lit = match &lit_val.kind {
                ExprKind::Int(n) => Value::int(*n),
                ExprKind::Float(bits) => Value::Float(f64::from_bits(*bits)),
                ExprKind::Bool(b) => Value::Bool(*b),
                ExprKind::String(s) => Value::string(interner.lookup(*s).to_string()),
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

            // Built-in Option/Result variants
            match (variant_name, value) {
                ("Some", Value::Some(v)) | ("Ok", Value::Ok(v)) | ("Err", Value::Err(v)) => {
                    return match inner.len() {
                        0 => Ok(Some(vec![])),
                        1 => try_match(&inner[0], v.as_ref(), arena, interner),
                        _ => Ok(None), // These variants have only one field
                    };
                }
                ("None", Value::None) => {
                    return if inner.is_empty() {
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

                match (inner.len(), fields.len()) {
                    // No inner patterns: matches unit variants or acts as wildcard
                    (0, _) => Ok(Some(vec![])),
                    // Single pattern for single-field variant
                    (1, 1) => try_match(&inner[0], &fields[0], arena, interner),
                    // Multiple patterns for multi-field variant
                    (n, m) if n == m => {
                        let mut all_bindings = Vec::new();
                        for (pat, val) in inner.iter().zip(fields.iter()) {
                            match try_match(pat, val, arena, interner)? {
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
                if patterns.len() != values.len() {
                    return Ok(None);
                }
                let mut all_bindings = Vec::new();
                for (pat, val) in patterns.iter().zip(values.iter()) {
                    match try_match(pat, val, arena, interner)? {
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
                if values.len() < elements.len() {
                    return Ok(None);
                }
                if rest.is_none() && values.len() != elements.len() {
                    return Ok(None);
                }
                let mut all_bindings = Vec::new();
                for (pat, val) in elements.iter().zip(values.iter()) {
                    match try_match(pat, val, arena, interner)? {
                        Some(bindings) => all_bindings.extend(bindings),
                        None => return Ok(None),
                    }
                }
                if let Some(rest_name) = rest {
                    let rest_values: Vec<_> = values[elements.len()..].to_vec();
                    all_bindings.push((*rest_name, Value::list(rest_values)));
                }
                Ok(Some(all_bindings))
            } else {
                Ok(None)
            }
        }

        MatchPattern::Or(patterns) => {
            for pat in patterns {
                if let Some(bindings) = try_match(pat, value, arena, interner)? {
                    return Ok(Some(bindings));
                }
            }
            Ok(None)
        }

        MatchPattern::At { name, pattern } => {
            if let Some(mut bindings) = try_match(pattern, value, arena, interner)? {
                bindings.push((*name, value.clone()));
                Ok(Some(bindings))
            } else {
                Ok(None)
            }
        }

        MatchPattern::Struct { fields } => {
            if let Value::Struct(s) = value {
                let mut all_bindings = Vec::new();
                for (field_name, inner_pat) in fields {
                    if let Some(field_val) = s.get_field(*field_name) {
                        if let Some(pat) = inner_pat {
                            match try_match(pat, field_val, arena, interner)? {
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
pub fn eval_match<EvalFn, GuardFn>(
    value: &Value,
    arms: ArmRange,
    arena: &ExprArena,
    interner: &StringInterner,
    env: &mut Environment,
    mut eval_fn: EvalFn,
    guard_fn: GuardFn,
) -> EvalResult
where
    EvalFn: FnMut(ExprId) -> EvalResult,
    GuardFn: Fn(ExprId, &mut Environment) -> EvalResult,
{
    let arm_list = arena.get_arms(arms);

    for arm in arm_list {
        // Try to match the pattern first
        if let Some(bindings) = try_match(&arm.pattern, value, arena, interner)? {
            // Push scope with bindings
            env.push_scope();
            for (name, val) in bindings {
                env.define(name, val, false);
            }

            // Check if guard passes (if present) - bindings are now available
            if let Some(guard) = arm.guard {
                let guard_result = guard_fn(guard, env)?;
                if !guard_result.is_truthy() {
                    env.pop_scope();
                    continue;
                }
            }

            // Evaluate body
            let result = eval_fn(arm.body);
            env.pop_scope();
            return result;
        }
    }

    Err(non_exhaustive_match())
}

/// Result of a for loop iteration.
pub enum LoopAction {
    Continue,
    Break(Value),
    Error(EvalError),
}

/// Evaluate a for loop.
pub fn eval_for<F>(
    binding: Name,
    iter: Value,
    guard: Option<ExprId>,
    body: ExprId,
    is_yield: bool,
    env: &mut Environment,
    mut eval_body: F,
) -> EvalResult
where
    F: FnMut(ExprId, Option<ExprId>, &mut Environment) -> Result<(Value, LoopAction), EvalError>,
{
    let items = match iter {
        Value::List(list) => list.iter().cloned().collect::<Vec<_>>(),
        Value::Range(range) => range.iter().map(Value::int).collect(),
        _ => return Err(for_requires_iterable()),
    };

    if is_yield {
        let mut results = Vec::new();
        for item in items {
            env.push_scope();
            env.define(binding, item, false);

            let (result, action) = eval_body(body, guard, env)?;
            env.pop_scope();

            match action {
                LoopAction::Continue => {
                    results.push(result);
                }
                LoopAction::Break(val) => {
                    return Ok(val);
                }
                LoopAction::Error(e) => {
                    return Err(e);
                }
            }
        }
        Ok(Value::list(results))
    } else {
        for item in items {
            env.push_scope();
            env.define(binding, item, false);

            let (_, action) = eval_body(body, guard, env)?;
            env.pop_scope();

            match action {
                LoopAction::Continue => {}
                LoopAction::Break(val) => {
                    return Ok(val);
                }
                LoopAction::Error(e) => {
                    return Err(e);
                }
            }
        }
        Ok(Value::Void)
    }
}

/// Evaluate a loop expression.
pub fn eval_loop<F>(body: ExprId, mut eval_fn: F) -> EvalResult
where
    F: FnMut(ExprId) -> Result<LoopAction, EvalError>,
{
    loop {
        match eval_fn(body)? {
            LoopAction::Continue => {}
            LoopAction::Break(val) => {
                return Ok(val);
            }
            LoopAction::Error(e) => {
                return Err(e);
            }
        }
    }
}

/// Parse a loop control message (break/continue).
pub fn parse_loop_control(message: &str) -> LoopAction {
    if message == "continue" {
        LoopAction::Continue
    } else if let Some(val_str) = message.strip_prefix("break:") {
        if val_str == "void" {
            LoopAction::Break(Value::Void)
        } else {
            // For simplicity, just return void
            LoopAction::Break(Value::Void)
        }
    } else {
        LoopAction::Error(EvalError::new(message))
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
pub fn eval_block<F, G>(
    stmts: StmtRange,
    result: Option<ExprId>,
    arena: &ExprArena,
    env: &mut Environment,
    mut eval_fn: F,
    mut bind_fn: G,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
    G: FnMut(&BindingPattern, Value, bool) -> EvalResult,
{
    env.push_scope();

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
                let value = eval_fn(*init)?;
                bind_fn(pattern, value, *mutable)?;
            }
        }
    }

    let result_val = if let Some(r) = result {
        eval_fn(r)?
    } else {
        Value::Void
    };

    env.pop_scope();
    Ok(result_val)
}

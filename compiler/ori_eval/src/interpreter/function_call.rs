//! Function call evaluation methods for the Interpreter.

use super::scope_guard::CallFrameGuard;
use super::Interpreter;
use crate::errors::not_callable;
use crate::exec::call::{
    bind_captures, bind_parameters_with_defaults, check_arg_count, eval_function_val_call,
};
use crate::{Environment, EvalResult, FunctionValue, Mutability, Value};
use ori_patterns::ControlAction;

/// Catch `ControlAction::Propagate` at function call boundaries.
///
/// The `?` operator emits `Propagate(Value::Err(e))` or `Propagate(Value::None)` to signal
/// early return from the enclosing function. At the function call boundary, this signal must
/// be converted back to a normal return value — the function "returns" the error/none value.
/// This mirrors Rust's `?` semantics: `fn foo() -> Result<T,E>` returns `Err(e)` on `?`.
#[inline]
fn catch_propagation(result: EvalResult) -> EvalResult {
    match result {
        Err(ControlAction::Propagate(v)) => Ok(v),
        other => other,
    }
}

impl Interpreter<'_> {
    /// Evaluate a function body ON THIS interpreter (no child-interpreter build).
    ///
    /// Saves the per-call fields (`env`, `imported_arena`, `canon`), installs the call
    /// state, evaluates the body, then restores — eliminating the per-call `Interpreter`
    /// struct build + drop. Mode-state counters accumulate directly (no child `ModeState`,
    /// no merge). The `CallFrame` is pushed/popped on the shared `call_stack`. The inner
    /// IIFE funnels every exit path (incl. a `?` early return from parameter binding)
    /// through the restore, so the parent state is never left installed. Saved locals live
    /// on this Rust call frame, so recursive calls nest correctly (each frame restores its
    /// own caller's state). `self.arena` is intentionally NOT swapped — `eval_can` reads
    /// `self.canon`, so the parent's `&'a` arena borrow is never consulted during the call.
    ///
    /// Precondition: `check_recursion_limit()` has passed, so `call_stack.push` cannot fail.
    pub(super) fn run_function_body(
        &mut self,
        f: &FunctionValue,
        func: &Value,
        args: &[Value],
        call_env: Environment,
        self_name: ori_ir::Name,
    ) -> EvalResult {
        let new_canon = f.canon().cloned().or_else(|| self.canon.clone());
        // RAII guard: installs the call frame + restores the caller's state on drop, INCLUDING
        // a panic unwinding out of the body eval (the panic-safety the removed per-call child
        // interpreter's Drop provided). The IIFE funnels the `?` early-return from
        // `bind_parameters_with_defaults` through the same restore path.
        let mut guard = CallFrameGuard::install(
            self,
            call_env,
            f.shared_arena().clone(),
            new_canon,
            self_name,
        );

        let result = (|| {
            bind_parameters_with_defaults(&mut guard, f, args)?;
            // Bind 'self' to the current function for recursive patterns.
            guard
                .env
                .define(self_name, func.clone(), Mutability::Immutable);
            guard.eval_can(f.can_body)
        })();

        drop(guard);
        catch_propagation(result)
    }

    /// Evaluate a function call.
    #[tracing::instrument(level = "debug", skip_all)]
    pub(super) fn eval_call(&mut self, func: &Value, args: &[Value]) -> EvalResult {
        self.mode_state.count_function_call();

        // Enforce ConstEval call budget (no-op for other modes).
        if let Err(exceeded) = self.mode_state.check_budget() {
            return Err(crate::errors::budget_exceeded(exceeded.calls, exceeded.budget).into());
        }

        match func {
            Value::Function(f) => {
                self.check_recursion_limit()?;
                let self_name = self.self_name;
                let call_env = self.prepare_call_env(f, args)?;
                self.run_function_body(f, func, args, call_env, self_name)
            }
            Value::MemoizedFunction(mf) => {
                // Check cache first
                if let Some(cached) = mf.get_cached(args) {
                    return Ok(cached);
                }

                self.check_recursion_limit()?;
                let self_name = self.self_name;
                let f = &mf.func;
                let call_env = self.prepare_call_env(f, args)?;
                // `func` (the MemoizedFunction value) is bound as 'self' so recursive calls
                // also hit the cache; `run_function_body` already applies `catch_propagation`.
                let result = self.run_function_body(f, func, args, call_env, self_name);

                // Cache the result before returning
                if let Ok(ref value) = result {
                    mf.cache_result(args, value.clone());
                }

                result
            }
            Value::FunctionVal(func_ptr, _name) => eval_function_val_call(*func_ptr, args),
            Value::VariantConstructor {
                type_name,
                variant_name,
                field_count,
            } => {
                // Check argument count matches field count
                if args.len() != *field_count {
                    return Err(crate::errors::wrong_arg_count(
                        self.interner.lookup(*variant_name),
                        *field_count,
                        args.len(),
                    )
                    .into());
                }
                // Construct the variant with the provided arguments
                Ok(Value::variant(*type_name, *variant_name, args.to_vec()))
            }
            Value::NewtypeConstructor { type_name } => {
                // Newtypes take exactly one argument (the underlying value)
                if args.len() != 1 {
                    return Err(crate::errors::wrong_arg_count(
                        self.interner.lookup(*type_name),
                        1,
                        args.len(),
                    )
                    .into());
                }
                // Construct the newtype wrapping the underlying value
                Ok(Value::newtype(*type_name, args[0].clone()))
            }
            _ => Err(not_callable(func.type_name()).into()),
        }
    }

    /// Shared environment setup for `Function` and `MemoizedFunction` calls.
    ///
    /// Performs: arg count check, child environment creation, capture binding,
    /// and capability propagation. Returns the prepared environment.
    ///
    /// The caller is responsible for creating the child interpreter, binding
    /// parameters (which may need `eval_can` for defaults), binding `self`,
    /// and executing the body.
    fn prepare_call_env(
        &self,
        f: &FunctionValue,
        args: &[Value],
    ) -> Result<Environment, ori_patterns::ControlAction> {
        check_arg_count(f, args)?;

        let mut call_env = self.env.child();
        call_env.push_scope();

        bind_captures(&mut call_env, f);

        // Pass capabilities from calling scope to called function.
        // The type checker already validated capability requirements;
        // missing capabilities at runtime are deferred to usage site.
        for cap_name in f.capabilities() {
            if let Some(cap_value) = self.env.lookup(*cap_name) {
                call_env.define(*cap_name, cap_value, Mutability::Immutable);
            }
        }

        Ok(call_env)
    }

    /// Call a function value with the given arguments.
    ///
    /// This is a public wrapper around `eval_call` for use in queries.
    pub fn eval_call_value(&mut self, func: &Value, args: &[Value]) -> EvalResult {
        self.eval_call(func, args)
    }
}

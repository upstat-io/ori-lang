//! Function call evaluation methods for the Interpreter.

use super::Interpreter;
use crate::errors::not_callable;
use crate::exec::call::{
    bind_captures, bind_parameters_with_defaults, check_arg_count, eval_function_val_call,
};
use crate::{Environment, EvalResult, FunctionValue, Mutability, Value};

impl Interpreter<'_> {
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
                let mut call_interpreter = self.create_function_interpreter(
                    f.shared_arena(),
                    call_env,
                    self_name,
                    f.canon().cloned(),
                );
                bind_parameters_with_defaults(&mut call_interpreter, f, args)?;

                // Bind 'self' to the current function for recursive patterns
                call_interpreter
                    .env
                    .define(self_name, func.clone(), Mutability::Immutable);

                let result = call_interpreter.eval_can(f.can_body);
                self.mode_state
                    .merge_child_counters(&call_interpreter.mode_state);
                result
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
                let mut call_interpreter = self.create_function_interpreter(
                    f.shared_arena(),
                    call_env,
                    self_name,
                    f.canon().cloned(),
                );
                bind_parameters_with_defaults(&mut call_interpreter, f, args)?;

                // Bind 'self' to the MEMOIZED function so recursive calls also use the cache
                call_interpreter
                    .env
                    .define(self_name, func.clone(), Mutability::Immutable);

                let result = call_interpreter.eval_can(f.can_body);
                self.mode_state
                    .merge_child_counters(&call_interpreter.mode_state);

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

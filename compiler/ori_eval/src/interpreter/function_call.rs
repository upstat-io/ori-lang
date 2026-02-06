//! Function call evaluation methods for the Interpreter.

use super::Interpreter;
use crate::exec::call::{
    bind_captures, bind_parameters_with_defaults, check_arg_count, check_named_arg_count,
    eval_function_val_call, extract_named_args,
};
use crate::exec::control::try_match;
use crate::{not_callable, EvalResult, Mutability, Value};
use ori_ir::CallArgRange;

impl Interpreter<'_> {
    /// Evaluate a function call.
    #[tracing::instrument(level = "debug", skip_all)]
    pub(super) fn eval_call(&mut self, func: &Value, args: &[Value]) -> EvalResult {
        match func {
            Value::Function(f) => {
                // Check recursion limit before making the call (WASM only)
                self.check_recursion_limit()?;
                check_arg_count(f, args)?;

                // Create new environment with captures, then push a local scope
                let mut call_env = self.env.child();
                call_env.push_scope();

                // Bind captured variables
                bind_captures(&mut call_env, f);

                // Pass capabilities from calling scope to called function
                // This enables capability propagation: functions that `uses` a capability
                // can access it when called from within a `with Capability = ... in` block
                for cap_name in f.capabilities() {
                    if let Some(cap_value) = self.env.lookup(*cap_name) {
                        call_env.define(*cap_name, cap_value, Mutability::Immutable);
                    }
                    // Note: If capability is not in scope, we don't error here.
                    // The type checker already validated that the capability is required.
                    // Runtime errors will occur if the function tries to use the capability.
                }

                // Bind parameters, evaluating defaults for missing arguments
                let func_arena = f.arena();
                let mut call_interpreter = self.create_function_interpreter(func_arena, call_env);
                bind_parameters_with_defaults(&mut call_interpreter, f, args)?;

                // Bind 'self' to the current function for recursive patterns
                // Uses pre-computed self_name to avoid repeated interning
                call_interpreter
                    .env
                    .define(self.self_name, func.clone(), Mutability::Immutable);

                // Evaluate body using the function's arena (arena threading pattern).
                // The scope is popped automatically via RAII when call_interpreter drops.
                call_interpreter.eval(f.body)
            }
            Value::MemoizedFunction(mf) => {
                // Check cache first
                if let Some(cached) = mf.get_cached(args) {
                    return Ok(cached);
                }

                // Check recursion limit before making the call (WASM only)
                self.check_recursion_limit()?;

                // Not cached - evaluate the underlying function
                let f = &mf.func;
                check_arg_count(f, args)?;

                // Create new environment with captures, then push a local scope
                let mut call_env = self.env.child();
                call_env.push_scope();

                // Bind captured variables
                bind_captures(&mut call_env, f);

                // Pass capabilities from calling scope to called function
                for cap_name in f.capabilities() {
                    if let Some(cap_value) = self.env.lookup(*cap_name) {
                        call_env.define(*cap_name, cap_value, Mutability::Immutable);
                    }
                }

                // Bind parameters, evaluating defaults for missing arguments
                let func_arena = f.arena();
                let mut call_interpreter = self.create_function_interpreter(func_arena, call_env);
                bind_parameters_with_defaults(&mut call_interpreter, f, args)?;

                // Bind 'self' to the MEMOIZED function so recursive calls also use the cache
                // Uses pre-computed self_name to avoid repeated interning
                call_interpreter
                    .env
                    .define(self.self_name, func.clone(), Mutability::Immutable);

                // Evaluate body using the function's arena (arena threading pattern).
                // The scope is popped automatically via RAII when call_interpreter drops.
                let result = call_interpreter.eval(f.body);

                // Cache the result before returning
                if let Ok(ref value) = result {
                    mf.cache_result(args, value.clone());
                }

                result
            }
            Value::MultiClauseFunction(clauses) => {
                // Check recursion limit before making the call (WASM only)
                self.check_recursion_limit()?;

                // Try each clause in order until one matches
                for f in clauses.iter() {
                    // Check argument count
                    if args.len() != f.params.len() {
                        continue; // Wrong arity, try next clause
                    }

                    // Try to match each argument against its pattern
                    let mut all_bindings: Vec<(ori_ir::Name, Value)> = Vec::new();
                    let mut all_match = true;
                    let func_arena = f.arena();

                    for (i, arg) in args.iter().enumerate() {
                        if let Some(pattern) = f.patterns.get(i).and_then(|p| p.as_ref()) {
                            // There's a pattern for this parameter - match against it
                            if let Some(bindings) = try_match(
                                pattern,
                                arg,
                                func_arena,
                                self.interner,
                                None,
                                self.pattern_resolutions,
                            )? {
                                all_bindings.extend(bindings);
                            } else {
                                all_match = false;
                                break;
                            }
                        }
                        // No pattern means simple binding - always matches
                        // The binding is done via the parameter name
                    }

                    if !all_match {
                        continue; // Patterns didn't match, try next clause
                    }

                    // Create environment for clause execution
                    let mut call_env = self.env.child();
                    call_env.push_scope();

                    // Bind captured variables
                    bind_captures(&mut call_env, f);

                    // Pass capabilities from calling scope
                    for cap_name in f.capabilities() {
                        if let Some(cap_value) = self.env.lookup(*cap_name) {
                            call_env.define(*cap_name, cap_value, Mutability::Immutable);
                        }
                    }

                    // Create interpreter for guard and body evaluation
                    let mut call_interpreter =
                        self.create_function_interpreter(func_arena, call_env);

                    // Bind parameters (positionally, since patterns already matched)
                    for (param, arg) in f.params.iter().zip(args.iter()) {
                        call_interpreter
                            .env
                            .define(*param, arg.clone(), Mutability::Immutable);
                    }

                    // Bind pattern-extracted bindings
                    for (name, value) in all_bindings {
                        call_interpreter
                            .env
                            .define(name, value, Mutability::Immutable);
                    }

                    // Check guard if present
                    if let Some(guard_expr) = f.guard {
                        let guard_result = call_interpreter.eval(guard_expr)?;
                        match guard_result {
                            Value::Bool(true) => {
                                // Guard passed - execute body
                            }
                            Value::Bool(false) => {
                                // Guard failed - try next clause
                                continue;
                            }
                            _ => {
                                return Err(crate::EvalError::new(
                                    "guard expression must return bool".to_string(),
                                ));
                            }
                        }
                    }

                    // Bind 'self' to the multi-clause function for recursion
                    call_interpreter.env.define(
                        self.self_name,
                        func.clone(),
                        Mutability::Immutable,
                    );

                    // Execute the matching clause's body
                    return call_interpreter.eval(f.body);
                }

                // No clause matched - non-exhaustive patterns error
                let clause_count = clauses.len();
                Err(crate::EvalError::new(format!(
                    "no matching clause found for function call (tried {clause_count} clauses)"
                )))
            }
            Value::FunctionVal(func_ptr, _name) => eval_function_val_call(*func_ptr, args),
            Value::VariantConstructor {
                type_name,
                variant_name,
                field_count,
            } => {
                // Check argument count matches field count
                if args.len() != *field_count {
                    return Err(crate::wrong_arg_count(
                        self.interner.lookup(*variant_name),
                        *field_count,
                        args.len(),
                    ));
                }
                // Construct the variant with the provided arguments
                Ok(Value::variant(*type_name, *variant_name, args.to_vec()))
            }
            Value::NewtypeConstructor { type_name } => {
                // Newtypes take exactly one argument (the underlying value)
                if args.len() != 1 {
                    return Err(crate::wrong_arg_count(
                        self.interner.lookup(*type_name),
                        1,
                        args.len(),
                    ));
                }
                // Construct the newtype wrapping the underlying value
                Ok(Value::newtype(*type_name, args[0].clone()))
            }
            _ => Err(not_callable(func.type_name())),
        }
    }

    /// Evaluate a function call with named arguments.
    ///
    /// Named arguments allow:
    /// - Arguments in any order (matched by name to parameters)
    /// - Omitting parameters that have default values
    pub(super) fn eval_call_named(&mut self, func: &Value, args: CallArgRange) -> EvalResult {
        if let Value::Function(f) = func {
            // Check recursion limit before making the call (WASM only)
            self.check_recursion_limit()?;

            // Validate argument count against parameters with defaults
            check_named_arg_count(f, args, self.arena)?;

            // Step 1: Evaluate call arguments in CALLER's arena context
            // Build a map of parameter name -> evaluated value from the call site
            let call_args = self.arena.get_call_args(args);
            let mut arg_values: rustc_hash::FxHashMap<ori_ir::Name, Value> =
                rustc_hash::FxHashMap::default();
            for arg in call_args {
                if let Some(name) = arg.name {
                    let value = self.eval(arg.value)?;
                    arg_values.insert(name, value);
                }
            }

            // Create new environment with captures, then push a local scope
            let mut call_env = self.env.child();
            call_env.push_scope();

            // Bind captured variables
            bind_captures(&mut call_env, f);

            // Pass capabilities from calling scope to called function
            for cap_name in f.capabilities() {
                if let Some(cap_value) = self.env.lookup(*cap_name) {
                    call_env.define(*cap_name, cap_value, Mutability::Immutable);
                }
            }

            // Step 2: Create function interpreter with function's arena for default eval
            let func_arena = f.arena();
            let mut call_interpreter = self.create_function_interpreter(func_arena, call_env);

            // Step 3: Bind parameters, using evaluated args or evaluating defaults
            for (i, param) in f.params.iter().enumerate() {
                let value = if let Some(val) = arg_values.get(param) {
                    // Named argument was provided for this parameter
                    val.clone()
                } else if let Some(default_expr) = f.defaults.get(i).and_then(|d| *d) {
                    // Use default expression (evaluated in function's arena)
                    call_interpreter.eval(default_expr)?
                } else {
                    // No argument and no default - shouldn't happen after check
                    return Err(crate::EvalError::new(
                        "missing required argument".to_string(),
                    ));
                };
                call_interpreter
                    .env
                    .define(*param, value, Mutability::Immutable);
            }

            // Bind 'self' to the current function for recursive patterns
            call_interpreter
                .env
                .define(self.self_name, func.clone(), Mutability::Immutable);

            // Evaluate body using the function's arena
            call_interpreter.eval(f.body)
        } else {
            // For other callables (MemoizedFunction, FunctionVal, constructors),
            // use positional evaluation
            let arg_values = extract_named_args(args, self.arena, |expr| self.eval(expr))?;
            self.eval_call(func, &arg_values)
        }
    }

    /// Call a function value with the given arguments.
    ///
    /// This is a public wrapper around `eval_call` for use in queries.
    pub fn eval_call_value(&mut self, func: &Value, args: &[Value]) -> EvalResult {
        self.eval_call(func, args)
    }
}

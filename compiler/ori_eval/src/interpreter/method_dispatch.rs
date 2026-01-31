//! Method dispatch methods for the Interpreter.

use super::resolvers::{CollectionMethod, MethodResolution};
use super::Interpreter;
use crate::{
    // Error factories for collection methods
    all_requires_list,
    any_requires_list,
    collect_requires_range,
    dispatch_builtin_method,
    filter_entries_not_implemented,
    filter_entries_requires_map,
    filter_requires_collection,
    find_requires_list,
    fold_requires_collection,
    map_entries_not_implemented,
    map_entries_requires_map,
    map_requires_collection,
    wrong_arg_count,
    wrong_function_args,
    EvalError,
    EvalResult,
    Mutability,
    UserMethod,
    Value,
};
use ori_ir::{ExprArena, Name};

impl Interpreter<'_> {
    /// Evaluate a method call using the Chain of Responsibility pattern.
    ///
    /// Methods are resolved in priority order:
    /// 0. Associated functions on type references (e.g., `Duration.from_seconds`)
    /// 1. User-defined methods from impl blocks (priority 0)
    /// 2. Derived methods from `#[derive(...)]` (priority 1)
    /// 3. Collection methods requiring interpreter (priority 2)
    /// 4. Built-in methods in `MethodRegistry` (priority 3)
    pub fn eval_method_call(
        &mut self,
        receiver: Value,
        method: Name,
        args: Vec<Value>,
    ) -> EvalResult {
        // Handle associated function calls on type references
        if let Value::TypeRef { type_name } = &receiver {
            let type_name_str = self.interner.lookup(*type_name);
            let method_str = self.interner.lookup(method);
            return crate::methods::dispatch_associated_function(type_name_str, method_str, args);
        }

        let type_name = self.get_value_type_name(&receiver);

        // Resolve the method using the resolver chain
        let resolution = self.resolve_method(&receiver, type_name, method);

        // Execute based on resolution type
        match resolution {
            MethodResolution::User(user_method) => {
                self.eval_user_method(receiver, &user_method, &args)
            }
            MethodResolution::Derived(derived_info) => {
                self.eval_derived_method(receiver, &derived_info, &args)
            }
            MethodResolution::Collection(collection_method) => {
                self.eval_collection_method(receiver, collection_method, &args)
            }
            MethodResolution::Builtin => {
                let method_name = self.interner.lookup(method);
                dispatch_builtin_method(receiver, method_name, args)
            }
            MethodResolution::NotFound => {
                // This shouldn't happen as BuiltinResolver always returns Builtin,
                // but if it does, fall back to dispatch_builtin_method which will
                // produce an appropriate error
                let method_name = self.interner.lookup(method);
                dispatch_builtin_method(receiver, method_name, args)
            }
        }
    }

    /// Resolve a method using the cached dispatcher chain.
    ///
    /// Uses the pre-built dispatcher to try resolvers in priority order.
    /// The dispatcher sees method registrations made after construction because
    /// `user_method_registry` uses interior mutability (`SharedMutableRegistry`).
    fn resolve_method(
        &self,
        receiver: &Value,
        type_name: Name,
        method_name: Name,
    ) -> MethodResolution {
        self.method_dispatcher
            .resolve(receiver, type_name, method_name)
    }

    /// Evaluate a collection method that requires interpreter access.
    fn eval_collection_method(
        &mut self,
        receiver: Value,
        method: CollectionMethod,
        args: &[Value],
    ) -> EvalResult {
        match method {
            CollectionMethod::Map => match receiver {
                Value::List(items) => self.eval_list_map(items.as_ref(), args),
                Value::Range(range) => self.eval_range_map(&range, args),
                _ => Err(map_requires_collection()),
            },
            CollectionMethod::Filter => match receiver {
                Value::List(items) => self.eval_list_filter(items.as_ref(), args),
                Value::Range(range) => self.eval_range_filter(&range, args),
                _ => Err(filter_requires_collection()),
            },
            CollectionMethod::Fold => match receiver {
                Value::List(items) => self.eval_list_fold(items.as_ref(), args),
                Value::Range(range) => self.eval_range_fold(&range, args),
                _ => Err(fold_requires_collection()),
            },
            CollectionMethod::Find => match receiver {
                Value::List(items) => self.eval_list_find(items.as_ref(), args),
                _ => Err(find_requires_list()),
            },
            CollectionMethod::Collect => match receiver {
                Value::Range(range) => self.eval_range_collect(&range, args),
                _ => Err(collect_requires_range()),
            },
            CollectionMethod::Any => match receiver {
                Value::List(items) => self.eval_list_any(items.as_ref(), args),
                _ => Err(any_requires_list()),
            },
            CollectionMethod::All => match receiver {
                Value::List(items) => self.eval_list_all(items.as_ref(), args),
                _ => Err(all_requires_list()),
            },
            CollectionMethod::MapEntries => match receiver {
                Value::Map(_) => Err(map_entries_not_implemented()),
                _ => Err(map_entries_requires_map()),
            },
            CollectionMethod::FilterEntries => match receiver {
                Value::Map(_) => Err(filter_entries_not_implemented()),
                _ => Err(filter_entries_requires_map()),
            },
        }
    }

    // Iterator Helper Methods - unify collection method implementations for lists and ranges

    /// Apply a transform function to each item in an iterator, collecting results.
    ///
    /// Uses `size_hint` to pre-allocate the result vector when the size is known.
    /// For list methods that already have references, use `map_slice` instead to
    /// avoid cloning items that may not need transformation.
    fn map_iterator(&mut self, iter: impl Iterator<Item = Value>, transform: &Value) -> EvalResult {
        let (lower, _) = iter.size_hint();
        let mut result = Vec::with_capacity(lower);
        for item in iter {
            let mapped = self.eval_call(transform, &[item])?;
            result.push(mapped);
        }
        Ok(Value::list(result))
    }

    /// Map over a slice, cloning items only at the call boundary.
    ///
    /// Uses `from_ref` to avoid explicit cloning - the clone happens inside
    /// `eval_call` when binding parameters, avoiding a double clone.
    fn map_slice(&mut self, items: &[Value], transform: &Value) -> EvalResult {
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            // from_ref creates &[Value] from &Value; clone happens in bind_parameters
            let mapped = self.eval_call(transform, std::slice::from_ref(item))?;
            result.push(mapped);
        }
        Ok(Value::list(result))
    }

    /// Filter items from an iterator using a predicate function.
    ///
    /// Uses `size_hint` to estimate initial capacity (filter results may be smaller).
    /// For list methods, use `filter_slice` to avoid cloning discarded items.
    fn filter_iterator(
        &mut self,
        iter: impl Iterator<Item = Value>,
        predicate: &Value,
    ) -> EvalResult {
        let (lower, _) = iter.size_hint();
        // Filter may remove items, so use lower bound as estimate
        let mut result = Vec::with_capacity(lower);
        for item in iter {
            let keep = self.eval_call(predicate, std::slice::from_ref(&item))?;
            if keep.is_truthy() {
                result.push(item);
            }
        }
        Ok(Value::list(result))
    }

    /// Filter a slice, cloning only items that pass the predicate.
    ///
    /// This is more efficient than `filter_iterator` for lists because:
    /// - Predicate check uses `from_ref` (no clone for the check)
    /// - Only items that pass are cloned into the result
    fn filter_slice(&mut self, items: &[Value], predicate: &Value) -> EvalResult {
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            // from_ref creates &[Value] from &Value without cloning
            let keep = self.eval_call(predicate, std::slice::from_ref(item))?;
            if keep.is_truthy() {
                // Clone only if keeping
                result.push(item.clone());
            }
        }
        Ok(Value::list(result))
    }

    /// Fold an iterator into a single value using an accumulator function.
    fn fold_iterator(
        &mut self,
        iter: impl Iterator<Item = Value>,
        mut acc: Value,
        op: &Value,
    ) -> EvalResult {
        for item in iter {
            acc = self.eval_call(op, &[acc, item])?;
        }
        Ok(acc)
    }

    /// Fold a slice into a single value, cloning items at the call boundary.
    fn fold_slice(&mut self, items: &[Value], mut acc: Value, op: &Value) -> EvalResult {
        for item in items {
            acc = self.eval_call(op, &[acc, item.clone()])?;
        }
        Ok(acc)
    }

    /// Find first matching item in a slice, cloning only the found item.
    ///
    /// Uses `from_ref` for predicate check (no clone), only clones the result.
    fn find_in_slice(&mut self, items: &[Value], predicate: &Value) -> EvalResult {
        for item in items {
            let found = self.eval_call(predicate, std::slice::from_ref(item))?;
            if found.is_truthy() {
                return Ok(Value::some(item.clone()));
            }
        }
        Ok(Value::None)
    }

    /// Check if any item in a slice matches a predicate (no cloning).
    fn any_in_slice(&mut self, items: &[Value], predicate: &Value) -> EvalResult {
        for item in items {
            let result = self.eval_call(predicate, std::slice::from_ref(item))?;
            if result.is_truthy() {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    /// Check if all items in a slice match a predicate (no cloning).
    fn all_in_slice(&mut self, items: &[Value], predicate: &Value) -> EvalResult {
        for item in items {
            let result = self.eval_call(predicate, std::slice::from_ref(item))?;
            if !result.is_truthy() {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    /// Validate that the expected number of arguments was provided.
    #[inline]
    fn expect_arg_count(
        method_name: &str,
        expected: usize,
        args: &[Value],
    ) -> Result<(), EvalError> {
        if args.len() == expected {
            Ok(())
        } else {
            Err(wrong_arg_count(method_name, expected, args.len()))
        }
    }

    fn eval_list_map(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        Self::expect_arg_count("map", 1, args)?;
        self.map_slice(items, &args[0])
    }

    fn eval_list_filter(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        Self::expect_arg_count("filter", 1, args)?;
        self.filter_slice(items, &args[0])
    }

    fn eval_list_fold(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        Self::expect_arg_count("fold", 2, args)?;
        self.fold_slice(items, args[0].clone(), &args[1])
    }

    fn eval_list_find(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        Self::expect_arg_count("find", 1, args)?;
        self.find_in_slice(items, &args[0])
    }

    fn eval_list_any(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        Self::expect_arg_count("any", 1, args)?;
        self.any_in_slice(items, &args[0])
    }

    fn eval_list_all(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        Self::expect_arg_count("all", 1, args)?;
        self.all_in_slice(items, &args[0])
    }

    #[expect(
        clippy::unused_self,
        reason = "Consistent method signature with other eval_range_* methods that do use self"
    )]
    fn eval_range_collect(&mut self, range: &crate::RangeValue, args: &[Value]) -> EvalResult {
        Self::expect_arg_count("collect", 0, args)?;
        let result: Vec<Value> = range.iter().map(Value::int).collect();
        Ok(Value::list(result))
    }

    fn eval_range_map(&mut self, range: &crate::RangeValue, args: &[Value]) -> EvalResult {
        Self::expect_arg_count("map", 1, args)?;
        self.map_iterator(range.iter().map(Value::int), &args[0])
    }

    fn eval_range_filter(&mut self, range: &crate::RangeValue, args: &[Value]) -> EvalResult {
        Self::expect_arg_count("filter", 1, args)?;
        self.filter_iterator(range.iter().map(Value::int), &args[0])
    }

    fn eval_range_fold(&mut self, range: &crate::RangeValue, args: &[Value]) -> EvalResult {
        Self::expect_arg_count("fold", 2, args)?;
        self.fold_iterator(range.iter().map(Value::int), args[0].clone(), &args[1])
    }

    /// Get the concrete type name for a value as an interned Name.
    ///
    /// For struct values, returns the struct's `type_name` directly.
    /// For other values, interns the static type name string.
    ///
    /// This avoids String allocation during method dispatch by using
    /// interned Names throughout the lookup chain.
    pub(super) fn get_value_type_name(&self, value: &Value) -> Name {
        match value {
            Value::Struct(s) => s.type_name,
            Value::Range(_) => self.interner.intern("range"),
            Value::Int(_) => self.interner.intern("int"),
            Value::Float(_) => self.interner.intern("float"),
            Value::Bool(_) => self.interner.intern("bool"),
            Value::Str(_) => self.interner.intern("str"),
            Value::Char(_) => self.interner.intern("char"),
            Value::Byte(_) => self.interner.intern("byte"),
            Value::Void => self.interner.intern("void"),
            Value::Duration(_) => self.interner.intern("Duration"),
            Value::Size(_) => self.interner.intern("Size"),
            Value::List(_) => self.interner.intern("list"),
            Value::Map(_) => self.interner.intern("map"),
            Value::Tuple(_) => self.interner.intern("tuple"),
            Value::Some(_) | Value::None => self.interner.intern("Option"),
            Value::Ok(_) | Value::Err(_) => self.interner.intern("Result"),
            Value::Variant { type_name, .. }
            | Value::VariantConstructor { type_name, .. }
            | Value::Newtype { type_name, .. }
            | Value::NewtypeConstructor { type_name }
            | Value::TypeRef { type_name } => *type_name,
            Value::Function(_) | Value::MemoizedFunction(_) => self.interner.intern("function"),
            Value::FunctionVal(_, _) => self.interner.intern("function_val"),
            Value::ModuleNamespace(_) => self.interner.intern("module"),
            Value::Error(_) => self.interner.intern("error"),
        }
    }

    /// Evaluate a user-defined method from an impl block.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "method params always include self, so len >= 1"
    )]
    pub(super) fn eval_user_method(
        &mut self,
        receiver: Value,
        method: &UserMethod,
        args: &[Value],
    ) -> EvalResult {
        // Method params include 'self' as first parameter
        if method.params.len() != args.len() + 1 {
            return Err(wrong_function_args(method.params.len() - 1, args.len()));
        }

        // Create new environment with captures
        let mut call_env = self.env.child();
        call_env.push_scope();

        // Bind captured variables (dereference Arc to iterate HashMap)
        for (name, value) in method.captures.iter() {
            call_env.define(*name, value.clone(), Mutability::Immutable);
        }

        // Bind 'self' to receiver (first parameter)
        if let Some(&self_param) = method.params.first() {
            call_env.define(self_param, receiver, Mutability::Immutable);
        }

        // Bind remaining parameters
        for (param, arg) in method.params.iter().skip(1).zip(args.iter()) {
            call_env.define(*param, arg.clone(), Mutability::Immutable);
        }

        // Evaluate method body using the method's arena (arena threading pattern).
        // The scope is popped automatically via RAII when call_interpreter drops.
        let func_arena: &ExprArena = &method.arena;
        let mut call_interpreter = self.create_function_interpreter(func_arena, call_env);
        call_interpreter.eval(method.body)
    }

    // NOTE: Derived method evaluation has been moved to `derived_methods.rs`
    // for better separation of concerns. The method `eval_derived_method`
    // and its helpers are now in that module.
}

#[cfg(test)]
mod tests {
    use super::*;

    mod expect_arg_count_tests {
        use super::*;

        #[test]
        fn correct_count_returns_ok() {
            let args = vec![Value::int(1)];
            assert!(Interpreter::expect_arg_count("test", 1, &args).is_ok());
        }

        #[test]
        fn wrong_count_returns_error() {
            let args = vec![Value::int(1), Value::int(2)];
            let result = Interpreter::expect_arg_count("test", 1, &args);
            let Err(err) = result else {
                panic!("expected error for wrong arg count");
            };
            assert!(err.message.contains("test"));
        }

        #[test]
        fn zero_expected_zero_given_ok() {
            let args: Vec<Value> = vec![];
            assert!(Interpreter::expect_arg_count("test", 0, &args).is_ok());
        }

        #[test]
        fn zero_expected_one_given_error() {
            let args = vec![Value::int(1)];
            assert!(Interpreter::expect_arg_count("test", 0, &args).is_err());
        }
    }

    // Note: More comprehensive method dispatch tests are in the integration
    // tests (tests/spec/) since they require a full interpreter setup.
    // These unit tests cover the simpler helper functions.
}

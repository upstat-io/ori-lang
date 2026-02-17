//! Method dispatch methods for the Interpreter.

use ori_ir::Name;

mod iterator;

use crate::errors::{
    all_requires_list, any_requires_list, collect_requires_range, filter_entries_not_implemented,
    filter_entries_requires_map, filter_requires_collection, find_requires_list,
    fold_requires_collection, map_entries_not_implemented, map_entries_requires_map,
    map_requires_collection, wrong_arg_count, wrong_function_args,
};
use crate::exec::call::bind_captures_iter;
use crate::methods::{dispatch_builtin_method, DispatchCtx};
use crate::{EvalError, EvalResult, Mutability, UserMethod, Value};

use super::resolvers::{CollectionMethod, MethodResolution};
use super::Interpreter;

impl Interpreter<'_> {
    /// Evaluate a method call using the Chain of Responsibility pattern.
    ///
    /// Methods are resolved in priority order:
    /// 0. Print methods (invoked via `PatternExecutor` for the Print capability)
    /// 1. Associated functions on type references (e.g., `Duration.from_seconds`)
    /// 2. User-defined methods from impl blocks (priority 0)
    /// 3. Derived methods from `#[derive(...)]` (priority 1)
    /// 4. Collection methods requiring interpreter (priority 2)
    /// 5. Built-in methods in `MethodRegistry` (priority 3)
    #[tracing::instrument(level = "debug", skip(self, receiver, args))]
    pub fn eval_method_call(
        &mut self,
        receiver: Value,
        method: Name,
        args: Vec<Value>,
    ) -> EvalResult {
        self.mode_state.count_method_call();

        // Handle print methods (invoked via PatternExecutor for the Print capability).
        // Pre-interned Name comparison avoids string lookup on every method call.
        let pn = self.print_names;
        if method == pn.println || method == pn.builtin_println {
            self.handle_println(&args);
            return Ok(Value::Void);
        }
        if method == pn.print || method == pn.builtin_print {
            self.handle_print(&args);
            return Ok(Value::Void);
        }

        // Handle associated function calls on type references
        if let Value::TypeRef { type_name } = &receiver {
            // First check user-defined associated functions in the registry
            // Clone the method to release the lock before calling eval_associated_function
            let user_method = self
                .user_method_registry
                .read()
                .lookup(*type_name, method)
                .cloned();

            if let Some(ref method_def) = user_method {
                return self.eval_associated_function(method_def, &args, method);
            }

            // Check derived methods (e.g., Default.default() is a static method)
            let derived_info = self
                .user_method_registry
                .read()
                .lookup_derived(*type_name, method)
                .cloned();
            if let Some(ref info) = derived_info {
                return self.eval_derived_method(
                    Value::TypeRef {
                        type_name: *type_name,
                    },
                    info,
                    &args,
                );
            }

            // Fall back to built-in associated functions (Duration, Size)
            let ctx = DispatchCtx {
                names: &self.builtin_method_names,
                interner: self.interner,
            };
            return crate::methods::dispatch_associated_function(*type_name, method, args, &ctx);
        }

        // Handle callable struct fields: if a struct has a field with the method name
        // and that field is a function, call it instead of treating as a method.
        // This enables patterns like: `Handler { callback: fn }.callback(arg)`
        if let Value::Struct(s) = &receiver {
            if let Some(field_value) = s.get_field(method) {
                // Check if the field is callable
                match &field_value {
                    Value::Function(_) | Value::MemoizedFunction(_) | Value::FunctionVal(_, _) => {
                        return self.eval_call(field_value, &args);
                    }
                    _ => {
                        // Field exists but isn't callable - fall through to method dispatch
                    }
                }
            }
        }

        let type_name = self.get_value_type_name(&receiver);

        // Resolve the method using the resolver chain
        let resolution = self.resolve_method(&receiver, type_name, method);

        // Execute based on resolution type
        match resolution {
            MethodResolution::User(user_method) => {
                self.eval_user_method(receiver, &user_method, &args, method)
            }
            MethodResolution::Derived(derived_info) => {
                self.eval_derived_method(receiver, &derived_info, &args)
            }
            MethodResolution::Collection(collection_method) => {
                self.eval_collection_method(receiver, collection_method, &args)
            }
            MethodResolution::Builtin => {
                let ctx = DispatchCtx {
                    names: &self.builtin_method_names,
                    interner: self.interner,
                };
                dispatch_builtin_method(receiver, method, args, &ctx)
            }
            MethodResolution::NotFound => {
                let method_str = self.interner.lookup(method);
                let type_str = self.interner.lookup(type_name);
                Err(crate::errors::no_such_method(method_str, type_str).into())
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
                _ => Err(map_requires_collection().into()),
            },
            CollectionMethod::Filter => match receiver {
                Value::List(items) => self.eval_list_filter(items.as_ref(), args),
                Value::Range(range) => self.eval_range_filter(&range, args),
                _ => Err(filter_requires_collection().into()),
            },
            CollectionMethod::Fold => match receiver {
                Value::List(items) => self.eval_list_fold(items.as_ref(), args),
                Value::Range(range) => self.eval_range_fold(&range, args),
                _ => Err(fold_requires_collection().into()),
            },
            CollectionMethod::Find => match receiver {
                Value::List(items) => self.eval_list_find(items.as_ref(), args),
                _ => Err(find_requires_list().into()),
            },
            CollectionMethod::Collect => match receiver {
                Value::Range(range) => self.eval_range_collect(&range, args),
                _ => Err(collect_requires_range().into()),
            },
            CollectionMethod::Any => match receiver {
                Value::List(items) => self.eval_list_any(items.as_ref(), args),
                _ => Err(any_requires_list().into()),
            },
            CollectionMethod::All => match receiver {
                Value::List(items) => self.eval_list_all(items.as_ref(), args),
                _ => Err(all_requires_list().into()),
            },
            CollectionMethod::MapEntries => match receiver {
                Value::Map(_) => Err(map_entries_not_implemented().into()),
                _ => Err(map_entries_requires_map().into()),
            },
            CollectionMethod::FilterEntries => match receiver {
                Value::Map(_) => Err(filter_entries_not_implemented().into()),
                _ => Err(filter_entries_requires_map().into()),
            },

            // Iterator methods — delegate to iterator submodule
            CollectionMethod::IterNext
            | CollectionMethod::IterMap
            | CollectionMethod::IterFilter
            | CollectionMethod::IterTake
            | CollectionMethod::IterSkip
            | CollectionMethod::IterEnumerate
            | CollectionMethod::IterZip
            | CollectionMethod::IterChain
            | CollectionMethod::IterFlatten
            | CollectionMethod::IterFlatMap
            | CollectionMethod::IterCycle
            | CollectionMethod::IterNextBack
            | CollectionMethod::IterRev
            | CollectionMethod::IterLast
            | CollectionMethod::IterRFind
            | CollectionMethod::IterRFold
            | CollectionMethod::IterFold
            | CollectionMethod::IterCount
            | CollectionMethod::IterFind
            | CollectionMethod::IterAny
            | CollectionMethod::IterAll
            | CollectionMethod::IterForEach
            | CollectionMethod::IterCollect
            | CollectionMethod::IterCollectSet
            | CollectionMethod::IterJoin => self.eval_iterator_method(receiver, method, args),
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
        if range.is_unbounded() {
            return Err(crate::errors::unbounded_range_eager("collect").into());
        }
        let result: Vec<Value> = range.iter().map(Value::int).collect();
        Ok(Value::list(result))
    }

    fn eval_range_map(&mut self, range: &crate::RangeValue, args: &[Value]) -> EvalResult {
        Self::expect_arg_count("map", 1, args)?;
        if range.is_unbounded() {
            return Err(crate::errors::unbounded_range_eager("map").into());
        }
        self.map_iterator(range.iter().map(Value::int), &args[0])
    }

    fn eval_range_filter(&mut self, range: &crate::RangeValue, args: &[Value]) -> EvalResult {
        Self::expect_arg_count("filter", 1, args)?;
        if range.is_unbounded() {
            return Err(crate::errors::unbounded_range_eager("filter").into());
        }
        self.filter_iterator(range.iter().map(Value::int), &args[0])
    }

    fn eval_range_fold(&mut self, range: &crate::RangeValue, args: &[Value]) -> EvalResult {
        Self::expect_arg_count("fold", 2, args)?;
        if range.is_unbounded() {
            return Err(crate::errors::unbounded_range_eager("fold").into());
        }
        self.fold_iterator(range.iter().map(Value::int), args[0].clone(), &args[1])
    }

    /// Handle a `println` method call via the print handler.
    fn handle_println(&self, args: &[Value]) {
        if let Some(msg) = args.first() {
            match msg {
                Value::Str(s) => self.print_handler.println(s),
                other => self.print_handler.println(&other.display_value()),
            }
        }
    }

    /// Handle a `print` method call via the print handler.
    fn handle_print(&self, args: &[Value]) {
        if let Some(msg) = args.first() {
            match msg {
                Value::Str(s) => self.print_handler.print(s),
                other => self.print_handler.print(&other.display_value()),
            }
        }
    }

    /// Get the concrete type name for a value as an interned Name.
    ///
    /// For struct values, returns the struct's `type_name` directly.
    /// For other values, uses pre-interned type names from `self.type_names`.
    ///
    /// # Performance
    ///
    /// This method is called on every method dispatch (extremely hot path).
    /// Using pre-interned names avoids hash lookups and lock acquisition
    /// that would occur with `interner.intern()` calls.
    pub(super) fn get_value_type_name(&self, value: &Value) -> Name {
        let names = &self.type_names;
        match value {
            Value::Struct(s) => s.type_name,
            Value::Range(_) => names.range,
            Value::Iterator(_) => names.iterator,
            Value::Int(_) => names.int,
            Value::Float(_) => names.float,
            Value::Bool(_) => names.bool_,
            Value::Str(_) => names.str_,
            Value::Char(_) => names.char_,
            Value::Byte(_) => names.byte,
            Value::Void => names.void,
            Value::Duration(_) => names.duration,
            Value::Size(_) => names.size,
            Value::Ordering(_) => names.ordering,
            Value::List(_) => names.list,
            Value::Map(_) => names.map,
            Value::Set(_) => names.set,
            Value::Tuple(_) => names.tuple,
            Value::Some(_) | Value::None => names.option,
            Value::Ok(_) | Value::Err(_) => names.result,
            Value::Variant { type_name, .. }
            | Value::VariantConstructor { type_name, .. }
            | Value::Newtype { type_name, .. }
            | Value::NewtypeConstructor { type_name }
            | Value::TypeRef { type_name } => *type_name,
            Value::Function(_) | Value::MemoizedFunction(_) => names.function,
            Value::FunctionVal(_, _) => names.function_val,
            Value::ModuleNamespace(_) => names.module,
            Value::Error(_) => names.error,
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
        method_name: Name,
    ) -> EvalResult {
        // Method params include 'self' as first parameter
        if method.params.len() != args.len() + 1 {
            return Err(wrong_function_args(method.params.len() - 1, args.len()).into());
        }
        self.eval_method_body(Some(receiver), method, args, method_name)
    }

    /// Evaluate an associated function (no `self` parameter).
    ///
    /// Associated functions are called on types rather than instances:
    /// `Point.origin()` instead of `point.method()`.
    pub(super) fn eval_associated_function(
        &mut self,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
    ) -> EvalResult {
        // Associated functions don't have 'self', so params == args
        if method.params.len() != args.len() {
            return Err(wrong_function_args(method.params.len(), args.len()).into());
        }
        self.eval_method_body(None, method, args, method_name)
    }

    /// Shared helper for evaluating a method/associated function body.
    ///
    /// When `receiver` is `Some`, binds it as `self` (first param) and zips
    /// remaining params with `args`. When `None`, zips all params with `args`.
    fn eval_method_body(
        &mut self,
        receiver: Option<Value>,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
    ) -> EvalResult {
        self.check_recursion_limit()?;

        let mut call_env = self.env.child();
        call_env.push_scope();

        bind_captures_iter(&mut call_env, method.captures.iter());

        // Bind self + remaining params, or all params directly
        let param_args: &[Name] = if let Some(recv) = receiver {
            if let Some(&self_param) = method.params.first() {
                call_env.define(self_param, recv, Mutability::Immutable);
            }
            &method.params[1..]
        } else {
            &method.params
        };

        for (param, arg) in param_args.iter().zip(args.iter()) {
            call_env.define(*param, arg.clone(), Mutability::Immutable);
        }

        // Evaluate body via canonical IR.
        // The scope is popped automatically via RAII when call_interpreter drops.
        let mut call_interpreter = self.create_function_interpreter(
            &method.arena,
            call_env,
            method_name,
            method.canon.clone(),
        );

        let result = call_interpreter.eval_can(method.can_body);
        self.mode_state
            .merge_child_counters(&call_interpreter.mode_state);
        result
    }
}

#[cfg(test)]
mod tests;

//! Collection method implementations requiring interpreter access.
//!
//! These methods (map, filter, fold, etc.) need `&mut self` because they call
//! user-provided closures via `eval_call`. They are dispatched from the main
//! method routing in `mod.rs` via `eval_collection_method`.

use crate::errors::{
    all_requires_list, any_requires_list, collect_requires_range, filter_entries_not_implemented,
    filter_entries_requires_map, filter_requires_collection, find_requires_list,
    fold_requires_collection, join_requires_list, map_entries_not_implemented,
    map_entries_requires_map, map_requires_collection, wrong_arg_count, wrong_arg_type,
};
use crate::{EvalError, EvalResult, Value};

use super::super::resolvers::CollectionMethod;
use super::super::Interpreter;

impl Interpreter<'_> {
    /// Evaluate a collection method that requires interpreter access.
    pub(super) fn eval_collection_method(
        &mut self,
        receiver: Value,
        method: CollectionMethod,
        args: &[Value],
    ) -> EvalResult {
        match method {
            CollectionMethod::Map => match receiver {
                Value::List(ref items) => self.eval_list_map(items, args),
                Value::Range(range) => self.eval_range_map(&range, args),
                _ => Err(map_requires_collection().into()),
            },
            CollectionMethod::Filter => match receiver {
                Value::List(ref items) => self.eval_list_filter(items, args),
                Value::Range(range) => self.eval_range_filter(&range, args),
                _ => Err(filter_requires_collection().into()),
            },
            CollectionMethod::Fold => match receiver {
                Value::List(ref items) => self.eval_list_fold(items, args),
                Value::Range(range) => self.eval_range_fold(&range, args),
                _ => Err(fold_requires_collection().into()),
            },
            CollectionMethod::Find => match receiver {
                Value::List(ref items) => self.eval_list_find(items, args),
                _ => Err(find_requires_list().into()),
            },
            CollectionMethod::Collect => match receiver {
                Value::Range(range) => self.eval_range_collect(&range, args),
                _ => Err(collect_requires_range().into()),
            },
            CollectionMethod::Any => match receiver {
                Value::List(ref items) => self.eval_list_any(items, args),
                _ => Err(any_requires_list().into()),
            },
            CollectionMethod::All => match receiver {
                Value::List(ref items) => self.eval_list_all(items, args),
                _ => Err(all_requires_list().into()),
            },
            CollectionMethod::Join => match receiver {
                Value::List(ref items) => self.eval_list_join(items, args),
                _ => Err(join_requires_list().into()),
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

            // Ordering — lazy lexicographic chaining
            CollectionMethod::OrderingThenWith => self.eval_ordering_then_with(&receiver, args),
        }
    }

    // Ordering Methods

    /// Lazy lexicographic chaining: `ordering.then_with(() -> compare(...))`.
    ///
    /// If `self` is `Equal`, evaluates the closure `f` and returns its result.
    /// Otherwise returns `self` without evaluating `f` (lazy semantics).
    fn eval_ordering_then_with(&mut self, receiver: &Value, args: &[Value]) -> EvalResult {
        use ori_patterns::{EvalError, OrderingValue};

        let Value::Ordering(ord) = receiver else {
            unreachable!("OrderingThenWith dispatched on non-Ordering receiver");
        };

        if args.len() != 1 {
            return Err(EvalError::new(format!(
                "then_with expects 1 argument, got {}",
                args.len()
            ))
            .into());
        }

        match ord {
            OrderingValue::Equal => self.eval_call(&args[0], &[]),
            _ => Ok(Value::Ordering(*ord)),
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
    pub(super) fn expect_arg_count(
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

    /// `[T].join(sep: str) -> str` — convert each item to string via `to_str()`, join with separator.
    fn eval_list_join(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        Self::expect_arg_count("join", 1, args)?;
        let Value::Str(separator) = &args[0] else {
            return Err(wrong_arg_type("join", "str").into());
        };
        let to_str = self.builtin_method_names.to_str;
        let mut result = String::new();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                result.push_str(separator);
            }
            // Fast path: string values don't need to_str() dispatch
            if let Value::Str(s) = item {
                result.push_str(s);
            } else {
                let str_val = self.eval_method_call(item.clone(), to_str, vec![])?;
                let Value::Str(s) = str_val else {
                    return Err(wrong_arg_type("join", "Printable element").into());
                };
                result.push_str(&s);
            }
        }
        Ok(Value::string(result))
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
}

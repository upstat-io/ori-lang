//! Iterator method dispatch for the Interpreter.
//!
//! Handles both adapter constructors (`map`, `filter`, `take`, `skip`, `enumerate`,
//! `zip`, `chain`, `flatten`, `flat_map`, `cycle`) that return new lazy iterators,
//! and consumer methods (`fold`, `count`, `find`, `any`, `all`, `for_each`, `collect`)
//! that eagerly consume iterators.
//!
//! Adapter iterators capture closures and need `eval_call()` to invoke them
//! on each `next()` call, which is why they live at the interpreter level
//! rather than in the pure `IteratorValue::next()`.

mod consumers;
mod next;

use ori_patterns::IteratorValue;

use crate::errors::wrong_arg_type;
use crate::{ControlAction, EvalResult, Value};

use super::super::resolvers::CollectionMethod;
use super::Interpreter;

impl Interpreter<'_> {
    /// Dispatch an iterator method call.
    pub(in crate::interpreter) fn eval_iterator_method(
        &mut self,
        receiver: Value,
        method: CollectionMethod,
        args: &[Value],
    ) -> EvalResult {
        let Value::Iterator(iter_val) = receiver else {
            unreachable!("eval_iterator_method called with non-iterator receiver")
        };

        match method {
            CollectionMethod::IterNext => {
                Self::expect_arg_count("next", 0, args)?;
                self.eval_iter_next_as_tuple(iter_val)
            }
            CollectionMethod::IterNextBack => {
                Self::expect_arg_count("next_back", 0, args)?;
                self.eval_iter_next_back_as_tuple(iter_val)
            }
            CollectionMethod::IterMap => {
                Self::expect_arg_count("map", 1, args)?;
                Ok(Self::make_mapped(iter_val, args[0].clone()))
            }
            CollectionMethod::IterFilter => {
                Self::expect_arg_count("filter", 1, args)?;
                Ok(Self::make_filtered(iter_val, args[0].clone()))
            }
            CollectionMethod::IterTake => {
                Self::expect_arg_count("take", 1, args)?;
                Self::make_take(iter_val, &args[0])
            }
            CollectionMethod::IterSkip => {
                Self::expect_arg_count("skip", 1, args)?;
                Self::make_skip(iter_val, &args[0])
            }
            CollectionMethod::IterEnumerate => {
                Self::expect_arg_count("enumerate", 0, args)?;
                Ok(Self::make_enumerated(iter_val))
            }
            CollectionMethod::IterZip => {
                Self::expect_arg_count("zip", 1, args)?;
                Self::make_zipped(iter_val, &args[0])
            }
            CollectionMethod::IterChain => {
                Self::expect_arg_count("chain", 1, args)?;
                Self::make_chained(iter_val, &args[0])
            }
            CollectionMethod::IterFlatten => {
                Self::expect_arg_count("flatten", 0, args)?;
                Ok(Self::make_flattened(iter_val))
            }
            CollectionMethod::IterFlatMap => {
                Self::expect_arg_count("flat_map", 1, args)?;
                Ok(Self::make_flat_mapped(iter_val, args[0].clone()))
            }
            CollectionMethod::IterCycle => {
                Self::expect_arg_count("cycle", 0, args)?;
                Ok(Self::make_cycled(iter_val))
            }
            CollectionMethod::IterRev => {
                Self::expect_arg_count("rev", 0, args)?;
                Self::make_reversed(iter_val)
            }
            CollectionMethod::IterFold => {
                Self::expect_arg_count("fold", 2, args)?;
                self.eval_iter_fold(iter_val, args[0].clone(), &args[1])
            }
            CollectionMethod::IterCount => {
                Self::expect_arg_count("count", 0, args)?;
                self.eval_iter_count(iter_val)
            }
            CollectionMethod::IterFind => {
                Self::expect_arg_count("find", 1, args)?;
                self.eval_iter_find(iter_val, &args[0])
            }
            CollectionMethod::IterAny => {
                Self::expect_arg_count("any", 1, args)?;
                self.eval_iter_any(iter_val, &args[0])
            }
            CollectionMethod::IterAll => {
                Self::expect_arg_count("all", 1, args)?;
                self.eval_iter_all(iter_val, &args[0])
            }
            CollectionMethod::IterForEach => {
                Self::expect_arg_count("for_each", 1, args)?;
                self.eval_iter_for_each(iter_val, &args[0])
            }
            CollectionMethod::IterCollect => {
                Self::expect_arg_count("collect", 0, args)?;
                self.eval_iter_collect(iter_val)
            }
            CollectionMethod::IterLast => {
                Self::expect_arg_count("last", 0, args)?;
                self.eval_iter_last(iter_val)
            }
            CollectionMethod::IterRFind => {
                Self::expect_arg_count("rfind", 1, args)?;
                self.eval_iter_rfind(iter_val, &args[0])
            }
            CollectionMethod::IterRFold => {
                Self::expect_arg_count("rfold", 2, args)?;
                self.eval_iter_rfold(iter_val, args[0].clone(), &args[1])
            }
            _ => unreachable!("non-iterator CollectionMethod in eval_iterator_method"),
        }
    }

    // ── Adapter constructors ────────────────────────────────────────────

    /// Create a `Mapped` adapter iterator.
    fn make_mapped(iter_val: IteratorValue, transform: Value) -> Value {
        Value::iterator(IteratorValue::Mapped {
            source: Box::new(iter_val),
            transform: Box::new(transform),
        })
    }

    /// Create a `Filtered` adapter iterator.
    fn make_filtered(iter_val: IteratorValue, predicate: Value) -> Value {
        Value::iterator(IteratorValue::Filtered {
            source: Box::new(iter_val),
            predicate: Box::new(predicate),
        })
    }

    /// Create a `TakeN` adapter iterator.
    fn make_take(iter_val: IteratorValue, count_val: &Value) -> EvalResult {
        let count = Self::extract_usize(count_val, "take")?;
        Ok(Value::iterator(IteratorValue::TakeN {
            source: Box::new(iter_val),
            remaining: count,
        }))
    }

    /// Create a `SkipN` adapter iterator.
    fn make_skip(iter_val: IteratorValue, count_val: &Value) -> EvalResult {
        let count = Self::extract_usize(count_val, "skip")?;
        Ok(Value::iterator(IteratorValue::SkipN {
            source: Box::new(iter_val),
            remaining: count,
        }))
    }

    /// Create an `Enumerated` adapter iterator.
    fn make_enumerated(iter_val: IteratorValue) -> Value {
        Value::iterator(IteratorValue::Enumerated {
            source: Box::new(iter_val),
            index: 0,
        })
    }

    /// Create a `Zipped` adapter iterator from two iterators.
    fn make_zipped(iter_val: IteratorValue, other: &Value) -> EvalResult {
        let Value::Iterator(other_iter) = other else {
            return Err(wrong_arg_type("zip", "Iterator").into());
        };
        Ok(Value::iterator(IteratorValue::Zipped {
            left: Box::new(iter_val),
            right: Box::new(other_iter.clone()),
        }))
    }

    /// Create a `Chained` adapter iterator from two iterators.
    fn make_chained(iter_val: IteratorValue, other: &Value) -> EvalResult {
        let Value::Iterator(other_iter) = other else {
            return Err(wrong_arg_type("chain", "Iterator").into());
        };
        Ok(Value::iterator(IteratorValue::Chained {
            first: Box::new(iter_val),
            second: Box::new(other_iter.clone()),
            first_done: false,
        }))
    }

    /// Create a `Flattened` adapter iterator.
    fn make_flattened(iter_val: IteratorValue) -> Value {
        Value::iterator(IteratorValue::Flattened {
            source: Box::new(iter_val),
            inner: None,
        })
    }

    /// Create a flat-mapped adapter: `Flattened { source: Mapped { source, transform } }`.
    ///
    /// `flat_map(f)` desugars to `.map(f).flatten()` — no separate variant needed.
    fn make_flat_mapped(iter_val: IteratorValue, transform: Value) -> Value {
        Value::iterator(IteratorValue::Flattened {
            source: Box::new(IteratorValue::Mapped {
                source: Box::new(iter_val),
                transform: Box::new(transform),
            }),
            inner: None,
        })
    }

    /// Create a `Reversed` adapter iterator (requires double-ended source).
    fn make_reversed(iter_val: IteratorValue) -> EvalResult {
        if !iter_val.is_double_ended() {
            return Err(wrong_arg_type("rev", "double-ended iterator").into());
        }
        Ok(Value::iterator(IteratorValue::Reversed {
            source: Box::new(iter_val),
        }))
    }

    /// Create a `Cycled` adapter iterator.
    fn make_cycled(iter_val: IteratorValue) -> Value {
        Value::iterator(IteratorValue::Cycled {
            source: Some(Box::new(iter_val)),
            buffer: Vec::new(),
            buf_pos: 0,
        })
    }

    /// Extract a non-negative integer from a Value for take/skip.
    fn extract_usize(val: &Value, method: &str) -> Result<usize, ControlAction> {
        match val {
            Value::Int(n) => {
                let n = n.raw();
                usize::try_from(n).map_err(|_| wrong_arg_type(method, "non-negative int").into())
            }
            _ => Err(wrong_arg_type(method, "int").into()),
        }
    }
}

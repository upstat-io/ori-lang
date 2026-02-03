//! Method dispatch for Ordering type.

use super::compare::{extract_ordering, ordering_to_value};
use super::helpers::require_args;
use ori_ir::StringInterner;
use ori_patterns::{no_such_method, EvalError, EvalResult, OrderingValue, Value};

/// Dispatch methods on Ordering values.
///
/// Implements the methods specified in ordering-type-proposal.md:
/// - `is_less()`, `is_equal()`, `is_greater()` -> `bool`
/// - `is_less_or_equal()`, `is_greater_or_equal()` -> `bool`
/// - `reverse()` -> `Ordering`
/// - `compare()` -> `Ordering` (Comparable trait)
/// - `equals()` -> `bool` (Eq trait)
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_ordering_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    // Extract the OrderingValue from Value::Ordering
    let Some(ord) = extract_ordering(&receiver) else {
        unreachable!("dispatch_ordering_method called with non-ordering receiver")
    };

    match method {
        // Predicate methods
        "is_less" => Ok(Value::Bool(ord == OrderingValue::Less)),
        "is_equal" => Ok(Value::Bool(ord == OrderingValue::Equal)),
        "is_greater" => Ok(Value::Bool(ord == OrderingValue::Greater)),
        "is_less_or_equal" => Ok(Value::Bool(
            ord == OrderingValue::Less || ord == OrderingValue::Equal,
        )),
        "is_greater_or_equal" => Ok(Value::Bool(
            ord == OrderingValue::Greater || ord == OrderingValue::Equal,
        )),

        // Reverse method
        "reverse" => {
            let reversed = match ord {
                OrderingValue::Less => OrderingValue::Greater,
                OrderingValue::Equal => OrderingValue::Equal,
                OrderingValue::Greater => OrderingValue::Less,
            };
            Ok(Value::Ordering(reversed))
        }

        // Clone trait
        "clone" => Ok(Value::Ordering(ord)),

        // Printable and Debug traits (same representation for Ordering)
        "to_str" | "debug" => Ok(Value::string(ord.name())),

        // Hashable trait
        "hash" => {
            let hash_val = match ord {
                OrderingValue::Less => -1i64,
                OrderingValue::Equal => 0i64,
                OrderingValue::Greater => 1i64,
            };
            Ok(Value::Int(hash_val.into()))
        }

        // Eq trait
        "equals" => {
            require_args("equals", 1, args.len())?;
            let Some(other_ord) = extract_ordering(&args[0]) else {
                return Err(EvalError::new("equals requires Ordering value"));
            };
            Ok(Value::Bool(ord == other_ord))
        }

        // Comparable trait: Less < Equal < Greater
        "compare" => {
            require_args("compare", 1, args.len())?;
            let Some(other_ord) = extract_ordering(&args[0]) else {
                return Err(EvalError::new("compare requires Ordering value"));
            };
            // Tags are ordered: Less(0) < Equal(1) < Greater(2)
            Ok(ordering_to_value(
                ord.to_tag().cmp(&other_ord.to_tag()),
                interner,
            ))
        }

        _ => Err(no_such_method(method, "Ordering")),
    }
}

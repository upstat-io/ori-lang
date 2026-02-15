//! Method dispatch for Ordering type.

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalError, EvalResult, OrderingValue, Value};

use super::compare::{extract_ordering, ordering_to_value};
use super::helpers::require_args;
use super::DispatchCtx;

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
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    // Extract the OrderingValue from Value::Ordering
    let Some(ord) = extract_ordering(&receiver) else {
        unreachable!("dispatch_ordering_method called with non-ordering receiver")
    };

    let n = ctx.names;

    // Predicate methods
    if method == n.is_less {
        Ok(Value::Bool(ord == OrderingValue::Less))
    } else if method == n.is_equal {
        Ok(Value::Bool(ord == OrderingValue::Equal))
    } else if method == n.is_greater {
        Ok(Value::Bool(ord == OrderingValue::Greater))
    } else if method == n.is_less_or_equal {
        Ok(Value::Bool(
            ord == OrderingValue::Less || ord == OrderingValue::Equal,
        ))
    } else if method == n.is_greater_or_equal {
        Ok(Value::Bool(
            ord == OrderingValue::Greater || ord == OrderingValue::Equal,
        ))
    // Reverse method
    } else if method == n.reverse {
        let reversed = match ord {
            OrderingValue::Less => OrderingValue::Greater,
            OrderingValue::Equal => OrderingValue::Equal,
            OrderingValue::Greater => OrderingValue::Less,
        };
        Ok(Value::Ordering(reversed))
    // Clone trait
    } else if method == n.clone_ {
        Ok(Value::Ordering(ord))
    // Printable and Debug traits (same representation for Ordering)
    } else if method == n.to_str || method == n.debug {
        Ok(Value::string(ord.name()))
    // Hashable trait
    } else if method == n.hash {
        let hash_val = match ord {
            OrderingValue::Less => -1i64,
            OrderingValue::Equal => 0i64,
            OrderingValue::Greater => 1i64,
        };
        Ok(Value::Int(hash_val.into()))
    // then: lexicographic comparison chaining
    } else if method == n.then {
        require_args("then", 1, args.len())?;
        let Some(other_ord) = extract_ordering(&args[0]) else {
            return Err(EvalError::new("then requires Ordering value").into());
        };
        // If self is Equal, use other; otherwise keep self
        let result = match ord {
            OrderingValue::Equal => other_ord,
            _ => ord,
        };
        Ok(Value::Ordering(result))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let Some(other_ord) = extract_ordering(&args[0]) else {
            return Err(EvalError::new("equals requires Ordering value").into());
        };
        Ok(Value::Bool(ord == other_ord))
    // Comparable trait: Less < Equal < Greater
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let Some(other_ord) = extract_ordering(&args[0]) else {
            return Err(EvalError::new("compare requires Ordering value").into());
        };
        // Tags are ordered: Less(0) < Equal(1) < Greater(2)
        Ok(ordering_to_value(ord.to_tag().cmp(&other_ord.to_tag())))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "Ordering").into())
    }
}

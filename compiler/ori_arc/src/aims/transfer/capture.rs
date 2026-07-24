//! Closure-capture state transfer.

use super::{AccessClass, AimsState, Cardinality, Consumption};

/// Update a captured variable's state for `PartialApply`.
pub(crate) fn capture_state_update(current: &AimsState, closure_state: &AimsState) -> AimsState {
    if current.is_scalar() {
        return *current;
    }
    let mut state = *current;
    state.access = AccessClass::Owned;

    if closure_state.cardinality <= Cardinality::Once {
        if state.consumption < Consumption::Affine {
            state.consumption = Consumption::Affine;
        }
        if state.cardinality < Cardinality::Once {
            state.cardinality = Cardinality::Once;
        }
    } else {
        state.consumption = Consumption::Unrestricted;
        state.cardinality = Cardinality::Many;
    }

    if state.locality < closure_state.locality {
        state.locality = closure_state.locality;
    }

    state.canonicalize();
    state
}

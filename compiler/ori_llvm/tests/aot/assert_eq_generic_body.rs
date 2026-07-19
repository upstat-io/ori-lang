//! AOT tests for the §05 ROOT-B transitive-mono-worklist surface: a generic
//! function whose body calls a trait method (`equals`/`debug`) on its own rigid
//! type param, where the concrete type arrives through an associated-type
//! resolution. Pre-fix the nested call is recorded against rigid/error `T`, so
//! at true AOT (`ori build` -> native binary) `lookup_mono_dispatch` cannot
//! resolve it (`mono_instance_id = None` fallback gap + arg-type fallback
//! structural-matching an `Idx::ERROR` param) and emission aborts with
//! `unresolved function ... missing mono instance?`. The fix (w-0371395b)
//! resolves the nested generic/trait call per concrete instantiation from
//! `Pool.resolutions`. These drive the REAL `ori build` production path.

use crate::util::assert_aot_success;

// `check_eq<T: Eq>` body calls `equals(T)` on its rigid `T`; the concrete `int`
// arrives via `b.first().unwrap()` where `Self.Item = int` (associated type).
// Pre-fix: AOT compile FAIL (`unresolved function 'check_eq' in apply --
// missing mono instance?`). Interp passes (dynamic dispatch).
#[test]
fn assoc_type_resolved_generic_body_call_monos() {
    assert_aot_success(
        include_str!("fixtures/assert_eq_generic_body/assoc_type_assert_eq.ori"),
        "assert_eq_generic_body_assoc_type",
    );
}

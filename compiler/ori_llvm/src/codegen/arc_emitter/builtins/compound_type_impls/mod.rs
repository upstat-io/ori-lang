//! Type-specific compound trait implementations (Option, Result, Tuple, Str, Map).
//!
//! Implements `equals`, `compare`, and `hash` for compound wrapper types
//! by structural recursion into element types via `emit_element_*` dispatch.
//!
//! ## ARC enum convention
//!
//! - **Option**: `{i64 tag, T payload}` — Some=0, None=1
//! - **Result**: `{i64 tag, payload}`   — Ok=0, Err=1
//! - **Tuple**:  `{A, B, ...}`         — flat struct of resolved element types

mod option;
mod result;
mod str_map;
mod tuple;

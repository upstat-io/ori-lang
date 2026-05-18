//! AOT Test Modules
//!
//! Organized by test category following patterns from:
//! - Rust: `tests/run-make/` integration tests
//! - Zig: `test/link/` and `test/standalone/` tests

pub mod aims_burden_alias;
pub mod aims_interactions;
pub mod arc;
pub mod borrow_independence;
pub mod cli;
pub mod closure_drop;
pub mod coalesce;
pub mod codegen;
pub mod collections_ext;
pub mod conversions;
pub mod cow_map_set;
pub mod cross;
pub mod depth;
pub mod derives;
pub mod drop_augment;
pub mod elem_dec_scope;
pub mod empty_container;
pub mod empty_container_empty_list;
pub mod enum_discriminant;
pub mod enum_tagged_ptr;
pub mod enum_zero_payload;
pub mod error_handling;
pub mod fat_matrix;
pub mod fat_ptr_iter;
pub mod ffi_types;
pub mod for_loops;
pub mod for_yield_option;
pub mod formattable;
pub mod generics;
pub mod higher_order;
pub mod inc_symmetry;
pub mod ir_checks;
pub mod ir_quality_attributes;
pub mod ir_quality_block_merge;
pub mod ir_quality_cfg_simplify;
pub mod ir_quality_codegen;
pub mod ir_quality_loops;
pub mod iter_rc_matrix;
pub mod iterator_drop;
pub mod iterators;
pub mod journey_guard;
pub mod linking;
pub mod lto;
pub mod match_alias;
pub mod memory_stress;
pub mod mutations;
pub mod narrowing;
pub mod operators;
pub mod panic;
pub mod patterns;
pub mod poly_lambda_mono;
pub mod rc_matrix;
pub mod recursion;
pub mod recursive_drop;
pub mod repr;
pub mod scoping;
pub mod sets;
pub mod slices;
pub mod spec;
pub mod stress;
pub mod string_sso;
pub mod strings;
pub mod structs;
pub mod traits;
pub mod trmc;
pub mod tuples;
pub mod wasm;
pub mod wrapper_rc_retain;

// Re-export test utilities
pub mod util;

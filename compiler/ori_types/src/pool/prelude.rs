//! Public type-pool construction and transformation surface.

pub use super::collection_surface::{collect_public_collection_types, walk_collection_types};
pub use super::construct::*;
pub use super::descriptor::{TypeDescriptor, VariantDescriptor};
pub use super::re_intern::{
    re_intern_sig, re_intern_sig_with_var_remap, re_intern_type, re_intern_type_with_var_remap,
};
pub use super::substitute::{
    build_finalized_body_type_map, build_impl_mono_body_type_map, build_mono_body_type_map,
    extend_var_subst_with_roots, extract_var_from_types, substitute_in_existing_pool,
    substitute_in_pool, BodyTypeMapSink, MissingSubstitution,
};

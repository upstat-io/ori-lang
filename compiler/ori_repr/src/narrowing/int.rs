//! Integer narrowing passes.

mod collections;
mod fields;

pub(crate) use collections::narrow_collection_elements;
pub(crate) use fields::narrow_struct_fields;

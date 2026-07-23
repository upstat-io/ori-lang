//! Narrowing codegen.

mod implementation;
mod loop_exclusions;
mod struct_fields;

pub(crate) use implementation::narrowed_collection_element_width;

#[cfg(test)]
mod tests;

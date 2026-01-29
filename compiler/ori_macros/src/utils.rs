//! Shared utilities for diagnostic derive macros.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{punctuated::Punctuated, token::Comma, Data, DeriveInput, Field, Fields};

/// Validate that the input is a struct with named fields, returning the fields.
pub fn validate_struct_with_named_fields<'a>(
    input: &'a DeriveInput,
    macro_name: &str,
) -> syn::Result<&'a Punctuated<Field, Comma>> {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok(&fields.named),
            _ => Err(syn::Error::new_spanned(
                input,
                format!("{macro_name} derive only supports structs with named fields"),
            )),
        },
        _ => Err(syn::Error::new_spanned(
            input,
            format!("{macro_name} derive only supports structs"),
        )),
    }
}

/// Generate format arguments for message interpolation from all struct fields.
///
/// Returns a `TokenStream` of the form `field1 = self.field1, field2 = self.field2, ...`
pub fn generate_format_args<'a>(fields: impl Iterator<Item = &'a Field>) -> TokenStream2 {
    let args: Vec<_> = fields
        .filter_map(|f| f.ident.as_ref())
        .map(|name| quote! { #name = self.#name })
        .collect();

    if args.is_empty() {
        quote! {}
    } else {
        quote! { #(#args),* }
    }
}

/// Check if a type is Option<T>.
pub fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

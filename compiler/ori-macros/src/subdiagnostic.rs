//! Subdiagnostic derive macro implementation.
//!
//! Generates `AddToDiagnostic` implementations for additional labels, notes, and suggestions.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields, LitStr};

/// Main entry point for the Subdiagnostic derive macro.
pub fn derive_subdiagnostic(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_subdiagnostic_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_subdiagnostic_impl(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;

    // Get struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "Subdiagnostic derive only supports structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "Subdiagnostic derive only supports structs",
            ))
        }
    };

    // Determine the kind of subdiagnostic from struct-level attributes
    let subdiag_kind = determine_subdiag_kind(input)?;

    // Find the span field
    let span_field = find_span_field(fields.iter())?;

    // Generate the body based on kind
    let body = match subdiag_kind {
        SubdiagKind::Label(msg) => {
            quote! {
                diag.with_secondary_label(self.#span_field, format!(#msg))
            }
        }
        SubdiagKind::Note(msg) => {
            quote! {
                diag.with_note(format!(#msg))
            }
        }
        SubdiagKind::Help(msg) => {
            quote! {
                diag.with_suggestion(format!(#msg))
            }
        }
    };

    Ok(quote! {
        impl #name {
            /// Add this subdiagnostic to an existing Diagnostic.
            pub fn add_to(self, diag: crate::diagnostic::Diagnostic) -> crate::diagnostic::Diagnostic {
                #body
            }
        }
    })
}

enum SubdiagKind {
    Label(LitStr),
    Note(LitStr),
    Help(LitStr),
}

fn determine_subdiag_kind(input: &DeriveInput) -> syn::Result<SubdiagKind> {
    for attr in &input.attrs {
        if attr.path().is_ident("label") {
            let msg: LitStr = attr.parse_args()?;
            return Ok(SubdiagKind::Label(msg));
        }
        if attr.path().is_ident("note") {
            let msg: LitStr = attr.parse_args()?;
            return Ok(SubdiagKind::Note(msg));
        }
        if attr.path().is_ident("help") {
            let msg: LitStr = attr.parse_args()?;
            return Ok(SubdiagKind::Help(msg));
        }
    }

    Err(syn::Error::new_spanned(
        input,
        "missing #[label(...)], #[note(...)], or #[help(...)] attribute",
    ))
}

fn find_span_field<'a>(fields: impl Iterator<Item = &'a Field>) -> syn::Result<syn::Ident> {
    for field in fields {
        // Look for #[primary_span] attribute
        for attr in &field.attrs {
            if attr.path().is_ident("primary_span") {
                return field
                    .ident
                    .clone()
                    .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"));
            }
        }
    }

    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "no field marked with #[primary_span]",
    ))
}

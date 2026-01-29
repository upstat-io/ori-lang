//! Diagnostic derive macro implementation.
//!
//! Generates `IntoDiagnostic` implementations from struct definitions.
//!
//! # Note
//!
//! This macro generates code that references `crate::diagnostic::Diagnostic`.
//! It is designed for use in the `oric` crate which re-exports diagnostic types.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Field, Ident, LitStr};

use crate::utils::{generate_format_args, is_option_type, validate_struct_with_named_fields};

/// Main entry point for the Diagnostic derive macro.
pub fn derive_diagnostic(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);

    match derive_diagnostic_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_diagnostic_impl(input: &syn::DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;

    // Parse #[diag(CODE, "message")] attribute
    let (error_code, message) = parse_diag_attribute(input)?;

    // Get struct fields
    let fields = validate_struct_with_named_fields(input, "Diagnostic")?;

    // Collect fields for format args (needed by multiple generators)
    let field_names: Vec<_> = fields.iter().filter_map(|f| f.ident.as_ref()).collect();

    // Find primary span field
    let primary_span_field = generate_primary_span(&field_names, fields.iter())?;

    // Generate label additions
    let label_additions = generate_label_additions(&field_names, fields.iter())?;

    // Generate note additions
    let note_additions = generate_note_additions(&field_names, fields.iter())?;

    // Generate help additions
    let help_additions = generate_help_additions(&field_names, fields.iter())?;

    // Generate suggestion additions
    let suggestion_additions = generate_suggestion_additions(&field_names, fields.iter())?;

    // Generate the format arguments for message interpolation
    let format_args = generate_format_args(fields.iter());

    // Generate the impl
    Ok(quote! {
        impl #name {
            /// Convert this error into a Diagnostic.
            pub fn into_diagnostic(self) -> crate::diagnostic::Diagnostic {
                use crate::diagnostic::{Diagnostic, ErrorCode, Suggestion, Applicability};

                let message = format!(#message, #format_args);

                let mut diag = Diagnostic::error(ErrorCode::#error_code)
                    .with_message(message);

                // Add primary span label
                #primary_span_field

                // Add other labels
                #(#label_additions)*

                // Add notes
                #(#note_additions)*

                // Add help messages
                #(#help_additions)*

                // Add suggestions
                #(#suggestion_additions)*

                diag
            }
        }

        impl From<#name> for crate::diagnostic::Diagnostic {
            fn from(err: #name) -> Self {
                err.into_diagnostic()
            }
        }
    })
}

/// Parse the #[diag(CODE, "message")] attribute.
fn parse_diag_attribute(input: &syn::DeriveInput) -> syn::Result<(Ident, LitStr)> {
    for attr in &input.attrs {
        if attr.path().is_ident("diag") {
            return attr.parse_args_with(|input: syn::parse::ParseStream| {
                let code: Ident = input.parse()?;
                let _: syn::Token![,] = input.parse()?;
                let message: LitStr = input.parse()?;
                Ok((code, message))
            });
        }
    }

    Err(syn::Error::new_spanned(
        input,
        "missing #[diag(CODE, \"message\")] attribute",
    ))
}

/// Generate code for the primary span field.
fn generate_primary_span<'a>(
    field_names: &[&Ident],
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<TokenStream2> {
    for field in fields {
        for attr in &field.attrs {
            if attr.path().is_ident("primary_span") {
                let field_name = field
                    .ident
                    .as_ref()
                    .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;

                // Check if there's also a label on this field
                let label_msg = get_label_message(field)?;

                // Generate format args for all fields
                let format_args = field_names.iter().map(|name| quote! { #name = self.#name });

                return Ok(if let Some(msg) = label_msg {
                    quote! {
                        diag = diag.with_label(self.#field_name, format!(#msg, #(#format_args),*));
                    }
                } else {
                    quote! {
                        diag = diag.with_label(self.#field_name, "here");
                    }
                });
            }
        }
    }

    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "no field marked with #[primary_span]",
    ))
}

/// Get the label message from a field's #[label("message")] attribute.
fn get_label_message(field: &Field) -> syn::Result<Option<LitStr>> {
    for attr in &field.attrs {
        if attr.path().is_ident("label") {
            let msg: LitStr = attr.parse_args()?;
            return Ok(Some(msg));
        }
    }
    Ok(None)
}

/// Generate code to add labels for non-primary spans.
fn generate_label_additions<'a>(
    field_names: &[&Ident],
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut additions = Vec::new();

    // Generate format args once
    let format_args: Vec<_> = field_names
        .iter()
        .map(|name| quote! { #name = self.#name })
        .collect();

    for field in fields {
        // Skip primary_span fields (already handled)
        let is_primary = field
            .attrs
            .iter()
            .any(|a| a.path().is_ident("primary_span"));
        if is_primary {
            continue;
        }

        for attr in &field.attrs {
            if attr.path().is_ident("label") {
                let field_name = field
                    .ident
                    .as_ref()
                    .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;
                let msg: LitStr = attr.parse_args()?;

                additions.push(quote! {
                    diag = diag.with_secondary_label(self.#field_name, format!(#msg, #(#format_args),*));
                });
            }
        }
    }

    Ok(additions)
}

/// Generate code to add notes.
fn generate_note_additions<'a>(
    field_names: &[&Ident],
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut additions = Vec::new();

    // Generate format args once
    let format_args: Vec<_> = field_names
        .iter()
        .map(|name| quote! { #name = self.#name })
        .collect();

    for field in fields {
        for attr in &field.attrs {
            if attr.path().is_ident("note") {
                let msg: LitStr = attr.parse_args()?;
                additions.push(quote! {
                    diag = diag.with_note(format!(#msg, #(#format_args),*));
                });
            }
        }
    }

    Ok(additions)
}

/// Generate code to add help messages.
fn generate_help_additions<'a>(
    field_names: &[&Ident],
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut additions = Vec::new();

    // Generate format args once
    let format_args: Vec<_> = field_names
        .iter()
        .map(|name| quote! { #name = self.#name })
        .collect();

    for field in fields {
        for attr in &field.attrs {
            if attr.path().is_ident("help") {
                let msg: LitStr = attr.parse_args()?;
                additions.push(quote! {
                    diag = diag.with_suggestion(format!(#msg, #(#format_args),*));
                });
            }
        }
    }

    Ok(additions)
}

/// Generate code to add suggestions.
fn generate_suggestion_additions<'a>(
    field_names: &[&Ident],
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut additions = Vec::new();

    // Generate format args once
    let format_args: Vec<_> = field_names
        .iter()
        .map(|name| quote! { #name = self.#name })
        .collect();

    for field in fields {
        for attr in &field.attrs {
            if attr.path().is_ident("suggestion") {
                let field_name = field
                    .ident
                    .as_ref()
                    .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;

                // Parse suggestion attributes - now propagates errors properly
                let parsed = parse_suggestion_attr(attr)?;

                let msg = &parsed.message;
                let code = &parsed.code;
                let applicability = match parsed.applicability.as_str() {
                    "machine-applicable" => quote! { Applicability::MachineApplicable },
                    "maybe-incorrect" => quote! { Applicability::MaybeIncorrect },
                    "has-placeholders" => quote! { Applicability::HasPlaceholders },
                    _ => quote! { Applicability::Unspecified },
                };

                // Handle Option<Span> fields
                let is_option = is_option_type(&field.ty);

                if is_option {
                    additions.push(quote! {
                        if let Some(span) = self.#field_name {
                            diag = diag.with_structured_suggestion(
                                Suggestion::new(
                                    format!(#msg, #(#format_args),*),
                                    span,
                                    format!(#code, #(#format_args),*),
                                    #applicability,
                                )
                            );
                        }
                    });
                } else {
                    additions.push(quote! {
                        diag = diag.with_structured_suggestion(
                            Suggestion::new(
                                format!(#msg, #(#format_args),*),
                                self.#field_name,
                                format!(#code, #(#format_args),*),
                                #applicability,
                            )
                        );
                    });
                }
            }
        }
    }

    Ok(additions)
}

struct SuggestionParsed {
    message: LitStr,
    code: LitStr,
    applicability: String,
}

/// Parse suggestion attribute, properly propagating errors.
///
/// Supports two formats:
/// - `#[suggestion("message", code = "...", applicability = "...")]`
/// - `#[suggestion("message")]` (code and applicability optional)
fn parse_suggestion_attr(attr: &syn::Attribute) -> syn::Result<SuggestionParsed> {
    // Try parsing as a meta list with named arguments
    let mut message: Option<LitStr> = None;
    let mut code: Option<LitStr> = None;
    let mut applicability = "unspecified".to_string();

    // Parse the attribute arguments
    attr.parse_args_with(|input: syn::parse::ParseStream| {
        // First argument is always the message
        let msg: LitStr = input.parse()?;
        message = Some(msg);

        // Parse optional named arguments
        while input.peek(syn::Token![,]) {
            let _: syn::Token![,] = input.parse()?;

            // Check if we've reached the end
            if input.is_empty() {
                break;
            }

            let ident: Ident = input.parse()?;
            let _: syn::Token![=] = input.parse()?;
            let value: LitStr = input.parse()?;

            match ident.to_string().as_str() {
                "code" => code = Some(value),
                "applicability" => applicability = value.value(),
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("unknown suggestion attribute: `{other}`"),
                    ))
                }
            }
        }

        Ok(())
    })?;

    let message = message
        .ok_or_else(|| syn::Error::new_spanned(attr, "suggestion attribute requires a message"))?;

    let code = code.ok_or_else(|| {
        syn::Error::new_spanned(attr, "suggestion attribute requires `code = \"...\"`")
    })?;

    Ok(SuggestionParsed {
        message,
        code,
        applicability,
    })
}

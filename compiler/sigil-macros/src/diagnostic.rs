//! Diagnostic derive macro implementation.
//!
//! Generates `IntoDiagnostic` implementations from struct definitions.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, DeriveInput, Data, Fields, Field, Ident,
    LitStr,
};

/// Main entry point for the Diagnostic derive macro.
pub fn derive_diagnostic(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_diagnostic_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_diagnostic_impl(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;

    // Parse #[diag(CODE, "message")] attribute
    let (error_code, message) = parse_diag_attribute(input)?;

    // Get struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => return Err(syn::Error::new_spanned(
                input,
                "Diagnostic derive only supports structs with named fields"
            )),
        },
        _ => return Err(syn::Error::new_spanned(
            input,
            "Diagnostic derive only supports structs"
        )),
    };

    // Find primary span field
    let primary_span_field = find_primary_span_field(fields.iter())?;

    // Generate label additions
    let label_additions = generate_label_additions(fields.iter())?;

    // Generate note additions
    let note_additions = generate_note_additions(fields.iter())?;

    // Generate suggestion additions
    let suggestion_additions = generate_suggestion_additions(fields.iter())?;

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
fn parse_diag_attribute(input: &DeriveInput) -> syn::Result<(Ident, LitStr)> {
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
        "missing #[diag(CODE, \"message\")] attribute"
    ))
}

/// Find the field marked with #[primary_span].
fn find_primary_span_field<'a>(
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<TokenStream2> {
    for field in fields {
        for attr in &field.attrs {
            if attr.path().is_ident("primary_span") {
                let field_name = field.ident.as_ref()
                    .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;

                // Check if there's also a label on this field
                let label_msg = get_label_message(field)?;

                return Ok(if let Some(msg) = label_msg {
                    quote! {
                        diag = diag.with_label(self.#field_name, format!(#msg, #field_name = self.#field_name));
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
        "no field marked with #[primary_span]"
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
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut additions = Vec::new();

    for field in fields {
        // Skip primary_span fields (already handled)
        let is_primary = field.attrs.iter().any(|a| a.path().is_ident("primary_span"));
        if is_primary {
            continue;
        }

        for attr in &field.attrs {
            if attr.path().is_ident("label") {
                let field_name = field.ident.as_ref()
                    .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;
                let msg: LitStr = attr.parse_args()?;

                additions.push(quote! {
                    diag = diag.with_secondary_label(self.#field_name, format!(#msg));
                });
            }
        }
    }

    Ok(additions)
}

/// Generate code to add notes.
fn generate_note_additions<'a>(
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut additions = Vec::new();

    for field in fields {
        for attr in &field.attrs {
            if attr.path().is_ident("note") {
                let msg: LitStr = attr.parse_args()?;
                additions.push(quote! {
                    diag = diag.with_note(format!(#msg));
                });
            }
        }
    }

    Ok(additions)
}

/// Generate code to add suggestions.
fn generate_suggestion_additions<'a>(
    fields: impl Iterator<Item = &'a Field>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut additions = Vec::new();

    for field in fields {
        for attr in &field.attrs {
            if attr.path().is_ident("suggestion") {
                let field_name = field.ident.as_ref()
                    .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;

                // Parse suggestion attributes
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
                                    format!(#msg),
                                    span,
                                    format!(#code),
                                    #applicability,
                                )
                            );
                        }
                    });
                } else {
                    additions.push(quote! {
                        diag = diag.with_structured_suggestion(
                            Suggestion::new(
                                format!(#msg),
                                self.#field_name,
                                format!(#code),
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

fn parse_suggestion_attr(attr: &syn::Attribute) -> syn::Result<SuggestionParsed> {
    let mut message = None;
    let mut code = None;
    let mut applicability = "unspecified".to_string();

    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("code") {
            let value: LitStr = meta.value()?.parse()?;
            code = Some(value);
            Ok(())
        } else if meta.path.is_ident("applicability") {
            let value: LitStr = meta.value()?.parse()?;
            applicability = value.value();
            Ok(())
        } else {
            // First argument is the message
            Err(meta.error("unknown suggestion attribute"))
        }
    }).ok();

    // Try parsing as just a string (the message)
    if let Ok(msg) = attr.parse_args::<LitStr>() {
        message = Some(msg);
    }

    // If we still don't have a message, try parsing as a list
    if message.is_none() {
        if let Ok(nested) = attr.meta.require_list() {
            let tokens = &nested.tokens;
            // Parse the first token as a string literal
            let parsed: syn::Result<LitStr> = syn::parse2(tokens.clone());
            if let Ok(msg) = parsed {
                message = Some(msg);
            }
        }
    }

    Ok(SuggestionParsed {
        message: message.unwrap_or_else(|| LitStr::new("", proc_macro2::Span::call_site())),
        code: code.unwrap_or_else(|| LitStr::new("", proc_macro2::Span::call_site())),
        applicability,
    })
}

/// Check if a type is Option<T>.
fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

/// Generate format arguments for message interpolation.
fn generate_format_args<'a>(fields: impl Iterator<Item = &'a Field>) -> TokenStream2 {
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

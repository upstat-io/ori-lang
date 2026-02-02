---
title: "Features Overview"
description: "Ori LSP Design — Feature Implementations"
order: 1
section: "Features"
---

# Features Overview

This section details the implementation of each LSP feature.

## Implementation Status

| Feature | Status | LSP Method |
|---------|--------|------------|
| Diagnostics | ✅ Implemented | `publishDiagnostics` |
| Hover | ✅ Implemented | `textDocument/hover` |
| Go to Definition | ✅ Implemented | `textDocument/definition` |
| Completions | ✅ Implemented | `textDocument/completion` |
| Formatting | ✅ Implemented | `textDocument/formatting` |
| Find References | ⚠ Not Implemented | `textDocument/references` |
| Document Symbols | ⚠ Not Implemented | `textDocument/documentSymbol` |
| Code Actions | ⚠ Not Implemented | `textDocument/codeAction` |
| Semantic Tokens | ⚠ Not Implemented | `textDocument/semanticTokens` |

## Current Implementation

The current implementation uses `tower-lsp` with `DashMap` for document storage. Features access documents directly:

```rust
impl OriLanguageServer {
    fn get_hover_info(&self, uri: &Url, position: Position) -> Option<Hover> {
        // Get document from DashMap
        let doc = self.documents.get(uri)?;
        let module = doc.module.as_ref()?;
        let offset = position_to_offset(&doc.text, position);

        // Find item at position
        for item in &module.items {
            if let Some(info) = self.hover_for_item(item, offset) {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: info,
                    }),
                    range: None,
                });
            }
        }
        None
    }
}
```

## Detailed Documentation

The following pages describe both the current implementation and planned enhancements:

| Feature | Current | Planned |
|---------|---------|---------|
| [Diagnostics](diagnostics.md) | Parse and type errors | Semantic errors, warnings |
| [Formatting](formatting.md) | Full document format | Selection formatting |
| [Hover](hover.md) | Function/type signatures | Expression types, doc comments |

## Future Design: Feature Pattern

The enhanced implementation pattern (not yet implemented) would provide richer analysis:

```rust
// Planned: features/hover.rs
pub fn hover(
    docs: &DocumentManager,
    params: HoverParams,
) -> Option<Hover> {
    // 1. Get document
    let doc = docs.get(&params.text_document.uri)?;

    // 2. Get cached analysis (parse, typecheck)
    let types = doc.types()?;

    // 3. Find relevant info
    let position = params.position;
    let node = doc.ast()?.node_at(position)?;
    let type_info = types.type_of(node)?;

    // 4. Format response
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("```ori\n{}\n```", type_info),
        }),
        range: Some(node.range()),
    })
}
```

## Shared Infrastructure

### Position Mapping

All features need to convert between LSP positions and Ori spans:

```rust
// Shared in lib.rs or a utils module
pub fn lsp_position_to_offset(text: &str, pos: Position) -> usize {
    // ... (see document-sync.md)
}

pub fn offset_to_lsp_position(text: &str, offset: usize) -> Position {
    // ...
}

pub fn span_to_lsp_range(text: &str, span: Span) -> Range {
    Range {
        start: offset_to_lsp_position(text, span.start),
        end: offset_to_lsp_position(text, span.end),
    }
}
```

### Analysis Results

Features share analysis results from the document cache:

```rust
pub struct AnalysisResult {
    /// Parse result (may have errors)
    pub ast: Option<Module>,
    pub parse_errors: Vec<ParseError>,

    /// Type check result (may have errors)
    pub types: Option<TypeContext>,
    pub type_errors: Vec<TypeError>,
}

impl DocumentState {
    pub fn analyze(&mut self) -> &AnalysisResult {
        if self.analysis.is_none() {
            self.analysis = Some(self.run_analysis());
        }
        self.analysis.as_ref().unwrap()
    }

    fn run_analysis(&self) -> AnalysisResult {
        let parse_result = ori_parse::parse(&self.text);

        let (types, type_errors) = if let Some(ref ast) = parse_result.module {
            let check_result = ori_typeck::check(ast);
            (Some(check_result.context), check_result.errors)
        } else {
            (None, vec![])
        };

        AnalysisResult {
            ast: parse_result.module,
            parse_errors: parse_result.errors,
            types,
            type_errors,
        }
    }
}
```

## Error Recovery

Features should handle partial/broken code gracefully:

### Parse Errors

Even with parse errors, we may have a partial AST:

```rust
pub fn hover_with_errors(doc: &DocumentState, pos: Position) -> Option<Hover> {
    let analysis = doc.analyze();

    // Try to find node at position even with errors
    if let Some(ast) = &analysis.ast {
        if let Some(node) = ast.node_at(pos) {
            // Check if we have type info for this node
            if let Some(types) = &analysis.types {
                if let Some(t) = types.type_of(node) {
                    return Some(make_hover(t, node.range()));
                }
            }

            // Fallback: show node kind
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::PlainText,
                    value: format!("{:?}", node.kind()),
                }),
                range: Some(node.range()),
            });
        }
    }

    None
}
```

### Type Errors

Type errors shouldn't prevent hover from working on valid expressions:

```rust
// If type checking fails partway through, we still have
// types for successfully checked nodes
impl TypeContext {
    pub fn type_of(&self, node: &Node) -> Option<&Type> {
        // Returns type if this node was successfully typed
        self.node_types.get(&node.id())
    }
}
```

## Performance Considerations

### Incremental Analysis

Don't reanalyze unchanged code:

```rust
impl DocumentState {
    pub fn invalidate(&mut self) {
        self.analysis = None;
    }

    pub fn invalidate_types_only(&mut self) {
        if let Some(ref mut analysis) = self.analysis {
            analysis.types = None;
            analysis.type_errors.clear();
        }
    }
}
```

### Lazy Computation

Compute only what's needed:

```rust
// BAD: Always compute everything
pub fn handle_request(&mut self, req: Request) -> Response {
    let analysis = self.full_analysis(); // Always runs all phases
    // ...
}

// GOOD: Compute only what's needed
pub fn handle_hover(&mut self, params: HoverParams) -> Option<Hover> {
    let doc = self.docs.get(&params.uri)?;

    // Only parse if needed
    let ast = doc.ensure_parsed()?;

    // Only typecheck if we need type info
    let node = ast.node_at(params.position)?;
    let types = doc.ensure_typechecked()?;

    types.type_of(node).map(|t| make_hover(t))
}
```

## Future Features (Phase 2+)

### Go to Definition

Find where a symbol is defined:

```rust
pub fn definition(
    docs: &DocumentManager,
    params: DefinitionParams,
) -> Option<Location> {
    let doc = docs.get(&params.uri)?;
    let node = doc.ast()?.node_at(params.position)?;

    match node {
        Node::Identifier(name) => {
            let def = doc.types()?.definition_of(name)?;
            Some(Location {
                uri: def.file.clone(),
                range: span_to_range(&def.span),
            })
        }
        _ => None,
    }
}
```

### Find References

Find all uses of a symbol:

```rust
pub fn references(
    docs: &DocumentManager,
    params: ReferenceParams,
) -> Vec<Location> {
    let doc = docs.get(&params.uri)?;
    let node = doc.ast()?.node_at(params.position)?;

    let symbol = doc.types()?.symbol_at(node)?;

    // Search all open documents
    docs.all()
        .flat_map(|d| d.references_to(symbol))
        .map(|ref_| Location {
            uri: ref_.file.clone(),
            range: span_to_range(&ref_.span),
        })
        .collect()
}
```

### Completion

Provide completions at cursor:

```rust
pub fn completion(
    docs: &DocumentManager,
    params: CompletionParams,
) -> Vec<CompletionItem> {
    let doc = docs.get(&params.uri)?;

    // Determine completion context
    let context = analyze_completion_context(doc, params.position);

    match context {
        Context::FieldAccess(base_type) => {
            // Complete fields/methods of type
            base_type.fields().chain(base_type.methods())
                .map(|m| CompletionItem {
                    label: m.name.clone(),
                    kind: Some(CompletionItemKind::Field),
                    detail: Some(m.type_.to_string()),
                    ..Default::default()
                })
                .collect()
        }
        Context::TopLevel => {
            // Complete keywords, imports, etc.
            keywords_completions()
        }
        Context::Expression => {
            // Complete in-scope variables
            doc.types()?.in_scope(params.position)
                .map(|(name, ty)| CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::Variable),
                    detail: Some(ty.to_string()),
                    ..Default::default()
                })
                .collect()
        }
    }
}
```

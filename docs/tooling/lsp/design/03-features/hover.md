---
title: "Hover"
description: "Ori LSP Design — Type Information Display"
order: 3
section: "Features"
---

# Hover

Displaying type information and documentation when the user hovers over code.

## Implementation Status

| Feature | Status |
|---------|--------|
| Function signatures | ✅ Implemented |
| Type definitions | ✅ Implemented |
| Variable types | ⚠ Not Implemented |
| Doc comments | ⚠ Not Implemented |
| Expression types | ⚠ Not Implemented |

## Overview

Hover is a **request** from client to server. The server returns information to display in a tooltip.

```
textDocument/hover
    Client ─────────────────────────► Server
           ◄───────────────────────── (Hover)
```

## Current Implementation

> **Limitation**: Hover only works on function and type definitions at the top level. It does not resolve variable references, expressions, field accesses, or any identifiers inside function bodies. Hovering over a variable usage or a function call returns nothing.

The current hover implementation finds items (functions, types) at the cursor position:

```rust
impl OriLanguageServer {
    fn get_hover_info(&self, uri: &Url, position: Position) -> Option<Hover> {
        let doc = self.documents.get(uri)?;
        let module = doc.module.as_ref()?;
        let offset = position_to_offset(&doc.text, position);

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

    fn hover_for_item(&self, item: &Item, offset: usize) -> Option<String> {
        match item {
            Item::Function(fd) => {
                if fd.span.contains(&offset) {
                    Some(self.function_signature(fd))
                } else {
                    None
                }
            }
            Item::TypeDef(td) => {
                if td.span.contains(&offset) {
                    Some(self.type_signature(td))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
```

## Future Design: Enhanced Hover

The planned enhanced implementation would provide richer hover info:

```rust
// Planned: features/hover.rs
pub fn hover(
    docs: &DocumentManager,
    params: HoverParams,
) -> Option<Hover> {
    let uri = &params.text_document.uri;
    let pos = params.position;

    let doc = docs.get(uri)?;
    let analysis = doc.analyze();

    // Find the AST node at the cursor position
    let ast = analysis.ast.as_ref()?;
    let offset = position_to_offset(&doc.text, pos);
    let node = ast.node_at_offset(offset)?;

    // Generate hover content based on node type
    let content = generate_hover_content(node, &analysis)?;

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: Some(span_to_range(&doc.text, node.span())),
    })
}
```

### Node-Specific Hover

```rust
fn generate_hover_content(node: &Node, analysis: &AnalysisResult) -> Option<String> {
    let types = analysis.types.as_ref()?;

    match node {
        // Variable reference: show type
        Node::Identifier(name, span) => {
            let ty = types.type_of_binding(name, *span)?;
            Some(format!("```ori\n{}: {}\n```", name, ty))
        }

        // Function call: show signature
        Node::Call { func, .. } => {
            let sig = types.function_signature(func)?;
            Some(format!("```ori\n{}\n```", sig))
        }

        // Function definition: show full signature with doc
        Node::FunctionDef { name, params, return_type, doc, .. } => {
            let mut content = format!("```ori\n@{} ", name);

            // Parameters
            content.push('(');
            for (i, param) in params.iter().enumerate() {
                if i > 0 { content.push_str(", "); }
                content.push_str(&format!("{}: {}", param.name, param.ty));
            }
            content.push(')');

            // Return type
            if let Some(ret) = return_type {
                content.push_str(&format!(" -> {}", ret));
            }

            content.push_str("\n```");

            // Doc comment
            if let Some(doc) = doc {
                content.push_str("\n\n---\n\n");
                content.push_str(doc);
            }

            Some(content)
        }

        // Type reference: show definition
        Node::TypeRef(name, span) => {
            let def = types.type_definition(name)?;
            Some(format!("```ori\ntype {} = {}\n```", name, def))
        }

        // Field access: show field type
        Node::FieldAccess { field, base_type, .. } => {
            let field_type = types.field_type(base_type, field)?;
            Some(format!("```ori\n{}: {}\n```", field, field_type))
        }

        // Literal: show inferred type
        Node::Literal(lit) => {
            let ty = types.type_of_literal(lit)?;
            Some(format!("```ori\n{}\n```", ty))
        }

        // Pattern in match arm
        Node::Pattern(pat) => {
            let bindings = types.pattern_bindings(pat)?;
            let mut content = String::from("Pattern bindings:\n```ori\n");
            for (name, ty) in bindings {
                content.push_str(&format!("{}: {}\n", name, ty));
            }
            content.push_str("```");
            Some(content)
        }

        _ => None,
    }
}
```

## Hover Content Examples

### Variable

Hovering over `x` in `let y = x + 1`:

```markdown
```ori
x: int
```
```

### Function Call

Hovering over `fetch` in `fetch(url: endpoint)`:

```markdown
```ori
@fetch (url: str) -> Result<str, HttpError> uses Http
```
```

### Function Definition

Hovering over `@calculate`:

```markdown
```ori
@calculate (a: int, b: int) -> int
```

---

Calculates the sum of two integers.

@param a First operand
@param b Second operand
```

### Type Reference

Hovering over `User` in `let u: User`:

```markdown
```ori
type User = {
    id: int,
    name: str,
    email: str,
}
```
```

### Method Call

Hovering over `map` in `items.map(...)`:

```markdown
```ori
@map<U> (self, transform: (T) -> U) -> [U]
```
```

### Generic Type

Hovering over `Option<int>`:

```markdown
```ori
type Option<T> = Some(T) | None
```

Specialized as: `Option<int>`
```

## Finding the Node

LSP provides a position (line, column). We need to find which AST node is there:

```rust
impl Module {
    pub fn node_at_offset(&self, offset: usize) -> Option<&Node> {
        self.walk()
            .filter(|n| n.span().contains(offset))
            .min_by_key(|n| n.span().len())  // Innermost node
    }
}

impl Span {
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }
}
```

## Priority for Overlapping Nodes

When multiple nodes overlap, prefer:
1. Identifiers/names over enclosing expressions
2. Innermost node (smallest span)

```rust
fn best_hover_node<'a>(candidates: impl Iterator<Item = &'a Node>) -> Option<&'a Node> {
    candidates
        .filter(|n| is_hoverable(n))
        .min_by_key(|n| {
            // Prefer identifiers, then by span size
            let priority = match n {
                Node::Identifier(..) => 0,
                Node::TypeRef(..) => 0,
                Node::FieldAccess { .. } => 1,
                _ => 2,
            };
            (priority, n.span().len())
        })
}

fn is_hoverable(node: &Node) -> bool {
    matches!(node,
        Node::Identifier(..) |
        Node::TypeRef(..) |
        Node::FunctionDef { .. } |
        Node::Call { .. } |
        Node::FieldAccess { .. } |
        Node::Literal(..) |
        Node::Pattern(..)
    )
}
```

## Graceful Degradation

When full type info isn't available:

```rust
fn generate_hover_content(node: &Node, analysis: &AnalysisResult) -> Option<String> {
    // Try to get full type information
    if let Some(types) = &analysis.types {
        if let Some(content) = generate_typed_hover(node, types) {
            return Some(content);
        }
    }

    // Fallback: show what we know from syntax alone
    generate_syntax_hover(node)
}

fn generate_syntax_hover(node: &Node) -> Option<String> {
    match node {
        Node::FunctionDef { name, params, return_type, .. } => {
            // Show signature without inferred types
            let params_str = params.iter()
                .map(|p| match &p.ty {
                    Some(t) => format!("{}: {}", p.name, t),
                    None => p.name.clone(),
                })
                .collect::<Vec<_>>()
                .join(", ");

            let ret = return_type.as_ref().map(|t| format!(" -> {}", t)).unwrap_or_default();

            Some(format!("```ori\n@{} ({}){}\n```", name, params_str, ret))
        }

        Node::Identifier(name, _) => {
            // Just show the name
            Some(format!("`{}`", name))
        }

        _ => None,
    }
}
```

## WASM API

```rust
#[wasm_bindgen]
impl WasmLanguageServer {
    pub fn hover(&self, uri: &str, line: u32, character: u32) -> Option<String> {
        let uri = Url::parse(uri).ok()?;
        let params = HoverParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position { line, character },
            work_done_progress_params: Default::default(),
        };

        let result = hover(&self.inner.documents, params)?;

        Some(serde_json::to_string(&result).unwrap())
    }
}
```

## Monaco Integration

```typescript
monaco.languages.registerHoverProvider('ori', {
    provideHover(model, position) {
        const uri = model.uri.toString();
        const hoverJson = server.hover(
            uri,
            position.lineNumber - 1,  // LSP is 0-indexed
            position.column - 1
        );

        if (!hoverJson) return null;

        const hover = JSON.parse(hoverJson);
        return {
            contents: [{
                value: hover.contents.value,
                isTrusted: true,  // Allow markdown
            }],
            range: hover.range ? {
                startLineNumber: hover.range.start.line + 1,
                startColumn: hover.range.start.character + 1,
                endLineNumber: hover.range.end.line + 1,
                endColumn: hover.range.end.character + 1,
            } : undefined,
        };
    }
});
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_variable() {
        let code = "let x: int = 42\nlet y = x + 1";
        let server = test_server(code);

        // Hover over 'x' on line 2
        let hover = server.hover(pos(1, 8)).unwrap();

        assert!(hover.contents.value.contains("x: int"));
    }

    #[test]
    fn test_hover_function_call() {
        let code = "@add (a: int, b: int) -> int = a + b\nlet sum = add(a: 1, b: 2)";
        let server = test_server(code);

        // Hover over 'add' call
        let hover = server.hover(pos(1, 10)).unwrap();

        assert!(hover.contents.value.contains("@add"));
        assert!(hover.contents.value.contains("a: int"));
        assert!(hover.contents.value.contains("-> int"));
    }

    #[test]
    fn test_hover_no_type_info() {
        let code = "let x = unknown_function()";  // Type error
        let server = test_server(code);

        // Should still get some hover for 'x'
        let hover = server.hover(pos(0, 4));

        // May return None or fallback content
        // Depends on error recovery strategy
    }
}
```

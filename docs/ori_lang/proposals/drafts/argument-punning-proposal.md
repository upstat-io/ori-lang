# Proposal: Argument Punning

**Status:** Draft
**Author:** Eric
**Created:** 2026-02-21
**Affects:** Parser, formatter, IDE/LSP
**Depends on:** None

---

## Summary

Allow omitting the value in a named function argument when the argument name matches the variable name being passed. `f(target: target)` can be written as `f(target:)`. This extends the existing struct field punning (`Point { x, y }` for `Point { x: x, y: y }`) to function call arguments.

---

## Motivation

### The `target: target` Problem

Named arguments are one of Ori's best features — they make code self-documenting and eliminate positional-argument bugs. But when the variable name matches the parameter name (which is common in well-named code), the repetition is pure noise:

```ori
// Today: repetitive
nll_loss(input: output, target: target)
conv2d(input: input, weight: weight, bias: bias, stride: stride, padding: padding)
SelfAttention.new(embed_dim: embed_dim, num_heads: num_heads)
```

This is especially painful in ML code where mathematical variables naturally match parameter names. A function that takes `weight`, `bias`, `input`, `target` will almost always be called with variables of the same names.

### Struct Punning Already Exists

Ori already supports this pattern for struct literals:

```ori
// These are equivalent:
Point { x: x, y: y }
Point { x, y }
```

The same logic applies to function arguments — if the value is a variable with the same name as the parameter, the value is redundant information.

### Impact on ML Code

Before:

```ori
let $attn = SelfAttention.new(embed_dim: embed_dim, num_heads: num_heads);
let $loss = nll_loss(input: output, target: target);
let $out = conv2d(input: input, weight: weight, bias: bias, stride: stride, padding: padding);
```

After:

```ori
let $attn = SelfAttention.new(embed_dim:, num_heads:);
let $loss = nll_loss(input: output, target:);
let $out = conv2d(input:, weight:, bias:, stride:, padding:);
```

The non-matching argument (`input: output`) remains explicit. The matching ones collapse to just the name, highlighting which arguments are passed through vs transformed.

---

## Design

### Syntax

When a named argument's value is a variable with the same name as the parameter, the value may be omitted:

```ori
// Full form:
f(name: name, age: age, active: is_active)

// Punned form:
f(name:, age:, active: is_active)
```

The trailing colon distinguishes punned arguments from positional arguments:

- `f(x)` — positional (single-param functions, lambdas)
- `f(x:)` — punned named argument (expands to `f(x: x)`)
- `f(x: expr)` — explicit named argument

### Grammar

```ebnf
(* Updated call_arg — value is optional when name is present *)
call_arg = identifier ":" [ expression ]
         | expression                     (* positional — single-param only *)
         | "..." expression .             (* spread *)
```

### Desugaring

The parser desugars `f(x:)` to `f(x: x)` by creating a synthetic `Expr::Ident` with the argument name. This happens entirely in the parser — the type checker and evaluator see the expanded form and require no changes.

Concretely, in `CallArg`:

```rust
// Current:
pub struct CallArg {
    pub name: Option<Name>,
    pub value: ExprId,
    pub is_spread: bool,
    pub span: Span,
}
```

No IR change needed. When the parser sees `name:` followed by `,` or `)`, it:
1. Creates an `Expr::Ident { name }` in the arena
2. Sets `CallArg { name: Some(name), value: ident_expr_id, ... }`

This is identical to how struct field punning works today.

### Formatting

`ori fmt` canonicalizes to punned form when applicable:

```ori
// Input:
f(name: name, age: age)

// ori fmt output:
f(name:, age:)
```

This matches how `ori fmt` handles struct field punning. The formatter detects when `name == value_ident` and emits the short form.

### Method Calls

Punning works with method calls:

```ori
// Full:
tensor.reshape(shape: shape)
model.forward(input: input)

// Punned:
tensor.reshape(shape:)
model.forward(input:)
```

### Mixed Punned and Explicit

Punning and explicit args can be freely mixed:

```ori
// Some match, some don't:
conv2d(input:, weight:, bias:, stride: 2, padding: 1)
//     ^^^^^^  ^^^^^^^  ^^^^^  explicit    explicit
//     punned  punned   punned
```

This naturally highlights which arguments are "pass-through" (punned) vs "configured" (explicit) — useful visual information.

---

## Examples

### Neural Network Construction

```ori
// Before:
MnistNet {
    conv1: Conv2d.new(in_channels: in_channels, out_channels: 32, kernel_size: kernel_size, stride: stride),
    fc1: Linear.new(in_features: in_features, out_features: out_features),
}

// After:
MnistNet {
    conv1: Conv2d.new(in_channels:, out_channels: 32, kernel_size:, stride:),
    fc1: Linear.new(in_features:, out_features:),
}
```

### Test Assertions

```ori
// Before:
assert_eq(actual: actual, expected: expected)
assert_eq(actual: result, expected: expected)

// After:
assert_eq(actual:, expected:)
assert_eq(actual: result, expected:)
```

### Forward Pass

```ori
// Before:
@forward (self, input: Tensor) -> Tensor = {
    let $input = relu(input: self.conv1.forward(input: input));
    let $input = relu(input: self.conv2.forward(input: input));
    let $input = self.fc1.forward(input: input);
    log_softmax(input: input, dim: 1)
}

// After:
@forward (self, input: Tensor) -> Tensor = {
    let $input = relu(input: self.conv1.forward(input:));
    let $input = relu(input: self.conv2.forward(input:));
    let $input = self.fc1.forward(input:);
    log_softmax(input:, dim: 1)
}
```

### General Ori Code

```ori
// Database query — before:
let $users = db.query(sql: sql, params: params, timeout: timeout);

// After:
let $users = db.query(sql:, params:, timeout:);

// HTTP request — before:
let $response = client.get(url: url, headers: headers);

// After:
let $response = client.get(url:, headers:);
```

---

## Design Decisions

### Why `f(x:)` with trailing colon, not `f(x)`?

`f(x)` already means "positional argument" for single-parameter functions and lambda variables. Without the colon, the parser cannot distinguish:

```ori
f(x)   // Is this positional arg `x` or punned named arg `x: x`?
```

The trailing colon makes it unambiguous:

- `f(x)` — positional
- `f(x:)` — punned named

### Why not `f(.x)` (Swift style)?

Swift uses `.x` for enum member shorthand. Ori could use a similar prefix, but:

1. `.x` already means field access in Ori
2. The colon is already associated with named arguments (`x: value`)
3. `x:` reads naturally as "x is..." with the value implied

### Why auto-format to punned form?

The punned form is strictly more information-dense with no loss of clarity. When `name` and `value` are identical, showing both is redundant. The formatter enforces consistency — same rationale as struct field punning auto-formatting.

### Does this encourage matching variable names to parameter names?

Yes, and that's a feature. Code where `input` is passed as `input:`, `target` as `target:`, and `weight` as `weight:` is more readable than code using arbitrary abbreviations. Named arguments already push toward this; punning rewards it.

---

## Interaction with Existing Features

| Feature | Impact |
|---------|--------|
| Struct field punning | Same mechanism, extended to call args |
| Positional args (single-param) | Unchanged — `f(x)` is still positional |
| Spread args | Unchanged — `f(...list)` is still spread |
| Default parameters | Compatible — `f(x:)` passes `x`, omitting `y` uses default |
| Variadic args | Not applicable — variadics are positional |
| Lambda shorthand | Unchanged — `list.map(x -> x + 1)` |

---

## Implementation

| Layer | Change |
|-------|--------|
| **Parser** | In call argument parsing: when `name:` is followed by `,` or `)`, create synthetic `Expr::Ident` |
| **IR** | No change — `CallArg` already holds `name: Option<Name>` + `value: ExprId` |
| **Type checker** | No change — sees expanded form |
| **Evaluator** | No change — sees expanded form |
| **LLVM** | No change — sees expanded form |
| **Formatter** | Detect `name == value_ident` and emit `name:` form |
| **LSP** | Autocomplete: suggest `param:` when variable matching param name is in scope |

Estimated scope: ~50-100 lines of parser change, ~30 lines of formatter change.

---

## Verification

1. `f(x:)` parses identically to `f(x: x)` — same AST after desugaring
2. `f(x:, y: 42)` — mixed punned and explicit works
3. `f(x)` — single-param positional unchanged (no regression)
4. `Point { x, y }` — struct punning unchanged (no regression)
5. `ori fmt` canonicalizes `f(x: x)` → `f(x:)`
6. `ori fmt` preserves `f(x: other)` — no punning when names differ
7. Error: `f(x:)` when `x` is not in scope → "cannot find value `x`"
8. Error: `f(x:)` when function has no param named `x` → existing "unknown parameter" error

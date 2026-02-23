# Proposal: Pipe Operator

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-25
**Rejected:** 2026-01-28
**Reconsidered:** 2026-02-21
**Approved:** 2026-02-21
**Affects:** Lexer, parser, IR, type checker, formatter
**Depends on:** None (named arguments already exist; argument punning and operator traits are orthogonal)

---

## Errata: Why This Rejection Was Reconsidered

The original rejection (2026-01-28) stated:

> "The pipe operator solves a problem that Ori doesn't have. Ori already provides multiple mechanisms for readable data transformation chains: method chaining, extension methods, and `run` blocks."

This was evaluated in the context of **collection processing**, where method chaining genuinely covers the use case. The rejection was correct for that narrow scope.

However, the rejection did not consider three domains where method chaining fundamentally does not work:

### 1. ML / Neural Network Pipelines

Neural network forward passes chain through **heterogeneous receivers** — each step calls a method on a different object:

```ori
// Method chaining doesn't work here — each step is a different object
let $x = self.conv1.forward(input: x);
let $x = relu(input: x);
let $x = self.conv2.forward(input: x);
let $x = max_pool2d(input: x, kernel_size: 2);
let $x = self.dropout.forward(input: x);
let $x = x.flatten(start_dim: 1);
let $x = self.fc1.forward(input: x);
```

The `extend` workaround — "wrap every free function as an extension method on Tensor" — moves boilerplate instead of eliminating it.

### 2. Data Processing Pipelines

Data science code is structurally a pipeline of free functions from different modules. Let-chains create `data: data` repetition on every line.

### 3. Cross-Module Function Composition

Combining functions from different modules that weren't designed to chain requires either deep nesting (inside-out reading) or verbose let-chains.

### Summary

The original rejection evaluated pipe against collection chaining and correctly found it redundant there. But Ori's ambition now extends to ML, data science, and cross-module composition — domains where data flows through **free functions and methods on different objects**. Neither method chaining nor `extend` provides a clean solution.

The revised design uses **implicit fill** — a delegation-style mechanism that is more consistent with how Ori already works.

---

## Summary

Add a pipe operator `|>` for left-to-right function composition. The piped value automatically fills the single unspecified parameter — no placeholder needed. This follows the same principle as delegation in other languages: pass the function name, and the system wires the arguments based on what's already specified.

```ori
// Current (nested calls)
sum(items: filter(predicate: x -> x > 0, over: map(transform: x -> x * 2, over: data)))

// With pipe
data
    |> map(transform: x -> x * 2)
    |> filter(predicate: x -> x > 0)
    |> sum
```

---

## Motivation

### The Problem

Data transformations often chain multiple operations. Currently this requires either:

**Nested calls (inside-out reading):**
```ori
let $result = join(
    separator: ", ",
    items: map(
        transform: u -> u.name,
        over: filter(
            predicate: u -> u.active,
            over: users,
        ),
    ),
)
```

**Let-chain (always works but verbose):**
```ori
let $data = users;
let $data = filter(predicate: u -> u.active, over: data);
let $data = map(transform: u -> u.name, over: data);
let $result = join(separator: ", ", items: data);
```

**With pipe:**
```ori
let $result = users
    |> filter(predicate: u -> u.active)
    |> map(transform: u -> u.name)
    |> join(separator: ", ")
```

### Prior Art

| Language | Syntax | Pipe Target | Notes |
|----------|--------|-------------|-------|
| Elixir | `data \|> func()` | First arg (positional) | Works because Elixir uses positional args |
| F# | `data \|> func` | Last arg (positional) | Works because F# uses currying |
| Gleam | `data \|> func()` | First arg (positional) | Labelled args specified, unlabelled piped |
| Hack | `data \|> func($$)` | Explicit placeholder | Most explicit |
| **Ori** | `data \|> func(other: v)` | **Single unspecified param** | Named args make this unambiguous |

Ori's approach is **more precise** than all of the above. Elixir/F# guess by position. Hack requires a noisy placeholder. Ori uses what it already knows — the named arguments — to determine exactly which parameter the pipe fills. No guessing, no noise.

---

## Design

### Core Rule: Implicit Fill

When `|>` pipes a value into a function call, the piped value fills the **single unspecified parameter**:

```ori
// relu has one param: (input: Tensor) -> Tensor
x |> relu
// Compiler fills: relu(input: x)

// max_pool2d has two params: (input: Tensor, kernel_size: int) -> Tensor
x |> max_pool2d(kernel_size: 2)
// Compiler fills: max_pool2d(input: x, kernel_size: 2)

// log_softmax has two params: (input: Tensor, dim: int) -> Tensor
x |> log_softmax(dim: -1)
// Compiler fills: log_softmax(input: x, dim: -1)
```

The compiler knows exactly which parameter to fill because named arguments identify every other parameter. There is **exactly one slot left** — the pipe fills it.

**"Unspecified" definition:** A parameter is _unspecified_ when it is both (a) not provided in the call arguments and (b) has no default value. Parameters with defaults are treated as already filled for pipe purposes.

```ori
// foo has params: (a: int, b: int, c: int = 0) -> int
5 |> foo(b: 3)
// OK: a is unspecified (no default, not in call). c has a default. Fills a.
// Equivalent to: foo(a: 5, b: 3, c: 0)

5 |> foo
// Error: `foo` has 2 unspecified parameters (a, b); pipe can only fill one.
```

**Compile errors:**
- Zero unspecified parameters → "all parameters already specified; nothing for pipe to fill"
- Two or more unspecified parameters → "ambiguous pipe target; specify all parameters except one"

### Why This Is Delegation, Not Magic

This is the same pattern as delegation in C#, Kotlin, and Swift: pass the function name, and the system matches parameters by signature. You don't write `x => Transform(x)` when `Transform` already has the right signature — you just write `Transform`.

Ori already uses this pattern in two places:

1. **Single-param positional calls**: `list.map(x -> x + 1)` doesn't need `transform:` because there's only one parameter.

2. **Default parameters**: `Linear.new(in_features: 128, out_features: 10)` fills `bias: true` by default — you specify what you need, the rest is filled.

Pipe extends this principle: **you specify the arguments you know, and the pipe fills the one remaining slot.** The compiler never guesses — it fills the only empty slot.

### Syntax

```ebnf
pipe_expr = coalesce_expr { "|>" pipe_step } .
pipe_step = "." member_name [ call_args ]
          | postfix_expr [ call_args ]
          | lambda .
```

### Method Calls on the Piped Value

Use `.method()` to call a method on the piped value itself:

```ori
x
    |> .flatten(start_dim: 1)       // x.flatten(start_dim: 1)
    |> .reshape(shape: [b, t, c])   // result.reshape(shape: ...)
```

The leading `.` distinguishes "call a method on the piped value" from "call a free function with the piped value as an argument":

```ori
x |> relu           // relu(input: x) — free function, pipe fills param
x |> .flatten()     // x.flatten() — method on x
```

### Lambda Pipe Steps

For expression-level operations that don't fit the function-call model, use a lambda:

```ori
x |> (a -> a @ weight + bias)    // matmul and add
x |> (a -> a ** 2)                // power
```

The lambda receives the piped value as its parameter and returns the result. This provides full flexibility without introducing a separate placeholder mechanism.

### Precedence

`|>` has lower precedence than all other binary operators:

| Level | Operators |
|-------|-----------|
| ... | ... |
| 15 | `??` |
| **16** | **`\|>`** |

```ori
// Parsed as: (a + b) |> process
a + b |> process
```

### Associativity

Left-to-right:

```ori
a |> f |> g |> h
// Equivalent to: h(g(f(a)))
```

---

## Examples

### Neural Network Forward Pass

```ori
impl Module for MnistNet {
    @forward (self, x: Tensor) -> Tensor =
        x
        |> self.conv1.forward
        |> relu
        |> self.conv2.forward
        |> relu
        |> max_pool2d(kernel_size: 2)
        |> self.dropout1.forward
        |> .flatten(start_dim: 1)
        |> self.fc1.forward
        |> relu
        |> self.dropout2.forward
        |> self.fc2.forward
        |> log_softmax(dim: 1)
}
```

Compare with Python:

```python
def forward(self, x):
    x = self.conv1(x)
    x = F.relu(x)
    x = self.conv2(x)
    x = F.relu(x)
    x = F.max_pool2d(x, 2)
    x = self.dropout1(x)
    x = torch.flatten(x, 1)
    x = self.fc1(x)
    x = F.relu(x)
    x = self.dropout2(x)
    x = self.fc2(x)
    output = F.log_softmax(x, dim=1)
    return output
```

Ori is **cleaner**: no `x = ` repetition, no `F.` prefixes, no `return`, and the pipeline structure is explicit rather than implied by sequential mutation.

### Data Processing Pipeline

```ori
let $report = load_csv(path: "transactions.csv")
    |> filter(predicate: t -> t.date >= start_date)
    |> group_by(key: t -> t.category)
    |> map(transform: (cat, txns) -> {
        let $total = txns
            |> map(transform: t -> t.amount)
            |> sum;
        CategoryTotal { category: cat, total }
    })
    |> sort_by(key: c -> c.total, descending: true);
```

### Cross-Module Composition

```ori
let $token = raw_input
    |> json_parse
    |> .to_bytes()
    |> hash_sha256
    |> base64_encode;
```

### String Processing

```ori
let $slug = title
    |> .to_lower()
    |> .trim()
    |> .replace(pattern: " ", with: "-")
    |> .replace(pattern: "[^a-z0-9-]", with: "");
```

### Self-Attention with `@` and `**`

```ori
@forward (self, x: Tensor) -> Tensor = {
    let ($b, $t, $c) = x.shape_3d();
    let $qkv = self.qkv.forward(input: x)
        |> .reshape(shape: [b, t, 3, self.num_heads, self.head_dim])
        |> .permute(dims: [2, 0, 3, 1, 4]);
    let $q = qkv.select(dim: 0, index: 0);
    let $k = qkv.select(dim: 0, index: 1);
    let $v = qkv.select(dim: 0, index: 2);

    softmax(input: q @ k.T * self.head_dim ** -0.5, dim: -1)
        |> (attn -> attn @ v)
        |> .transpose(dim0: 1, dim1: 2)
        |> .reshape(shape: [b, t, c])
        |> self.proj.forward
}
```

### Pipe with Error Propagation

```ori
let $data = read_file(path: "input.csv")?
    |> parse_csv?
    |> validate;
```

The `?` on a pipe step applies to the **result** of the desugared call. `|> parse_csv?` desugars to `{ let $__pipe = ...; parse_csv(input: __pipe)? }` — the `?` propagates the error from the call result, not from the function reference.

---

## Design Rationale

### Why Implicit Fill Instead of Explicit Placeholder?

The original proposal required `_` on every pipe step: `|> relu(input: _)`. This was noisy — every line had a placeholder doing the same obvious thing.

Implicit fill is more consistent with Ori's existing patterns:

| Pattern | How It Works |
|---------|-------------|
| Single-param positional call | `list.map(x -> x + 1)` — one slot, fill it |
| Default parameters | `f(a: 1)` — unspecified params use defaults |
| **Pipe implicit fill** | `x \|> f(b: 2)` — one unspecified param, pipe fills it |

All three say: **when there's exactly one unfilled slot, the answer is obvious.**

The `_` placeholder was actually *less* Ori-like — it introduced a positional concept ("put the value here") into a named-argument language. Implicit fill uses the named arguments themselves to determine the target.

### Why `.method()` for Method Calls?

Without the dot prefix, the parser cannot distinguish:
- `x |> sort` — free function `sort(data: x)`?
- `x |> sort` — method call `x.sort()`?

The dot makes it unambiguous:
- `x |> sort` — free function with implicit fill
- `x |> .sort()` — method on the piped value

### Why Not Just Use Methods / Extend?

Methods require the type to define them. `extend` requires writing wrapper methods for every free function. Pipes work directly with:
- Free functions from any module
- Methods on different objects (heterogeneous chaining)
- Mixed method + free function pipelines

The `extend` answer moves boilerplate from one place to another. Pipe eliminates it.

### Why Lambdas Instead of `_` Placeholder?

For expression-level operations (like `a @ v` or `a ** 2`), the pipe uses a lambda instead of a `_` placeholder:

```ori
x |> (a -> a @ weight)    // instead of: x |> _ @ weight
x |> (a -> a ** 2)         // instead of: x |> _ ** 2
```

This keeps the pipe operator as a single-mechanism feature — implicit fill for function calls, lambdas for everything else. The alternative (a `_` placeholder) would introduce a second, different mode with its own scoping rules, multi-use semantics, and nesting behavior. Lambdas are already well-understood in Ori and compose with all existing features.

---

## Edge Cases

### All Parameters Specified

```ori
5 |> add(a: 1, b: 2)
// Error: all parameters of `add` are specified; nothing for pipe to fill
```

### Multiple Unspecified Parameters

```ori
5 |> add
// Error: `add` has 2 unspecified parameters (a, b); pipe can only fill one.
// Specify all parameters except one: `|> add(a: 3)` or `|> add(b: 3)`
```

### Zero-Parameter Function

```ori
5 |> get_value
// Error: `get_value` takes no parameters; nothing for pipe to fill
```

### Default Parameters

```ori
// foo has params: (a: int, b: int, c: int = 0) -> int
5 |> foo(b: 3)
// OK: a is unspecified (no value, no default). c has a default.
// Fills a. Equivalent to: foo(a: 5, b: 3)

// bar has params: (a: int = 0, b: int = 0) -> int
5 |> bar
// Error: zero params without defaults; nothing for pipe to fill

5 |> bar(b: 3)
// Error: zero params without defaults; a has a default, b is specified
```

### Method vs Free Function Ambiguity

```ori
x |> sort          // Calls free function sort(data: x)
x |> .sort()       // Calls method x.sort()
```

These are always distinguishable by the leading dot.

### Nested Pipes

```ori
a |> f(x: b |> g)
// Equivalent to: f(x: g(b), <piped>: a)
// where <piped> is the single unspecified param of f besides x
```

### Pipe with Argument Punning

Pipe and argument punning are orthogonal and compose naturally:

```ori
// Without either:
let $x = conv2d(input: x, weight: weight, bias: bias, stride: 2);

// With punning only:
let $x = conv2d(input: x, weight:, bias:, stride: 2);

// With pipe only:
x |> conv2d(weight: weight, bias: bias, stride: 2)

// With both:
x |> conv2d(weight:, bias:, stride: 2)
```

---

## Implementation

### Desugaring

```ori
expr |> func(arg: val)
// When func has params (input: T, arg: U):
// Desugars to:
{
    let $__pipe = expr;
    func(input: __pipe, arg: val)
}
```

### `.method()` Desugaring

```ori
expr |> .method(arg: val)

// Desugars to:
{
    let $__pipe = expr;
    __pipe.method(arg: val)
}
```

### Lambda Desugaring

```ori
expr |> (x -> x @ weight)

// Desugars to:
{
    let $__pipe = expr;
    (x -> x @ weight)(__pipe)
}
```

### `?` on Pipe Steps

```ori
expr |> parse_csv?

// Desugars to:
{
    let $__pipe = expr;
    parse_csv(input: __pipe)?
}
```

The `?` is postfix on the desugared call result, not on the function name.

### Phases

The parser produces a `Pipe` AST node. The type checker resolves implicit fill by looking up the function signature, identifying the single unspecified parameter, and desugaring to a let-binding + ordinary function call. The evaluator and LLVM codegen see only the desugared form.

| Crate | Change |
|-------|--------|
| `ori_lexer` | Recognize `\|>` as a two-character token |
| `ori_ir` | Add `Pipe` expression variant (LHS expression + pipe step) |
| `ori_parse` | Parse at precedence 16 (below `??`); produce `Pipe` AST node |
| `ori_types` | Resolve implicit fill: identify unspecified param, desugar to let-binding + call |
| `ori_eval` | No change — sees desugared let-binding + call |
| `ori_llvm` | No change — sees desugared let-binding + call |
| `ori_fmt` | Format pipe chains with line-break-per-step |

---

## Interaction with Other Features

| Feature | Interaction |
|---------|------------|
| Named arguments | Implicit fill uses named args to identify the unfilled slot |
| Default parameters | Params with defaults are treated as filled; only no-default params count as "unspecified" |
| Argument punning | Orthogonal — `x \|> f(weight:, bias:, stride: 2)` |
| Method chaining | Complementary — use `.method()` chains when same type, pipe when crossing types/modules |
| `?` operator | Applies to the desugared call result: `x \|> parse? \|> validate` desugars to `parse(input: x)?` |
| `@` matmul | Use lambda: `x \|> (a -> a @ weight)` |
| `**` power | Use lambda: `x \|> (a -> a ** 2)` |
| Lambdas | Pipe step can be a lambda for expression-level operations |

---

## Grammar Changes

```ebnf
pipe_expr    = coalesce_expr { "|>" pipe_step } .
pipe_step    = "." member_name [ call_args ]
             | postfix_expr [ call_args ]
             | lambda .
```

---

## Summary

| Aspect | Design |
|--------|--------|
| Operator | `\|>` |
| Fill mechanism | Implicit — fills the single unspecified parameter (no default, not in call) |
| Method calls | `\|> .method()` — leading dot calls method on piped value |
| Expression fallback | Lambda: `\|> (x -> expr)` |
| Precedence | 16 (lowest binary, below `??`) |
| Associativity | Left-to-right |
| Desugars to | Let-binding + function call (resolved in type checker) |
| IR impact | `Pipe` AST node in `ori_ir`; fully desugared before eval/LLVM |

---

## Verification

1. `5 \|> double` evaluates to `double(x: 5)` (single-param)
2. `5 \|> add(b: 3)` evaluates to `add(a: 5, b: 3)` (fills unspecified `a`)
3. Chains: `5 \|> double \|> square` evaluates to `square(double(5))`
4. Method call: `"hi" \|> .to_upper()` evaluates to `"HI"`
5. Error: `5 \|> add` → "2 unspecified parameters, pipe can only fill one"
6. Error: `5 \|> add(a: 1, b: 2)` → "all parameters specified, nothing for pipe to fill"
7. Nested: `a \|> f(x: b \|> g)` → `f(x: g(b), <remaining>: a)`
8. With `?`: `x \|> parse_csv?` desugars to `parse_csv(input: x)?`
9. Lambda: `x \|> (a -> a @ weight)` evaluates to `x @ weight`
10. Formatter: pipe chains break one-step-per-line
11. With punning: `x \|> conv2d(weight:, bias:, stride: 2)` fills `input`
12. Default params: `5 \|> foo(b: 3)` fills `a` when `c` has a default value
13. Error: `5 \|> bar` when all params have defaults → "nothing for pipe to fill"

# Proposal: Optional Method Forwarding (Auto-Deref for Option/Result)

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-02-05
**Affects:** Type system, inference engine, method dispatch

---

## Summary

This proposal explores how to call methods on the inner value of `Option<T>` and `Result<T, E>` without explicit unwrapping or mapping. Three approaches are analyzed: implicit forwarding, optional chaining (`?.`), and the current status quo (`.map()`).

---

## Problem Statement

When working with `Option<T>`, users frequently need to apply a method that exists on `T`, not on `Option` itself. Today this requires `.map()`:

```ori
let name: Option<str> = get_user_name(id: 42)
let upper = name.map(n -> n.to_upper())
let length = name.map(n -> n.len())
```

This is verbose for simple method forwarding, especially when chaining:

```ori
let result = get_config()
    .map(c -> c.database)
    .map(d -> d.host)
    .map(h -> h.to_upper())
```

The same applies to `Result<T, E>`:

```ori
let parsed = parse_int(s: input).map(n -> n.abs())
```

---

## Background

The types_v2 roadmap (§07.5) listed "auto-deref for Option/Result" as deferred. This feature is **not in the spec** and **not in the current roadmap**. This proposal evaluates whether and how to add it.

### Prior Art

| Language | Mechanism | Syntax | Notes |
|----------|-----------|--------|-------|
| Rust | `Deref` trait + auto-deref | `x.method()` | Implicit, compiler-driven |
| Swift | Optional chaining | `x?.method()` | Returns `Optional`, nil-safe |
| Kotlin | Safe call | `x?.method()` | Returns nullable, null-safe |
| C# | Null-conditional | `x?.Method()` | Returns nullable |
| TypeScript | Optional chaining | `x?.method()` | Returns `undefined` if nullish |
| Haskell | Functor/Monad | `fmap f x` or `x >>= f` | Explicit, via type classes |
| Elm | `Maybe.map` | `Maybe.map f x` | Explicit, no operator |
| Gleam | `option.map` | `option.map(x, f)` | Explicit, pipe-friendly |

---

## Approaches

### Approach A: Implicit Forwarding (Rust-style Auto-Deref)

When calling `opt.method()` where `method` doesn't exist on `Option<T>` but exists on `T`, the compiler automatically lifts the call through the wrapper:

```ori
let name: Option<str> = Some("hello")
let upper = name.to_upper()     // Option<str> — implicitly maps
let length = name.len()         // Option<int> — implicitly maps

let config: Result<Config, err> = load_config()
let host = config.database.host // Result<str, err> — chain forwarding
```

**Desugaring:**

```ori
name.to_upper()
// becomes:
name.map(x -> x.to_upper())
```

**Advantages:**
- Minimal syntax — reads like the value is always present
- Method chains stay clean and linear
- Familiar to Rust developers

**Disadvantages:**
- **Violates Ori's explicitness pillar** — reader cannot tell if `name` is `str` or `Option<str>`
- Ambiguous when both `Option<T>` and `T` have a method of the same name (e.g., `.map()`)
- Makes type inference harder — compiler must speculatively resolve methods on inner types
- Error messages become confusing when forwarding fails at depth
- Hides the fact that the value might be `None` — the opposite of Ori's design intent

**Type inference impact:** High. The method resolution algorithm must try the outer type first, then peel wrappers. This interacts poorly with generic type variables.

---

### Approach B: Optional Chaining (`?.`)

Introduce a `?.` operator that explicitly forwards a method call through `Option` or `Result`:

```ori
let name: Option<str> = Some("hello")
let upper = name?.to_upper()     // Option<str>
let length = name?.len()         // Option<int>

// Chaining
let host = get_config()?.database?.host?.to_upper()  // Option<str>
```

**Desugaring:**

```ori
name?.to_upper()
// becomes:
name.map(x -> x.to_upper())
```

For `Result<T, E>`:

```ori
parse_int(s: input)?.abs()
// becomes:
parse_int(s: input).map(x -> x.abs())
```

**Grammar addition:**

```ebnf
postfix_expr = primary_expr { "." ident call_args | "?." ident call_args | "[" expr "]" } .
```

**Advantages:**
- **Explicit** — the `?` signals "this might be absent"
- Well-understood from Swift, Kotlin, TypeScript, C#
- No ambiguity with existing methods
- Composable with existing patterns
- Clear desugaring semantics

**Disadvantages:**
- Potential confusion with `?` error propagation (if Ori adds that later)
- New syntax to learn
- Only saves a few characters over `.map()`
- Still requires understanding that `?.method()` returns `Option<ReturnType>`

**Type inference impact:** Low. The `?.` operator has clear semantics: resolve the inner type, apply the method, re-wrap in the outer container.

---

### Approach C: Status Quo (`.map()` and Friends)

Keep the current approach — users explicitly use `.map()`, `.and_then()`, and `.unwrap_or()`:

```ori
let name: Option<str> = Some("hello")
let upper = name.map(n -> n.to_upper())     // Option<str>
let length = name.map(n -> n.len())         // Option<int>

// Chaining
let host = get_config()
    .and_then(c -> c.database)
    .and_then(d -> d.host)
    .map(h -> h.to_upper())
```

**Advantages:**
- **Already works** — no language changes needed
- **Maximally explicit** — every transformation is visible
- Consistent with Elm, Gleam, and Haskell
- No ambiguity in any context
- Closures enable inline transformations beyond single method calls
- Matches Ori's "no hidden control flow" principle

**Disadvantages:**
- Verbose for simple single-method forwarding
- Deeply nested chains are less readable than `?.` chains
- Lambda boilerplate: `x -> x.method()` is ceremonial

---

## Evaluation Against Design Pillars

| Pillar | Approach A (Implicit) | Approach B (`?.`) | Approach C (Status Quo) |
|--------|----------------------|-------------------|------------------------|
| **Expression-Based** | Neutral | Neutral | Neutral |
| **Explicit Effects** | Violates — hides optionality | Upholds — `?` marks it | Upholds — `.map()` is explicit |
| **ARC-Safe** | Neutral | Neutral | Neutral |
| **Mandatory Verification** | Harder to test — implicit paths | Easy to test | Easy to test |
| **Dependency-Aware** | Neutral | Neutral | Neutral |

---

## Recommendation

**Approach C (Status Quo)** is recommended for the 0.1-alpha release, with **Approach B (`?.`)** as a candidate for future consideration.

### Rationale

1. **Explicitness is a core value.** Ori's design explicitly chose capabilities over implicit effects, `uses` over hidden dependencies, and expression-based over statement-based. Implicit method forwarding (Approach A) directly contradicts this.

2. **The problem is mild.** The verbosity of `.map(x -> x.method())` is a minor ergonomic cost, not a correctness issue. Languages like Elm and Gleam thrive without optional chaining.

3. **`?.` has future potential but risks.** If Ori later adds `?` for error propagation (like Rust), having `?.` for method forwarding creates confusing overlap. The semantics would need careful design to avoid collision.

4. **Pipe operator may subsume this.** If Ori adds a pipe operator (`|>`), it would naturally compose with `.map()`:
   ```ori
   name |> Option.map(str.to_upper)
   ```

### If `?.` is Added Later

The implementation would require:
- Lexer: new `QuestionDot` token
- Parser: `?.` as postfix operator in `postfix_expr`
- Type checker: resolve inner type of `Option<T>` or `Result<T, E>`, check method on `T`, wrap result
- Evaluator: short-circuit on `None`/`Err`, apply method on inner value

This is a self-contained change that does not affect existing code.

---

## Open Questions

1. Should `?.` work on `Result<T, E>` or only `Option<T>`?
2. If `?.` is added, should it also support field access (`config?.host`) or only method calls?
3. How does `?.` interact with a future `?` error propagation operator?
4. Should `.map()` get a shorter alias (e.g., `.then()`) as an alternative?

---

## References

- Types V2 Roadmap §07.5 (deferred item)
- [Swift Optional Chaining](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/optionalchaining/)
- [Kotlin Safe Calls](https://kotlinlang.org/docs/null-safety.html#safe-calls)
- [TypeScript Optional Chaining](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-7.html)
- Ori Spec: `docs/ori_lang/0.1-alpha/spec/07-properties-of-types.md`

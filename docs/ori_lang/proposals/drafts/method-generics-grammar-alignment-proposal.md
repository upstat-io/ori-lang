# Proposal: Method-Generics Grammar Alignment

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-04-24
**Affects:** grammar, spec (Clauses 8, 11, 14), parser, type checker
**Depends On:** none (cites already-approved proposals)

---

## Summary

Add the optional `[ generics ]` production between the identifier and the parameter list in four method-family EBNF rules — `method_sig`, `default_method`, `method`, `def_impl_method` — to align `grammar.ebnf` with already-approved spec prose that describes generic instance methods as an existing language feature. This is grammar/prose drift correction, not a new language feature: the feature is already presumed by object-safety rules, const-generics discussions, and prior approved proposals.

---

## Motivation

Ori's grammar lives in two authoritative places: `compiler_repo/docs/ori_lang/v2026/spec/grammar.ebnf` (formal BNF) and the spec prose (`03-terms-and-definitions.md`, `08-types.md`, approved proposals). When the two disagree, downstream implementations have no single source of truth to honor.

### The Drift

The spec prose already describes generic instance methods as an existing feature:

- `03-terms-and-definitions.md:182` — "A trait is object-safe if none of its methods return `Self`, take `Self` as a non-receiver parameter, **or are generic**."
- `08-types.md:645` — section heading **"Instance methods with const generics"**
- `08-types.md:1005` — example annotated `// NOT object-safe: generic method`
- Approved `object-safety-rules-proposal.md:90` — code example `// NOT object-safe: generic method`
- Approved `object-safety-rules-proposal.md:219` — diagnostic example `note: method 'convert' has generic type parameters`
- Approved `fixed-capacity-list-proposal.md:438` item 6 — **"Allow generic instance methods with const-generic parameters"**

However, `grammar.ebnf` does not carry the production:

```ebnf
/* Current — lines 299, 300, 314, 315 */
method_sig      = "@" identifier params "->" type ";" .
default_method  = "@" identifier params "->" type "=" expression [ ";" ] .
method          = "@" identifier params "->" type [ uses_clause ] "=" expression [ ";" ] .
def_impl_method = "@" identifier params "->" type "=" expression [ ";" ] .
```

Compare with the top-level `function` production (line 246), which DOES carry `[ generics ]`:

```ebnf
function = "@" identifier [ generics ] clause_params "->" type
           [ uses_clause ] [ where_clause ] [ guard_clause ]
           { contract } "=" expression [ ";" ] .
```

### The Problem in Practice

```ori
// Motivating example — fails today with "expected (, found <"
impl<T> Box<T> {
  @map<U> (self, f: T -> U) -> Box<U> = Box(self.value |> f)
}

// Also rejected — trait method-sig with generics
trait Convert {
  @into<U> (self) -> U
}

// Also rejected — default method on trait
trait Foldable {
  @fold<U> (self, init: U, f: (U, Self.Item) -> U) -> U = { ... }
}
```

The parser rejects all three with `expected (, found <`. Yet object-safety prose discusses this exact shape as a category of method (object-unsafe, but legal as a definition).

### When This Matters

- **Generic container methods** — `Box<T>::map<U>`, `List<T>::map<U>` require method-level `U` distinct from the container's `T`
- **Fluent APIs** — `builder.with<T>(value: T)` for typed builder patterns
- **Conversion traits** — `@into<U>` / `@as<U>` style conversions
- **Test fixture** `compiler_repo/compiler/ori_llvm/tests/aot/generics.rs::test_generic_method_on_generic_type` is currently `#[ignore]`-gated waiting for this feature (added with the expectation that it exists per approved prose)

---

## Design

### Grammar Changes

Add `[ generics ]` immediately after `identifier` in four productions. The `generics` non-terminal already exists (`grammar.ebnf:256-259`) and is reused verbatim.

```ebnf
/* Proposed — EBNF only, no semantic change */
method_sig      = "@" identifier [ generics ] params "->" type ";" .
default_method  = "@" identifier [ generics ] params "->" type "=" expression [ ";" ] .
method          = "@" identifier [ generics ] params "->" type [ uses_clause ] "=" expression [ ";" ] .
def_impl_method = "@" identifier [ generics ] params "->" type "=" expression [ ";" ] .
```

No other EBNF production changes. No new tokens, no new non-terminals.

### Semantics

**No new semantic rules are introduced.** Method-level generics compose with existing features per the same rules that already govern top-level function generics:

- **Scope** — a method-level type parameter `U` is in scope from the `<U>` declaration through the method body, shadowing any impl-level parameter with the same name (consistent with `typeck.md §EX-11` scoping).
- **Instantiation** — method-level generics instantiate at the call site via existing type-argument inference (the same `InferEngine` path used for top-level generic function calls).
- **Bounds** — `@map<U: Eq + Clone>` uses the existing `generic_param` production (line 257) with its bounds clause; no bound-syntax changes.
- **Const generics** — `@take<$N: int>` uses the existing `const_param` production (line 259); this is exactly what `fixed-capacity-list-proposal.md §438 item 6` already approved.
- **Object safety** — trait methods carrying generics are object-unsafe per `object-safety-rules-proposal.md`, which already encodes this rule. This proposal does not weaken or change the object-safety check; it only makes the grammar express methods the check was already written to reject.

### Error Handling

No new error codes required. Existing errors apply unchanged:

- **E1xxx parser errors** — the parser already emits `expected (, found <` today; after this proposal, the parser accepts the `<` and proceeds.
- **E2xxx typeck errors** — method-generic scoping, bound satisfaction, and inference failures reuse existing paths (same as top-level function generics).
- **Object-safety violation (E2xxx)** — a trait method with generics in a context requiring object safety continues to emit the existing object-safety diagnostic per `object-safety-rules-proposal.md`.

### Parse-Tree Shape

Parser implementation models method-level generics identically to function-level generics — the same `generics` non-terminal resolves to the same AST node (`ExprArena` allocation), and the method is represented as a method node carrying an optional `generics` field. No new `ExprKind` variants.

---

## Alternatives Considered

### Alternative 1: Reject as feature addition, require new proposal workflow for every use case

**Rejected.** Generic methods are already approved as a language feature via the prose citations above. Treating a grammar-catch-up edit as a novel feature proposal would require N separate proposals for features already approved (one per consuming proposal), and would leave the grammar contradicting the spec prose for the entire review cycle.

### Alternative 2: Modify `/sync-grammar` to own this drift without a proposal

**Rejected.** Per `CLAUDE.md §Spec & Grammar Changes Require Proposal Workflow`, any grammar.ebnf edit goes through the proposal governance gate. `/sync-grammar` is the mechanical tool for applying approved grammar changes, not the governance gate itself. This proposal IS the governance gate; `/sync-grammar` (or a direct edit within `/fix-bug BUG-01-002`) applies it after approval.

### Alternative 3: Disallow generic methods entirely, rewrite prose to match grammar

**Rejected.** This would require:
- Retracting approved `object-safety-rules-proposal.md`
- Retracting approved `fixed-capacity-list-proposal.md` item 6
- Deleting `08-types.md §Instance methods with const generics`
- Forcing every generic-method use case onto top-level functions or extension methods

The net effect is a significant language-surface reduction for no gain — generic methods are standard in the target audience (Rust, Swift, TypeScript, Koka) and prior approval is a stable signal.

### Alternative 4: Narrower proposal — impl methods only (skip `method_sig` and `default_method`)

**Rejected.** Object-safety prose explicitly discusses generic METHODS on TRAITS; the `method_sig` and `default_method` productions are where trait methods are defined. Narrowing the proposal to inherent-impl methods would re-open the spec-prose-vs-grammar drift for trait methods specifically. Fix all four at once for grammar consistency.

---

## Purity Analysis

**Can be pure Ori?** NO — grammar rules cannot be expressed in Ori itself; they are a compiler-surface change.

**If not, why:** EBNF is the formal syntax specification; adjusting it requires recompiling the parser with updated production rules. No amount of stdlib-side work can make the parser accept syntax it does not parse.

**Missing features that would enable purity:** None applicable. Grammar self-modification is not a goal.

**Recommendation:** Proceed as compiler feature (grammar + parser + typeck alignment). This is the smallest possible compiler change — a grammar correction that already has an approved semantic model.

---

## Spec & Grammar Impact

### Grammar

Four production edits to `compiler_repo/docs/ori_lang/v2026/spec/grammar.ebnf`:

| Line | Production | Change |
|------|------------|--------|
| 299 | `method_sig` | insert `[ generics ]` after `identifier` |
| 300 | `default_method` | insert `[ generics ]` after `identifier` |
| 314 | `method` | insert `[ generics ]` after `identifier` |
| 315 | `def_impl_method` | insert `[ generics ]` after `identifier` |

### Spec Prose

No spec prose changes required — existing prose already describes the feature. Minor clarifying edits in `08-types.md` and `11-declarations.md` to add positive examples of generic methods (currently only negative/object-safety examples exist) are optional and can land as a separate sync-docs pass.

### `ori-syntax.md` Quick Reference

Add a bullet to the `.claude/rules/ori-syntax.md` method section documenting method-level generics by example:

```
@method<T>              — method-level generic parameter
@method<T: Eq>          — with bound
@method<$N: int>        — const-generic method parameter
```

### Parser Implementation

The parser's method-header parsing function (in `compiler_repo/compiler/ori_parse/src/grammar/declarations/`) already contains the `parse_generics()` helper used by top-level function parsing. Reusing it at the four method-header call sites is the expected implementation shape; the detailed implementation plan lives in BUG-01-002's fix section after this proposal is approved.

---

## Roadmap Impact

- **Unblocks** `compiler_repo/compiler/ori_llvm/tests/aot/generics.rs::test_generic_method_on_generic_type` — currently `#[ignore]`-gated, resolves once this proposal lands AND BUG-04-091 (codegen gap for inherent generic methods) resolves.
- **Unblocks** `plans/typeck-inference-completeness/section-04-codegen-assertions.md §04.S` ("Bypass-path coverage") — BUG-01-002 is listed as a blocker annotation on §04.S.
- **No new plan** — grammar + parser implementation fits inline in BUG-01-002's `/fix-bug` cycle after proposal approval.

---

## Migration / Breaking Changes

**None.** This is an additive grammar change — existing code with no method-level generics continues to parse identically. No deprecations, no rewrites, no breaking edits to stdlib or test suite.

The existing parser rejection (`expected (, found <`) at method-header position becomes acceptance; any production code intentionally relying on that rejection would be unusual and is not known to exist in the corpus.

---

## Prior Art

### Languages with generic methods on impl/trait methods

| Language | Status | Notes |
|----------|--------|-------|
| **Rust** | Standard since 1.0 | `impl<T> Vec<T> { fn map<U>(self, f: impl Fn(T) -> U) -> Vec<U> { ... } }` — canonical shape |
| **Swift** | Standard | `extension Array { func map<U>(_ transform: (Element) -> U) -> [U] { ... } }` — type-parameter list on method |
| **TypeScript** | Standard | Open issues (#30810, #7391, #41596) center on inference, not grammar |
| **Go** | Not yet | Open proposals #75526, #77273, #64846 — Go is still debating whether to add method-level generics; cited as cautionary about late addition |
| **Koka** | Standard | Effect-typed methods can carry additional type parameters |

### Grammar-vs-parser-drift precedent

Zig PR #14107 ("parser: ensure the documented grammar matches grammar.y") and earlier #1685 (formal grammar), #1729 (parser rewrite to match documented grammar) establish that "documented EBNF disagrees with shipped parser" is a known compiler-maintenance pattern resolved by a grammar-correction PR, not by a language-design debate. This proposal follows the same pattern.

### Approved-proposal references (already in-tree)

| File | Reference | How it presumes generic methods |
|------|-----------|--------------------------------|
| `object-safety-rules-proposal.md` | lines 90, 219 | Defines object-safety rule for "method `convert` has generic type parameters" |
| `fixed-capacity-list-proposal.md` | line 438 item 6 | Enumerates "Allow generic instance methods with const-generic parameters" |
| `compile-time-reflection-proposal.md` | line 595 | Discusses generic-method use with `$for` reflection |
| `structural-trait-defaults-proposal.md` | lines 142, 144, 164, 234 | References generic-method dispatch in the context of structural trait defaults |

---

## Open Questions

1. **Where-clauses on methods** — the top-level `function` production accepts `[ where_clause ]` (line 246) but the four method productions do not. Should this proposal also add `[ where_clause ]` to method productions for parity, or is that a separate alignment question?
   - **Recommendation**: Address in a separate proposal if warranted. This proposal stays strictly scoped to the `[ generics ]` drift surfaced by BUG-01-002. Including where-clauses here would broaden scope beyond the specific spec/prose drift and risk review churn.

2. **Spec-prose positive examples** — existing spec examples are all negative (object-safety violations). Should `08-types.md` and `11-declarations.md` gain positive examples of generic methods as part of proposal approval?
   - **Recommendation**: Yes, but via a follow-up `/sync-docs` pass after this proposal's approval lands the EBNF — the positive examples need the grammar to be valid first.

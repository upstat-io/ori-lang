# Proposal: Method-Generics-and-Where-Clause Grammar Alignment

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-04-24
**Approved:** 2026-04-24
**Updated:** 2026-04-24 — revised twice after two 3-reviewer TPR consensus rounds (codex + gemini + opencode)
**Affects:** grammar, spec (Clauses 8, 11, 14, 20, 27), parser, AST/IR, type checker, object-safety check, Salsa query keys, monomorphization keys
**Depends On:** none (cites already-approved proposals)

---

## Summary

Add optional `[ generics ]` **and** `[ where_clause ]` productions between the identifier and the parameter list in four method-family EBNF rules — `method_sig`, `default_method`, `method`, `def_impl_method` — to align `grammar.ebnf` with already-approved spec prose that uses generic instance methods (with bounds and where-clauses) as an existing language feature. This is grammar-vs-prose drift correction: the feature is already presumed by approved normative spec clauses and approved proposals; only `grammar.ebnf` and the downstream parser/AST/typeck surface have not caught up.

**Scope**: four EBNF productions plus an enumerated list of parser/AST/typeck/registry implementation touchpoints that activate when those productions parse. The proposal does NOT define new semantic rules — scope, instantiation, bounds, and where-clause checking all reuse existing top-level-function paths. It DOES document load-bearing implementation work (AST field additions, object-safety check wire-up, Salsa query-key extension) that was latent behind the grammar gap.

---

## Motivation

Ori's grammar authority splits across `compiler_repo/docs/ori_lang/v2026/spec/grammar.ebnf` (formal BNF) and the spec prose plus approved proposals. When these disagree, downstream implementations have no single source of truth to honor. Today the grammar rejects syntax the normative spec authoritatively writes.

### Normative spec already writes unparseable code

**`27-reflection.md §27.4 (lines 109-117)`** — the authoritative spec declares 5 method-level-generic methods on the non-generic `impl Unknown`:

```ori
impl Unknown {
    @new<T: Reflect> (value: T) -> Unknown;
    // @type_name (self) -> str;         // non-generic methods omitted
    // @type_info (self) -> TypeInfo;    //   for clarity — see spec
    @is<T: Reflect> (self) -> bool;
    @downcast<T: Reflect> (self) -> Option<T>;
    @unwrap<T: Reflect> (self) -> T;
    @unwrap_or<T: Reflect> (self, default: T) -> T;
}
```

§27.4.1 (lines 125, 128) also uses explicit call-site method type-args: `value.is<int>()`, `value.downcast<int>()`.

**`20-capabilities.md §Cache (lines 292-295)`** — a normative trait declares 3 generic methods with bound constraints:

```ori
trait Cache {
    @get<K: Hashable + Eq, V: Clone> (self, key: K) -> Option<V>;
    @set<K: Hashable + Eq, V: Clone> (self, key: K, value: V, ttl: Duration) -> void;
    @invalidate<K: Hashable + Eq> (self, key: K) -> void;
    @clear (self) -> void;
}
```

**`20-capabilities.md §Intrinsics (lines 381-401)`** — 23+ generic trait methods, all carrying `<T, $N: int>`:

```ori
trait Intrinsics {
    @simd_load<T, $N: int> (data: [T], offset: int) -> [T, max N];
    @simd_add<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N];
    @simd_cmpeq<T, $N: int> (a: [T, max N], b: [T, max N]) -> Mask<N>;
    // ... and 20+ more generic methods
}
```

All of the above fails to parse against `grammar.ebnf:299, 300, 314, 315`, where the method-family productions lack the `[ generics ]` slot that the top-level `function` production (line 246) has.

### Approved proposals also presume method generics + where-clauses

| Source | Line | Example | Feature |
|---|---|---|---|
| `object-safety-rules-proposal.md` | 90-93 | `@convert<T> (self) -> T` | Method-level generics |
| `object-safety-rules-proposal.md` | 219 | `note: method 'convert' has generic type parameters` | Object-safety violation diagnostic |
| `fixed-capacity-list-proposal.md` | 348-349, 438 item 6 | `[T].to_fixed<$N: int>()` + "Allow generic instance methods with const-generic parameters" | Method-level const generics |
| `iterator-traits-proposal.md` | 30, 355 | `@from_iter<I: Iterator> (iter: I) -> Self where I.Item == T` | Method generics + where-clause |
| `iterator-traits-proposal.md` | 149, 156 | `@map<U> (self, ...) -> MapIterator<Self, U>`, `@fold<U> (self, initial: U, op: ...) -> U` | Method-level generics |
| `rules/types.md` | 428 | `trait Collect<T> { @from_iter<I: Iterator> (iter: I) -> Self where I.Item == T; }` | Method generics + where-clause at trait level |

All of these fail to parse today.

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

// Also rejected — method generics + where-clause (the Collect<T> shape above)
impl<T> [T] {
  @from_iter<I: Iterator> (iter: I) -> [T] where I.Item == T = /* ... */
}
```

`test_generic_method_on_generic_type` (`compiler/ori_llvm/tests/aot/generics.rs`) is `#[ignore]`-gated, waiting for this proposal + BUG-04-091 (codegen gap).

---

## Design

### Grammar Changes

Four production edits to `compiler_repo/docs/ori_lang/v2026/spec/grammar.ebnf`. The `generics` (line 256-259) and `where_clause` non-terminals already exist and are reused verbatim.

```ebnf
/* Current (lines 299, 300, 314, 315) */
method_sig      = "@" identifier params "->" type ";" .
default_method  = "@" identifier params "->" type "=" expression [ ";" ] .
method          = "@" identifier params "->" type [ uses_clause ] "=" expression [ ";" ] .
def_impl_method = "@" identifier params "->" type "=" expression [ ";" ] .

/* Proposed — 2 optional slots added to each */
method_sig      = "@" identifier [ generics ] params "->" type [ where_clause ] ";" .
default_method  = "@" identifier [ generics ] params "->" type [ where_clause ] "=" expression [ ";" ] .
method          = "@" identifier [ generics ] params "->" type [ uses_clause ] [ where_clause ] "=" expression [ ";" ] .
def_impl_method = "@" identifier [ generics ] params "->" type [ where_clause ] "=" expression [ ";" ] .
```

No other EBNF production changes. No new tokens, no new non-terminals.

**Placement of `where_clause`**: identical to the top-level `function` production (line 246), which places `[ where_clause ]` after `[ uses_clause ]` and before the body. For method productions without a `uses_clause` slot (`method_sig`, `default_method`, `def_impl_method`), `where_clause` appears immediately after the return type.

### Semantics (no new language rules)

**Method-level generics, where-clauses, and their interaction with impl-level generics all reuse existing top-level-function semantics:**

- **Scope**: method-level type parameter `U` is in scope from the `<U>` declaration through the method body and return type, **shadowing any impl-level parameter with the same name** (HM nesting; consistent with `typeck.md §GN-1` rank-based generalization). See §Shadowing Semantics below for spec clarification.
- **Instantiation**: method-level generics instantiate at the call site via existing `InferEngine` type-argument inference paths — the same ones used for top-level generic function calls.
- **Bounds**: `@map<U: Eq + Clone>` uses the existing `generic_param` production with its bounds clause; no bound-syntax changes.
- **Where-clauses**: reuse existing `where_clause` constraint-collection path; constraints are added to `InferEngine` before body-checking.
- **Const generics**: `@take<$N: int>` uses the existing `const_param` production; this is what `fixed-capacity-list-proposal.md §438 item 6` already approves.
- **Object safety**: trait methods carrying generics are object-unsafe per `object-safety-rules-proposal.md`, which already encodes this rule as a latent check (see §Implementation Touchpoints below for wire-up).
- **Same-binder name uniqueness**: a single generic parameter list (e.g. `<T, T>` within one method or one impl) MUST be rejected — standard HM scoping. Name reuse ACROSS binders (impl-level `T` + method-level `T`) is NOT an error; it is shadowing per §Shadowing Semantics below.

### "No new language feature" — but parser/AST/typeck/registry work IS required

The proposal does NOT introduce new semantic rules, but it DOES activate load-bearing implementation work that is currently latent. The following are REQUIRED, not optional:

**AST / IR**

- `TraitMethodSig` (`compiler_repo/compiler/ori_ir/src/ast/items/traits.rs:180`) gains optional `generics: Option<GenericsId>` and `where_clause: Option<WhereClauseId>` fields.
- `TraitDefaultMethod` (`compiler_repo/compiler/ori_ir/src/ast/items/traits.rs:196`) gains the same two fields.
- `ImplMethod` (`compiler_repo/compiler/ori_ir/src/ast/items/traits.rs:303`) gains the same two fields. `DefImplDef` (`:389`) stores `Vec<ImplMethod>`, so that path inherits the field additions through `ImplMethod` — no separate `DefImplMethod` type exists.
- Constructors and spanned implementations updated accordingly.

**Parser**

- Method-header parser accepts `[ generics ]` and `[ where_clause ]` at the new positions. The trait-method path is `compiler_repo/compiler/ori_parse/src/grammar/item/trait_def.rs` (`parse_trait_item` starts at line 74); the impl-method path is `compiler_repo/compiler/ori_parse/src/grammar/item/impl_def/mod.rs` (`parse_impl_method` starts at line 117). Both reuse existing `parse_generics()` (`grammar/item/generics/mod.rs:43`) and `parse_where_clauses()` (`grammar/item/generics/mod.rs:294`) helpers.
- Call-site parsing is scoped separately — see §Call-Site Grammar below.

**Typeck / registry**

- Object-safety check at `compiler_repo/compiler/ori_types/src/check/registration/traits.rs:156-159` gets wired up. The existing inline comment literally says: *"Generic methods — currently trait methods cannot have their own generics (TraitMethodSig has no `generics` field), so this rule cannot be violated. When per-method generics are added to the parser, this check will need to be implemented."* This proposal triggers that wire-up.
- `ImplMethodDef` / `TraitMethodDef` gain scheme metadata comparable to `FunctionSig.scheme_var_ids` (see `ori_types/src/output/mod.rs:373-428`).
- Method-level type/const args participate in monomorphization and Salsa query cache keys — method resolution keys must include the full `(impl_params, method_params)` tuple rather than just impl-level params.
- Duplicate generic names **within a single binder** (e.g. `<T, T>` inside one parameter list) MUST be rejected per standard HM scoping. Cross-binder reuse (impl-level `T` + method-level `T`) is permitted and semantically produces shadowing — see §Shadowing Semantics.

**None of this is new language design** — it is implementation work the grammar gap has been masking.

### Shadowing Semantics

The HM nesting rule: a method-level type parameter named `T` shadows an impl-level parameter of the same name for the duration of the method body. This is defensible (standard HM scope nesting) but **no existing spec clause currently documents it**, since the grammar has rejected the case. Upon approval:

- Add a single sentence to `08-types.md §Method Scoping` (or nearest equivalent): *"If a method's generic parameter list binds a name also bound by the enclosing impl's generic parameter list, the method-level binding shadows the impl-level binding within the method signature and body."*
- `.claude/rules/typeck.md §GN-1` covers rank-based generalization; a cross-reference may help.

### Object-Safety Test Gap

`check/registration/traits.rs:156-159` documents the object-safety rule for generic trait methods but marks it not-yet-implementable. Activating the parser immediately creates a test gap:

- `trait Converter { @convert<T> (self) -> T }` will parse after this proposal's EBNF change.
- The object-safety check MUST correctly reject this trait at a trait-object position (per approved `object-safety-rules-proposal.md`).
- This check has **never been exercised end-to-end** on parsed generic trait methods. Implementation MUST add test coverage — the test gap is the deliverable of the parser+typeck implementation PR, not of this proposal.

### Error Handling

No new error codes required. Existing errors apply unchanged:

- **E1xxx parser errors** — parser accepts `<` after method identifier and proceeds, same way it does for top-level functions.
- **E2xxx typeck errors** — method-generic scoping, bound satisfaction, where-clause checking, and inference failures all reuse existing paths.
- **Object-safety violation (existing code)** — trait method with generics at a context requiring object safety emits the existing `object-safety-rules-proposal.md` diagnostic.

---

## Alternatives Considered

### Alternative 1: Reject as feature addition, require new proposal workflow for every use case

**Rejected.** Generic methods are already approved as a language feature via the 7+ citations above (27-reflection + 20-capabilities Cache + 20-capabilities Intrinsics + object-safety-rules + fixed-capacity-list + iterator-traits + rules/types.md). Treating a grammar-catch-up edit as N separate novel-feature proposals leaves the grammar contradicting spec prose for the entire review cycle.

### Alternative 2: Modify `/sync-grammar` to own this drift without a proposal

**Rejected.** Per `CLAUDE.md §Spec & Grammar Changes Require Proposal Workflow`, any grammar.ebnf edit goes through the proposal governance gate. `/sync-grammar` is the mechanical tool for applying approved grammar changes, not the governance gate itself. This proposal IS the governance gate; `/sync-grammar` (or a direct edit within `/fix-bug BUG-01-002`) applies it after approval.

### Alternative 3: Disallow generic methods entirely, rewrite prose to match grammar

**Rejected.** This would require retracting multiple approved proposals (object-safety-rules, fixed-capacity-list, iterator-traits, compile-time-reflection by extension), rewriting 27-reflection §27.4 and 20-capabilities §Cache/§Intrinsics, and forcing every generic-method use case onto top-level functions or extension methods. The net effect is a significant language-surface reduction for no gain — generic methods are standard in the target audience (Rust, Swift, TypeScript, Koka) and prior approval is a stable signal.

### Alternative 4: Narrower proposal — only `[ generics ]`, defer `[ where_clause ]` to a follow-up

**Rejected after 3-reviewer TPR consensus.** Originally the proposal deferred where-clauses as an "artificial scope split." Reviewers unanimously corrected this:

- `iterator-traits-proposal.md:30, 355` and `rules/types.md §TL-4 line 428` already couple method generics with where-clauses (`@from_iter<I: Iterator> (iter: I) -> Self where I.Item == T`).
- `Collect<T>`, a core trait in the spec, cannot land at approved grammar until BOTH `[ generics ]` AND `[ where_clause ]` land on method productions.
- The four EBNF production lines are identical between the two features. Doing them together is mechanical; splitting guarantees a second near-identical proposal within weeks.
- Top-level `function` (line 246) already carries `[ where_clause ]`. Adding it to method productions is parity restoration, not scope creep.

### Alternative 5: Include call-site grammar (`obj.method<U>(arg)`) in the same proposal

**Deferred with explicit anchor.** See §Call-Site Grammar below. The call-site is a necessary follow-up but is a distinct grammar site (`postfix_expr`, line 444-455) with its own disambiguation concerns (`<` as type-args vs less-than comparison). Keeping this proposal scoped to definition-site grammar lets call-site ambiguity be designed deliberately in its own proposal.

---

## Purity Analysis

**Can be pure Ori?** NO — grammar rules cannot be expressed in Ori itself; they are a compiler-surface change.

**If not, why:** EBNF is the formal syntax specification; adjusting it requires recompiling the parser with updated production rules. No amount of stdlib-side work can make the parser accept syntax it does not parse.

**Recommendation:** Proceed as compiler feature. This is the smallest possible compiler change at the grammar level — 8 optional slots total across 4 productions (each gains both `[ generics ]` and `[ where_clause ]`). Implementation scope is larger but all downstream work is already well-understood existing machinery.

---

## Spec & Grammar Impact

### Grammar

4 production edits to `compiler_repo/docs/ori_lang/v2026/spec/grammar.ebnf`:

| Line | Production | Change |
|------|------------|--------|
| 299 | `method_sig` | insert `[ generics ]` after `identifier`, `[ where_clause ]` after return type |
| 300 | `default_method` | insert `[ generics ]` after `identifier`, `[ where_clause ]` after return type |
| 314 | `method` | insert `[ generics ]` after `identifier`, `[ where_clause ]` after `uses_clause` |
| 315 | `def_impl_method` | insert `[ generics ]` after `identifier`, `[ where_clause ]` after return type |

### Spec Prose

Positive normative examples already exist — no spec prose additions required:

- `27-reflection.md §27.4 (lines 109-117)` — 5 generic methods on `impl Unknown`
- `20-capabilities.md §Cache (lines 292-295)` — 3 generic methods with bounds
- `20-capabilities.md §Intrinsics (lines 381-401)` — 23+ generic methods
- `08-types.md §Instance methods with const generics (line 645)` — section heading + examples
- `08-types.md line 1413, 1417` — method where-clauses

Spec-prose **clarifications** recommended (not load-bearing for approval):

- Add one-sentence shadowing-semantics note in `08-types.md §Method Scoping` (see §Shadowing Semantics above).

### `.claude/rules/ori-syntax.md` Quick Reference

Add bullets to the method-related sections documenting method-level generics + where-clauses by example:

```
@method<T>                — method-level generic parameter
@method<T: Eq>            — with bound
@method<$N: int>          — const-generic method parameter
@method<T> (...) where T: Clone  — method-level generics with where-clause
@method<T> (self, ...) -> T where T: Iterator  — in impl block, method where-clause
```

### Implementation Touchpoints (load-bearing)

Enumerated so the implementation PR has a checklist, not a scavenger hunt:

**AST / IR** (`compiler_repo/compiler/ori_ir/src/ast/items/traits.rs`)
- `TraitMethodSig` (line 180): add `generics: Option<GenericsId>` + `where_clause: Option<WhereClauseId>`
- `TraitDefaultMethod` (line 196): same
- `ImplMethod` (line 303): same. `DefImplDef` (line 389) stores `Vec<ImplMethod>` and inherits the additions automatically — no separate `DefImplMethod` type exists.

**Parser** (`compiler_repo/compiler/ori_parse/src/grammar/item/`)
- Trait-method path `trait_def.rs` (`parse_trait_item` at line 74): accept `[ generics ]` + `[ where_clause ]` in the method-sig + default-method branches.
- Impl-method path `impl_def/mod.rs` (`parse_impl_method` at line 117): accept `[ generics ]` + `[ where_clause ]` at the method-header positions.
- Both paths reuse `parse_generics()` (`generics/mod.rs:43`) and `parse_where_clauses()` (`generics/mod.rs:294`) helpers.
- Add negative test: duplicate names **within a single binder** (e.g. `@f<T, T>`) emit a clear error. Cross-binder reuse (impl `T` + method `T`) is NOT an error — it is shadowing per §Shadowing Semantics.

**Typeck / object-safety check** (`compiler_repo/compiler/ori_types/src/check/registration/traits.rs:156-159`)
- Replace the current inline comment ("currently trait methods cannot have their own generics... when per-method generics are added to the parser, this check will need to be implemented") with active enforcement: reject generic methods at object-safe trait positions, citing `object-safety-rules-proposal.md`.

**Typeck / scheme metadata** (`compiler_repo/compiler/ori_types/src/output/mod.rs:373-428`)
- `ImplMethodDef` and `TraitMethodDef` gain `scheme_var_ids: Vec<SchemeVarId>` comparable to `FunctionSig.scheme_var_ids`.
- Method resolution keys include the nested `(impl_params, method_params)` tuple.

**Salsa query caching**
- Query keys that include method signatures (typeck, canon) already serialize `FunctionSig`; with method signatures gaining generics + where-clause fields, cache keys automatically broaden. No manual invalidation needed, but ensure `Hash` + `Eq` derives cover the new fields.

**Monomorphization**
- Method-level type args join impl-level type args in mono key construction. `compiler_repo/compiler/ori_llvm/` mono code must unify the two binder levels in the order the call site presents them.

**Call-site** (grammar.ebnf:444-455, `ori_parse/src/grammar/expr/postfix.rs:189`) — see §Call-Site Grammar below.

### Verified spec-prose anchors (for reviewer cross-check)

| Source | Line | Anchor type |
|---|---|---|
| `03-terms-and-definitions.md` | 182 | Object-safety definition references generic methods |
| `08-types.md` | 645 | Section heading: "Instance methods with const generics" |
| `08-types.md` | 1005 | Example: `@convert<T> (self) -> T` marked non-object-safe |
| `08-types.md` | 1413, 1417 | Method where-clauses in spec |
| `27-reflection.md` | 109-117 | 5 generic methods on `impl Unknown` (primary evidence) |
| `20-capabilities.md` | 292-295 | Cache trait: 3 generic methods with bounds |
| `20-capabilities.md` | 381-401 | Intrinsics trait: 23+ generic methods |
| `object-safety-rules-proposal.md` | 90-93, 219 | `@convert<T>` + diagnostic |
| `fixed-capacity-list-proposal.md` | 348-349, 438 | Const-generic method examples + item 6 |
| `iterator-traits-proposal.md` | 26, 30, 149, 156, 355 | Method generics + where-clauses |
| `iterator-extended-methods-proposal.md` | 84, 99 | Method where-clauses |
| `cache-pattern-proposal.md` | 248-251 | Cache pattern uses generic methods |
| `rules/types.md §TL-4` | 428, 566 | `Collect<T>::from_iter<I>` method-generics + where-clause |

---

## Call-Site Grammar (dependent follow-up)

The proposal scopes strictly to definition-site grammar. Call-site support (`obj.method<U>(arg)`, `value.downcast<int>()` as used in `27-reflection.md §27.4.1`) requires a separate grammar decision:

- `grammar.ebnf:444-455` (postfix_expr) currently does NOT support call-site type arguments.
- `ori_parse/src/grammar/expr/postfix.rs:189` — after member name, parser only checks for `(`; `<` falls through to comparison.
- Ambiguity: `obj.method<U>(arg)` could parse as either method-call-with-type-args OR `(obj.method < U)(arg)` (less-than + parenthesized call). Disambiguation requires two-token lookahead or a balanced-brackets pre-scan.

**This proposal defers call-site grammar to a follow-up proposal** — the disambiguation concerns are non-trivial and deserve focused design. Without call-site grammar, `value.is<int>()` in `27-reflection.md §27.4.1:125` remains unparseable even after this proposal lands.

Concrete follow-up: open `call-site-method-generics-grammar-alignment-proposal.md` once this proposal approves.

---

## Roadmap Impact

- **Unblocks BUG-01-002** (parser rejection of `@map<U>` in impl blocks).
- **Unblocks `test_generic_method_on_generic_type`** at `compiler_repo/compiler/ori_llvm/tests/aot/generics.rs:557` (currently `#[ignore]`-gated, resolves once this proposal + BUG-04-091 codegen gap both land).
- **Unblocks §04.S** of `plans/typeck-inference-completeness/section-04-codegen-assertions.md` ("Bypass-path coverage") — BUG-01-002 is listed as a blocker annotation.
- **Implementation scope** — grammar + parser + AST + typeck + object-safety-wire-up + monomorphization-keys is larger than a point-fix; `/fix-bug BUG-01-002` after proposal approval may escalate to `/create-plan` for a focused subsection.

---

## Migration / Breaking Changes

**None.** Additive grammar change — existing code with no method-level generics or where-clauses continues to parse identically. No deprecations, no rewrites, no breaking edits to stdlib or test suite.

Existing parser rejections (`expected (, found <`) at method-header position become acceptance. Any production code intentionally relying on that rejection would be unusual and is not known to exist in the corpus.

---

## Prior Art

### Languages with generic methods in impl / trait blocks

| Language | Status | Notes |
|----------|--------|-------|
| **Rust** | Standard since 1.0 | `impl<T> Vec<T> { fn map<U>(self, f: impl Fn(T) -> U) -> Vec<U> { ... } }` |
| **Swift** | Standard | `extension Array { func map<U>(_ transform: (Element) -> U) -> [U] { ... } }` |
| **TypeScript** | Standard | Open issues center on inference/inheritance, not grammar |
| **Koka** | Standard | Effect-typed methods can carry additional type parameters |

### Grammar-vs-parser-drift precedent

- **Zig PR #14107** ("parser: ensure the documented grammar matches grammar.y") and earlier PRs #1685, #1729 establish that "documented EBNF disagrees with shipped parser" is a well-known compiler-maintenance pattern resolved by a grammar-correction PR, not by a language-design debate.

### Approved-proposal cross-references (all verified against file contents)

- `object-safety-rules-proposal.md:90-93, 219` — `@convert<T> (self) -> T` + diagnostic
- `fixed-capacity-list-proposal.md:348-349, 438` — `[T].to_fixed<$N: int>()` + item 6
- `iterator-traits-proposal.md:30, 149, 156, 355` — method generics + where-clauses (line 26's `@iter ... where Item == Self.Item` is an `impl Trait` return-type constraint, not a method where-clause; excluded)
- `iterator-extended-methods-proposal.md:84, 99` — method where-clauses
- `cache-pattern-proposal.md:248-251` — cache pattern uses generic methods

The original draft also cited `compile-time-reflection-proposal.md:595` and `structural-trait-defaults-proposal.md:142,144,164,234`; both have been **removed** after 3-reviewer verification confirmed they describe top-level generic functions and type-level-generic dispatch contexts respectively, not method-level generics.

---

## Open Questions

1. **Call-site grammar** — `obj.method<U>(arg)` requires postfix-grammar updates and two-token lookahead / balanced-brackets disambiguation. Recommendation: defer to a dedicated follow-up proposal (`call-site-method-generics-grammar-alignment-proposal.md`), explicitly opened by this proposal's approval commit. Without that follow-up, `27-reflection.md §27.4.1` examples like `value.is<int>()` remain unparseable.

2. **Shadowing-semantics spec clarification** — method-level `T` shadowing impl-level `T` needs a one-sentence normative statement in `08-types.md §Method Scoping` (or nearest equivalent). Recommendation: land this clarification as part of the implementation PR, co-committed with the grammar change.

3. **Object-safety test-gap** — `check/registration/traits.rs:156-159` has a latent check that activates only after parser support. Recommendation: implementation PR adds both the check wire-up AND test coverage for object-safe-context rejection.

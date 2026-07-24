# Proposal: JavaScript Interop with TypeScript-Driven Bindings

**Status:** Draft (research)
**Author:** Eric (with AI assistance)
**Created:** 2026-04-29
**Scope:** Research-stage; multi-crate — spans grammar, typeck, capability system, AIMS lattice, runtime, FFI, stdlib. Implementation gated on prerequisite drafts landing first.
**Affects:** grammar, type system, capability system, AIMS lattice, runtime, FFI, stdlib
**Depends On:** `capability-propagation-completion-proposal.md`, `negative-effect-without-proposal.md`, `deep-ffi-proposal.md` (approved), `ffi-boundary-safety-proposal.md` (approved)
**Soft dependencies:** `unsafe-operation-gating-proposal.md` (engine-internal `Unsafe` ops only)

---

## Summary

- Ori imports JavaScript libraries via `use js "<package>" { ... }`.
- Type signatures are inferred from the package's TypeScript declaration files (`.d.ts`).
- Calls into JS propagate Ori capability requirements through the boundary.
- Cross-runtime object lifetimes are managed by an 8th orthogonal AIMS dimension `JsRef` (chain: `None < Borrowed < Owned < Shared`, with `Shared` as conservative top); JS-located values force `Locality = HeapEscaping` UNLESS `Locality = Borrowed(p)` from FFI boundary; per-handle obligations tracked per-SSA in `MemoryContract.js_handles: Map<SsaIdx, JsHandleObligation>`.
- An embedded JS engine (QuickJS for v1, pluggable contract for V8/JavaScriptCore) executes the JS portion.
- The boundary is invisible at the Ori source level and compile-time-checked end-to-end.
- Implementation timeline: ~7–9 months JS-interop phases post-prerequisite; ~12–16 months total wall-clock baseline including prerequisite proposal landing.

---

## Motivation

### The interop landscape is empty

| Approach | Compile-time type-check at boundary | Compile-time effect propagation | GC ↔ host-memory bridge | UX |
|---|---|---|---|---|
| napi-rs (Rust ↔ Node) | manual macros (`#[napi]` annotations) | none (runtime checks only) | borrow-checker manages `Env` lifetime; manual N-API ref counting for JS objects | boundary visible everywhere |
| Neon (Rust ↔ Node) | manual, older API | none | manual ref counting | verbose conversion ladders |
| deno_core (Rust ↔ V8) | low-level ops, no static types at boundary | runtime permission system (Deno) | manual | not idiomatic Rust |
| wasm-bindgen (Rust ↔ JS via WASM) | macro-driven, type-restricted | none | opaque handles | always feels like FFI |
| Embind (C++ ↔ JS) | template-driven | none | `shared_ptr`-like helpers | best-in-class for C++ |
| Pyodide (Python ↔ JS) | dynamic | none | proxy objects | smooth UX, dynamic-language tax |
| GraalVM polyglot | runtime-checked via `HostAccess` policy | runtime-checked policy | shared VM | VM tax, no native perf |
| Cloudflare Workers Bindings | runtime config + per-binding TS types | runtime permission per binding | V8 isolate | Cloudflare-specific |
| Kotlin/JS, Scala.js, Fable F# | full type system on JS side | none | runs on JS VM | architectural difference: hosted on JS VM; same-VM not cross-runtime |

The empty bucket: native systems language with separate runtime, **compile-time** type-checked via authoritative TypeScript types, **compile-time** capability-tracked across the boundary, AIMS-managed cross-runtime lifetimes. This proposal occupies that bucket.

### Targeted scope — pure-JS packages first

This proposal targets packages that run on the QuickJS engine without external runtime dependencies — i.e., pure-JS libraries like `lodash`, JSON parsers, regex utilities, validation libraries, cryptography pure-JS implementations. Packages that depend on Node-API runtime (`fs`, `net`, `http`, `process`, `Buffer`, etc.) require Node polyfills the embedded engine does NOT provide in v1. Express, Koa, axios, fastify, and similar Node-runtime-dependent packages are out of scope for v1; they become available after v2 ships a Node polyfill layer (separate proposal). The §Worked Example uses lodash specifically because it is Node-runtime-independent.

### Pain in current cross-language interop

Today's Rust↔Node binding (napi-rs hypothetical):

```rust
#[napi]
pub fn process_users(env: Env, users: JsObject, size: i32) -> Result<JsObject> {
    // Manual JsObject ↔ Rust struct conversion at every call.
    // No capability tracking.
    // Errors as Result<T, napi::Error>, not the language's native error type.
}
```

This proposal:

```ori
use js "lodash" { chunk };

@process_users (users: [User]) -> Result<[[User]], JsError> uses Js = {
    chunk(users, size: 10)?
}
```

- Type-checked from `lodash`'s `.d.ts` (no hand-written binding).
- `Js` capability propagated at every call site.
- `Result<_, JsError>` wrapping per `deep-ffi-proposal.md §Phase 1 Error Protocols` (see §3.3 below).
- Cross-runtime lifetime managed by AIMS (see §4).

### Sponsor / launch relevance

- The web-server use case Ori targets requires npm ecosystem access (validation libraries like `zod`, date utilities like `date-fns`, JSON-schema generators, lodash-style utilities) without paying Bun's "everything runs in JS" tax. v1 scope is pure-JS packages (per §Targeted Scope); Node-runtime-dependent libraries (auth via `passport.js`, ORMs like Prisma, ML inference clients with native `fs`/`net` deps) become available via the v2 Node-polyfill layer (separate proposal).
- The capability-tracked interop angle is publishable research (USENIX Security, POPL).
- Cloudflare Workers, Vercel, Modular have direct interest in capability-sandboxed JS execution.

---

## Design

### §1. Top-Level JS Import Declaration

#### Grammar (matches actual `grammar.ebnf` productions)

A new top-level form `js_import` slots alongside the existing `import | reexport | extension_import` alternation in `source_file` (line 181 of `grammar.ebnf`). The existing productions are unchanged.

```ebnf
(* Amended source_file — js_import added to existing alternation *)
source_file = [ file_attribute ] { import | js_import | reexport | extension_import } { declaration } .

(* New productions *)
js_import      = "use" "js" string_literal js_import_list ";" .
js_import_list = "{" js_import_item { "," js_import_item } "}" .
js_import_item = "default" "as" identifier
               | "*" "as" identifier
               | "type" js_named_identifier [ "as" identifier ]
               | js_named_identifier [ "as" identifier ] .
js_named_identifier = identifier .  (* MUST NOT be the contextual keywords "default", "type", or "*"; checked at parse time *)
                                    (* `* as NS` namespace import: produces an opaque record type whose fields have the
                                     * translated type of the corresponding `.d.ts` export. Field access on the namespace
                                     * record dispatches to the underlying JS-imported binding through the same boundary
                                     * trampoline as direct named imports. *)
```

Production order matters: parser tries `default as` and `* as` and `type ...` BEFORE the bare-identifier case, eliminating the ambiguity where a binding named `default` could parse as a normal import_item.

Disambiguation rule: after `use`, the parser peeks 2 tokens.

- Tokens `identifier="js"` followed by `string_literal` ⟹ parse `js_import`.
- Otherwise ⟹ parse the existing `import` (where `import_path = string_literal | identifier { "." identifier }` already covers `use "lodash"` as a string-literal path; the `use js "lodash"` form is unambiguous because regular `import_path` cannot continue with another `string_literal` after an identifier).

Properties:

- `js` is a context-sensitive keyword (only after `use`, when followed by `string_literal`); outside this position `js` is a normal identifier.
- The 2-token lookahead is bounded and deterministic.
- Existing `import`, `reexport`, `extension_import`, `extern_block`, and `declaration` productions are unchanged.

#### Resolution

| Source form | Resolution rule |
|---|---|
| `use js "lodash" { ... }` | npm package: `node_modules/lodash/package.json` → `types` field → `.d.ts` entry |
| `use js "lodash/chunk" { ... }` | npm subpath: resolves the subpath |
| `use js "./local-types.d.ts" { ... }` | Workspace-relative `.d.ts` |
| `use js "@scope/pkg" { ... }` | Scoped npm package |

- Resolution failures emit `E1500..E1519`.
- Every `use js` import implicitly requires `Js` capability at every call site.

### §2. `.d.ts` → Ori Type Translation

The TypeScript type system is large; a defined subset covers the common case. Constructs outside the subset fall back to `JsAny` with runtime check.

#### Supported subset (v1)

| TypeScript construct | Ori translation | Notes |
|---|---|---|
| `string` | `str` | UTF-16 conversion at boundary; lone surrogates → error |
| `number` | `float` (default) OR `int` (when `.ori-caps.json` declares `"@oriType": "int"`) | See §2.1 below |
| `bigint` | `int` | |
| `boolean` | `bool` | |
| `null` / `undefined` | `Option<T>` | Translation context-sensitive (see §2.2) |
| `T[]`, `Array<T>`, `ReadonlyArray<T>` | `[T]` | Immutability is Ori-default |
| `Record<K, V>` | `{K: V}` (structural record) | |
| `Map<K, V>` | `JsMap<K, V>` (stdlib wrapper preserving JS Map insertion-order + `.get`/`.has`/`.entries` semantics) | TS `Record` is plain-object; TS `Map` preserves order — translating both to `{K: V}` would silently change semantics for order-sensitive consumers |
| `Set<T>` | `Set<T>` | |
| `T \| U` (discriminable union) | Ori sum type | Discriminator detection per §2.3 |
| `T \| U` (non-discriminable) | `JsAny` | Falls back to runtime check |
| `T & U` (intersection) | `JsAny` | No structural intersection in Ori |
| Function type `(x: T) => U` | `(T) -> U uses <Caps>` | See §2.4 (variance + capability propagation) |
| Generic class `class Foo<T>` | `type Foo<T> = ...` opaque | Methods exposed; instantiation per §2.5 |
| `Promise<T>` | `JsPromise<T>` | Awaited via `Suspend` capability (auto-propagated) |
| `interface { ... }` | structural Ori record | Optional fields → `Option<T>` |
| Literal types `"foo" \| "bar"` | sum type with literal variants | |
| Mapped types `{ [K in keyof T]: U }` | `JsAny` | Too dynamic for compile-time translation |
| Conditional types `T extends U ? X : Y` | `JsAny` | Too dynamic |
| Template literal types | `JsAny` with optional runtime regex | |
| `unique symbol` | `JsAny` | |
| `never` (bottom) | `Never` | Ori's bottom type; semantically identical |
| `unknown` (safe top) | `JsAny` | Conservative — matches semantic role; consumer must narrow |
| `void` / `any` | `void` / `JsAny` | TS `void` in return position permits any runtime value (semantically discarded); Ori `void` is the unit type with exactly one value `()`. The boundary thunk discards the JS-returned value when translating `void` returns; no runtime check |

#### §2.1. Number Translation

- Default: `number → float` (preserves f64 semantics).
- Override via `.ori-caps.json` sidecar: `"<symbol>": { "@oriType": { "<param>": "int" } }` declares per-parameter `int` semantics.
- Function-position annotations override per parameter; other positions inherit.
- Bitwise-typed APIs (e.g., `Buffer.readUInt32`) get `int` defaults via the heuristic-known-package list (§3).
- **Boundary validation is REQUIRED for sidecar `int` overrides**: at the call boundary, every `number → int` coercion runs `Number.isFinite(v) ∧ Number.isSafeInteger(v) ∧ v >= ORI_INT_MIN ∧ v <= ORI_INT_MAX`. Failure raises `JsError { name: "RangeError", message: "non-integer or out-of-range number at boundary" }` and the call returns `Result::Err`. JS `number` permits NaN, Infinity, fractional values, and integers outside `Number.MAX_SAFE_INTEGER`; the sidecar opt-in MUST validate at the seam.

```json
{ "lodash.chunk": { "@oriType": { "size": "int" } },
  "express.Application.listen": { "@oriType": { "port": "int" } } }
```

#### §2.2. Null / Undefined → Option

| TypeScript position | Ori translation |
|---|---|
| Return type `T \| null` | `Option<T>` |
| Optional parameter `x?: T` | `Option<T>` parameter |
| Field `x: T \| undefined` | Optional field `Option<T>` |
| `x: T \| null \| undefined` | `Option<T>` (collapsed) |

Distinguishing `null` from `undefined` semantically is rejected — both map to `None`.

#### §2.3. Discriminable Unions

- A union is **discriminable** when every member is an object type sharing a literal-typed property key (e.g., `kind`, `tag`).
- Discriminable unions translate to Ori sum types.
- Non-discriminable unions translate to `JsAny`.
- Detection runs in the typeck `.d.ts` translator.

#### §2.4. Function Types — Variance + Capability Propagation

Function types translate as:

```
(x: T) => U   ⟶   (T) -> U uses <Caps>(fn-name)
```

Where `<Caps>(fn-name)` is determined by:

1. **Sidecar metadata** — `.ori-caps.json` `"<package>.<symbol>": ["Cap1", "Cap2"]` overrides everything.
2. **Heuristic package classification** — built-in for top-N packages (`fs-*` → `FileSystem`, `axios`/`node-fetch` → `Net`, `crypto-*` → `Random,Crypto`).
3. **Default conservative** — `Js + UnknownEffects` (see §3.1).

##### Variance rules

- **Covariant** positions: function return type, `Promise<T>` payload, immutable iterable element, optional field on returned object.
- **Contravariant** positions: function argument, callback parameter. Translation MUST require an exact Ori type at the call site OR emit a compiler-generated boundary thunk that performs Ori-side argument coercion + JS-side call. Implicit widening is rejected.
- **Invariant** positions: mutable container element (`T[]` when JS-side mutation is observable, mutable object field, `Map<K, V>` value), generic instantiation, type alias parameter. Substitution rejected; `JsAny` fallback when an exact type is unavailable.
- **TypeScript method bivariance exception**: TypeScript's `strictFunctionTypes` flag does NOT apply to method signatures (per the [TypeScript 2.6 release notes](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-2-6.html)). `.d.ts` translation MUST detect method/constructor signatures and either (a) emit them as exact-type wrappers, or (b) fall back to `JsAny` for the affected position with diagnostic `W1500: bivariant method signature normalized to JsAny`.
- "Wrap the callsite" precisely means: the translator emits a thin Ori-side trampoline function that takes the exact-type Ori value, converts it to the JS representation per the boundary protocol, performs the JS call, and converts the result back. No developer action required; the trampoline is invisible to user code. Type-checked against the `.d.ts` signature; runtime behavior is the call boundary protocol.

##### Callback capability propagation

A callback type `(x: T) => U` passed FROM Ori TO JS carries Ori-side capabilities into the call envelope:

```ori
use js "express" { Express };

@handler (req: Express.Request, res: Express.Response) -> void uses Net = {
    res.send("ok")
}

@main () -> void uses Js, Net = {
    let app = Express.new();
    app.get("/", handler)   // handler's `uses Net` is propagated; app.get callsite must be in scope of Net
}
```

- Sidecar metadata (`"<package>.<symbol>.<callback-position>": ["Cap"]`) declares which Ori capabilities a callback parameter MAY invoke; unannotated callback parameters default to **zero declared capabilities** (per `ffi-boundary-safety-proposal.md §5` callback ZeroCaps default).
- **Subset check at callback registration**: at the JS call site, the type checker verifies `<closure's uses set> ⊆ <callback parameter's declared uses set>`. Passing an Ori closure with `uses Net` to an unannotated callback parameter (which declared zero capabilities) is `E2055: callback capability requirement exceeds parameter scope`. The check direction is fixed: the closure cannot exceed what the parameter declared.
- Practical implication: registering an Ori closure with `uses Net` requires the JS package to publish sidecar metadata declaring `Net` for that callback position. Without sidecar declaration, only zero-`uses` closures may be passed. This forces sidecar adoption for capability-using callbacks rather than defaulting unsafely.

##### Generic instantiation

Generic functions and classes translate as opaque generics with the following rules:

- Generic parameters are erased at the JS call boundary (TypeScript's runtime behavior).
- Ori-side type-checking enforces the declared bound at compile time. JS-side runtime sees `JsAny` flow with no instance-of validation.
- **Boundary marshallers required for non-`Value` instantiations**: when an Ori type instantiated at a generic position has non-`Value` fields (i.e., requires marshalling other than direct copy), the translator inserts a per-type marshaller into the boundary trampoline. Without a marshaller, the translation falls back to `JsAny` at that position with diagnostic `W1501: non-Value generic instantiation lacks marshaller, marshalling skipped`.
- Higher-kinded types in `.d.ts` (e.g., `<F extends Functor>`) → `JsAny` fallback at the relevant position.
- Class constructors translate as associated functions on the opaque type; instance methods preserve method-bivariance handling per the variance section above.

#### §2.5. Type Translation Cache

Cache key composition:

```
sha256(
  package_name || "@" || package_version ||
  "|dts:" || sha256(<concatenated .d.ts file contents in package-export order>) ||
  "|caps:" || sha256(<contents of <pkg>.ori-caps.json or "<missing>">) ||
  "|tsconfig:" || sha256(<resolvable tsconfig.json relevant fields, normalized>) ||
  "|translator:" || $TRANSLATOR_VERSION ||
  "|compiler:" || $ORI_BUILD_NUMBER  (* read from compiler_repo/BUILD_NUMBER at build time per CLAUDE.md §Versioning *)
)
```

- `.d.ts` content hash invalidates on any declaration change post-install.
- `.ori-caps.json` hash invalidates capability metadata changes.
- Translator + compiler versions invalidate when translation rules evolve.
- Cached outputs at `target/js-bindings/<sha256>.ori-types`.

### §3. Capability Effect Propagation

Every `use js` import runs in `uses Js` context. Beyond `Js`, the type checker propagates additional capabilities based on package metadata.

#### §3.1. Capability Metadata Sources (Precedence Order)

**`Suspend` capability exception**: `Suspend` is NEVER auto-assigned by sidecar metadata, heuristic package classification, or default conservative tier — it propagates exclusively from binding-site resolution of a `JsPromise<T>` value (per `ori-syntax.md` async-WASM rules and OQ#7). The capability tiers below assign IO/effect capabilities (`Net`, `FileSystem`, `Random`, etc.); they MUST NOT add `Suspend`.


| Tier | Source | Use |
|---|---|---|
| 1 | Author sidecar `<pkg>.ori-caps.json` | Per-export and per-callback-position capabilities |
| 2 | Package metadata `package.json` `"oriCapabilities": {...}` | npm-ecosystem opt-in |
| 3 | Heuristic inference | Top-N packages hard-coded in stdlib (`fs-*` → `FileSystem`, etc.) |
| 4 | Default | `Js + UnknownEffects` for unannotated packages |

#### §3.2. The `UnknownEffects` Capability — v1 Hard-Error Semantics

A marker capability declared at imports without authoritative capability metadata. Properties:

- **Granted** via `uses UnknownEffects` (function-level capability declaration) or `with UnknownEffects = ... in expr` (expression-scoped explicit grant).
- **Deniable** via `without UnknownEffects` (preserves negative-effect monotonicity per `negative-effect-without-proposal.md` Rule 6). A function declaring `without UnknownEffects` is statically guaranteed to make NO calls into unannotated JS packages — useful for security boundaries, certified libraries, and modules that must avoid all unaudited JS code paths.
- **Non-dischargeable**: unlike capabilities with handler implementations, `UnknownEffects` cannot be discharged with a runtime handler (no `with UnknownEffects = MyHandler in expr` form). The `with` form is grant-only; there is no handler trait to implement.
- Default-imports `UnknownEffects` for any unannotated package; application code must explicitly grant via `uses` or be in a context that does NOT deny it via `without`.
- Note: `without UnknownEffects` is monotonic with `without Net`/etc. — denying `UnknownEffects` does NOT silently bypass `without Net`; both denials apply. A function `without Net, without UnknownEffects` rejects both (a) annotated JS packages requiring `Net` AND (b) any unannotated JS package, regardless of whether the unannotated package would have used `Net`.
- **Hard-error semantics**: a function calling an unannotated JS package without `uses Js, UnknownEffects` in scope is a compile error.
- Migration support: a package vendor publishing `oriCapabilities` metadata may set `"@compatibility": "warn-only"` per-symbol for one stable cycle before flipping to authoritative — per-package opt-in, NOT a language-level relaxation.

#### §3.3. `without` Clause Interaction (Negative Effects)

Per `negative-effect-without-proposal.md` Rules 3 ([DENY-CHECK]) and 4 ([DENY-NO-OVERRIDE]). Error codes match that proposal's authoritative range (E1260–E1262):

| Rule | Behavior |
|---|---|
| `without Net` on a function calling `axios.get()` (requires `Net` per metadata) | Compile error E1260; `without` denies `Net` and the JS call requires `Net` |
| `without Net` on a function calling an `UnknownEffects`-imported package | Compile error E1260; `UnknownEffects`'s conservative profile includes `Net` (Rule 3 [DENY-CHECK]) |
| `without Net` on a function calling a JS function with `Net` declared via sidecar | Compile error E1260 (Rule 3) |
| `with Net = handler in expr` overriding `without Net` from outer scope | Compile error E1261 per Rule 4 (DENY-NO-OVERRIDE; same condition as `negative-effect-without` for any deny target) |
| `without UnknownEffects` on a function calling any unannotated JS package | Compile error E1260 (Rule 3 [DENY-CHECK]); enforces "no calls into unaudited JS code" guarantee |

Compile-time enforcement is the primary safety claim. Engine-level runtime checks (§7 sandbox) are defense-in-depth, not the type system's authority.

#### §3.4. Error Model — Result Wrapping

JS calls may throw exceptions; Ori `Result` wraps them:

| Sidecar declaration | Translated signature |
|---|---|
| Default (no sidecar throws annotation) | `(...) -> Result<T, JsError> uses Js, ...` |
| `"<symbol>": { "@throws": "never" }` | `(...) -> T uses Js, ...` (unwrapped) |
| `"<symbol>": { "@throws": "MyError" }` | `(...) -> Result<T, MyError> uses Js, ...` (custom error type bound to a JS class) |

`JsError` is a stdlib type:

```ori
type JsError = {
    name: str,            // JS Error.name
    message: str,         // JS Error.message
    stack: Option<str>,   // JS Error.stack when available
    js_handle: JsRef,     // weak ref to the JS error object for advanced inspection
}
```

`?` operator works as expected; `Result<T, JsError>` flows through Ori's existing error handling without special cases.

### §4. AIMS Lattice Extension — `JsRef` as 8th Orthogonal Dimension

#### §4.1. Decision

Per AIMS missions §AIMS extension rule ("extends a lattice dimension OR extends a contract field OR feeds analysis as a typed pre-pass input"), `JsRef` is added as a **new orthogonal lattice dimension** (8th dimension on the existing 7-dimensional product lattice). This preserves AIMS invariants:

| Invariant | How preserved |
|---|---|
| Finite height (L-5) | `JsRef` is a 4-element chain: `None < Borrowed < Owned < Shared`; height = 3 |
| Defined join (L-10) | Join table specified below; closed under join |
| No shadow tracker | All cross-runtime ownership accounting flows through this dimension |
| Lattice-driven analysis | Per-SSA `JsRef` state computed by the same backward demand analysis as other dimensions |
| 7-dimensional product → 8-dimensional product | `Access × Consumption × Cardinality × Uniqueness × Locality × Shape × Effect × JsRef` |

The handle pointer (the actual `*mut JSValue` or equivalent) lives in a separate `JsHandleSideTable` keyed by SSA index — analogous to how `ReprPlan` carries layout metadata outside the lattice. The lattice value is a finite-element semantic class; the side-table carries the unbounded handle pointer.

#### §4.2. Lattice Definition

```
JsRef = { None, Borrowed, Owned, Shared }

partial order:  None < Borrowed < Owned < Shared

join table (⊔ = least upper bound):
        None      Borrowed   Owned     Shared
None    None      Borrowed   Owned     Shared
Bor     Borrowed  Borrowed   Owned     Shared
Own     Owned     Owned      Owned     Shared
Sha     Shared    Shared     Shared    Shared

(join semantics: Shared is the conservative top; merging Owned ⊔ Shared = Shared
 because the merged path may have JS holding a ref, so sync-decrement is the safe
 disposition rather than unconditional JS_FreeValue. Per Pierce, _Types and Programming
 Languages_ §16, lattice top = "may have any property the elements could have".)
```

#### §4.3. Sub-state semantics

| State | Meaning | Lifetime semantics |
|---|---|---|
| `None` | Pure Ori value, no JS handle | normal AIMS — unaffected |
| `Borrowed` | Transient view into JS heap; caller still owns the strong handle | no JS-side ref-count change; lifetime checked against caller scope |
| `Owned` | Ori holds the only strong handle; JS GC will not collect (no other JS-side rooting) | drop triggers JS-side `JS_FreeValue` (or engine-equivalent) |
| `Shared` | Both Ori (`RC≥1`) and JS hold the value | JS-side ref-counted; sync-decrement on drop, never unconditional free |

#### §4.4. Cross-Dimensional Canonicalization Rules

CN-9 through CN-12 fire in slot order **after CN-1..CN-5 (existing structural rules) and BEFORE CN-6 (Locality→Uniqueness demotion)**. This ordering is load-bearing — it lets CN-9 establish `Locality = HeapEscaping` for JS-located values, then the amended CN-6 (below) reads `JsRef` to skip Ori-heap-driven uniqueness demotion when JS-heap ownership is the relevant invariant.

| Rule ID | Slot | Constraint |
|---|---|---|
| CN-9 | post-CN-5, pre-CN-6 | `JsRef ≠ None ∧ Locality ∉ { Borrowed(p) }` ⟹ `Locality := HeapEscaping` (preserves `ffi-boundary-safety-proposal.md §4` `Locality::Borrowed(p)` parameter-bound borrows) |
| CN-6 (amended) | unchanged slot | `Locality ≥ HeapEscaping ∧ Uniqueness = Unique ∧ JsRef ∉ { Owned, Borrowed } ⟹ Uniqueness := MaybeShared` (only `Owned` and `Borrowed` preserve `Unique`; `Shared` MUST demote because multiple references exist by definition; `None` demotes per pre-amendment semantics) |
| CN-11 | post-CN-6 | `JsRef = Borrowed ∧ source is `#borrow_from(p)`-annotated JS function` ⟹ borrow tied to parameter `p` per `ffi-boundary-safety-proposal.md §4`. Lexical-scope check on the JS-call result site mirrors FFI boundary semantics. Violation = E4030. (NEW rule, not an amendment of an existing CN-11.) |
| CN-12 | post-CN-6 | `JsRef ≠ None ∧ crosses Nursery boundary` ⟹ rejected at compile time (E4032) per v1 thread-local `JsContext` constraint (§6). Multi-context concurrent support reverts this rule to `JsRef = Shared ⟹ Sendable` in v2 |
| CN-13 (intra-procedural local cycles only) | post-CN-12 | `JsRef = Owned ∨ JsRef = Shared` AND a strong cycle is statically detectable WITHIN the function body ⟹ rejected at compile time (E4031). Cross-procedural cycles cannot be detected by intra-procedural AIMS — they leak in v1 and require runtime cycle collection (v2). The compile-time claim is bounded to what AIMS can see |

#### §4.4a. AIMS Consumer Updates

The amended CN-6 creates feasible states `Locality = HeapEscaping ∧ Uniqueness = Unique ∧ JsRef ≠ None` that pre-amendment AIMS consumers did NOT expect. To preserve the unified model (AIMS invariant #5 — the unified model stays unified), every consumer of `Uniqueness` is enumerated and its handling of the new state is specified:

| Consumer | Reads `Uniqueness`? | Updated behavior under `JsRef ≠ None` |
|---|---|---|
| **CN-3 (COW eligibility)** | yes | `JsRef ≠ None` ⟹ NOT COW-eligible regardless of Ori `Uniqueness` (the JS engine owns its heap; Ori-side COW is not applicable). Pre-amendment CN-3 consumers reading only `Uniqueness` are explicitly extended to gate on `JsRef = None`. |
| **CN-6 (own amendment)** | yes | Already amended above. |
| **DP-5 / DP-6 (drop placement)** | yes | When `JsRef ≠ None`, drop placement consults `JsRef` state, not Ori-side `Uniqueness`. Owned → schedule `JS_FreeValue`; Shared → schedule sync-decrement; Borrowed → no drop (caller owns). |
| **`realize_cow` pass** | yes (transitively via CN-3) | Inherits CN-3 gate: `JsRef ≠ None` values skip COW realization. |
| **RL-11 (reuse eligibility)** | yes | `JsRef ≠ None` values are NOT reuse-eligible regardless of Ori-side uniqueness. |
| **VF-6 (FIP certification)** | yes | Extended to require per-parameter / per-return obligation match via `MemoryContract.js_param_handles` and `js_return_handles` (not scalar count). Intra-function obligations checked against `AimsStateMap.js_handles`. |
| **In-place mutation optimization** | yes | `JsRef ≠ None` ⟹ NOT in-place-mutable (Ori-side cannot mutate JS-engine memory). Pre-amendment `Uniqueness = Unique` was sufficient license; extended to also require `JsRef = None`. |

This enumeration is the load-bearing artifact for AIMS invariant #5: every facet that observes `Uniqueness` agrees on the same JS-locality predicate.

#### §4.5. Bridging Rules

| Operation | AIMS effect |
|---|---|
| Calling JS that returns an object | result enters `JsRef = Owned`, RC = 1 (Ori-side handle) |
| Passing Ori value to JS | requires `Sendable` per §6; transfer or copy at boundary |
| Passing `JsRef = Shared` across `Nursery` | requires CN-12 + `Sendable` proxy adapter |
| Last-use of `JsRef = Owned` value | AIMS schedules `JS_FreeValue` (or engine equivalent) via `JsHandleSideTable` lookup |
| `JsRef = Owned → Borrowed` (taking a view) | `RC` unchanged; new SSA gets `Borrowed` with caller-scope lifetime |

#### §4.6. Cycle Collection

- JS GC owns its side; Ori-side `JsRef = Shared` decrements via finalization callback registered at handle creation.
- **Intra-procedural strong cycles** (a function body that constructs Ori → JS → Ori within a single function) are rejected at compile time per CN-13 — `E4031: intra-procedural strong cycle (use weak/proxy adapter to break)`.
- **Cross-procedural strong cycles** cannot be detected by intra-procedural AIMS analysis. They leak in v1. Documented limitation; v2 adds runtime cycle collection (periodic sweep at quiescence).
- Weak refs (`JsRef::Weak` adapter, library-provided) and proxy adapters (`JsProxy<T>`, library-provided) break cycles by construction; recommended for any Ori → JS → Ori callback pattern.
- Linting: a future `js-cycle-lint` warning identifies common cross-procedural cycle patterns at code-review time (advisory, not enforcement).

#### §4.7. Verification + MemoryContract Extension

SSA indices are intra-procedural per `aims-rules.md §5` and MUST NOT leak into interprocedural summaries. `MemoryContract` (interprocedural) tracks handle obligations by **abstract identity** — parameter slots and return slots — not local SSA. The lattice (per-SSA `JsRef` dimension) and intra-function verification carry SSA-keyed obligations; the contract carries only what crosses the function boundary.

```ori
type JsHandleObligation = {
    state: JsRef,                       // Owned | Shared | Borrowed
    transfer: TransferKind,             // Consumed | Produced | Borrowed | None
}

type TransferKind =
    | Consumed                          // function takes ownership at this slot
    | Produced                          // function returns ownership at this slot
    | Borrowed { from_param: ParamIdx } // borrow tied to a specific parameter
    | None

// MemoryContract — per-parameter and per-return-slot, NOT per-SSA:
type MemoryContract = {
    // ... existing fields
    js_param_handles: [Option<JsHandleObligation>],    // index = parameter position
    js_return_handles: [Option<JsHandleObligation>],   // index = return slot (multi-return / Result wrapping)
}
```

Intra-function verification (per-SSA) lives in `AimsStateMap` (`aims-rules.md §4`), not `MemoryContract`:

```ori
// In AimsStateMap (intra-procedural):
type AimsStateMap = {
    // ... existing per-SSA state
    js_handles: Map<SsaIdx, IntraJsHandleState>,  // SSA-keyed; intra-procedural only
}
```

| Layer | Role | Scope |
|---|---|---|
| Lattice value (`JsRef` dimension) | Semantic class per SSA | Intra-procedural |
| `AimsStateMap.js_handles` | Per-SSA obligations with acquire/release sites | Intra-procedural (verification only) |
| `MemoryContract.js_param_handles` / `js_return_handles` | Boundary obligations by parameter/return identity | Interprocedural summary |
| Codegen layer (`ori_repr` analog) | Runtime pointer mapping `SsaIdx → *mut JSValue` | Codegen |

`verify_arc` checks per-SSA obligations within each function and matches function boundary obligations against `MemoryContract`. `FipContract::Certified` requires every internal SSA handle has a paired acquire/release AND every contract obligation is satisfied at boundary. Per-handle identity at the right granularity per layer.

AIMS invariant #5 preserved: each facet agrees on the same identity key (SSA inside function, parameter/return slot at boundary).

### §5. JS Engine Embedding — Pluggable Contract

```ori
trait JsEngine {
    type Value;
    type Error;

    @new_context () -> Self uses Allocator;
    @eval_module (self, source: str, name: str) -> Result<JsModule, Error>;
    @call (self, fn: Self.Value, args: [Self.Value]) -> Result<Self.Value, Error> uses Js;
    @get_property (self, obj: Self.Value, key: str) -> Result<Self.Value, Error>;
    @to_ori<T> (self, val: Self.Value) -> Result<T, Error>;
    @from_ori<T> (self, val: T) -> Result<Self.Value, Error>;
    @inc_ref (self, val: Self.Value) -> void;
    @dec_ref (self, val: Self.Value) -> void;
    @set_capability_gate (self, gate: CapabilityGate) -> void;
}
```

| Engine | Status | Tradeoffs |
|---|---|---|
| **QuickJS** (Bellard) | v1 default | ~50k LOC C, MIT, embeddable; FFI binding via Deep Safety capabilities (`RawMemory`, `Allocator`); good enough for boundary-crossing calls |
| V8 | v2 | Cloudflare-class performance; ~1M LOC C++; longer integration |
| JavaScriptCore | v2 (Apple platforms) | Bun-comparable performance |
| Hermes | v3 (embedded/mobile) | Smaller footprint than V8 |

Selection: compile-time feature flag `--js-engine=quickjs|v8|jsc|hermes` (per-build, not per-call).

### §6. `Sendable` and `Value` Trait Interaction

| Ori construct | JS-side representation | Crosses `Nursery` / channel? |
|---|---|---|
| `Value`-typed (int, float, bool, char, byte, Duration, Size, Ordering) | direct copy | yes — bitwise |
| `str` | UTF-16 conversion; lone surrogates → error | yes |
| `[T]` where `T: Sendable` | array proxy, lazy element conversion | yes |
| `{K: V}` where `K: Sendable, V: Sendable` | object proxy | yes |
| `JsRef::Owned<T>` | direct handle | NO — `Sendable` not implemented; use channels with explicit conversion |
| `JsRef::Shared<T>` | shared handle with proxy | yes IFF JS context is shared (single-context only in v1) |
| Capability handles | not transferable | NO |

v1 thread-safety model: **`JsContext` is thread-local, NOT shared across `Nursery` workers**. QuickJS `JSRuntime`/`JSContext` is not thread-safe; sharing across threads risks heap corruption. v1 therefore restricts `JsContext` lifetime to a single Ori thread; values crossing `Nursery` boundaries with `JsRef ≠ None` are blocked at compile time (E4032: cross-thread JS handle transfer requires explicit serialization). Multi-context concurrent support (per-worker `JsContext` with explicit hand-off protocol or a global engine mutex) is v2 work.

### §7. Sandbox Surface — Compile-Time + Runtime Two-Layer

The `JsSandbox` API is two layers:

#### §7.1. Compile-Time Layer (Type-Checker Authority)

- Sandbox capability filters are type-checker constraints, NOT runtime checks.
- Declaring a sandbox with `denied: [Net]` rejects any `use js` import that requires `Net` at compile time.
- Diagnostic: `E2058: capability denied by sandbox`.
- Authoritative for any code path the type checker fully analyzes via `.d.ts` + sidecar.

```ori
@spawn_plugin_sandbox<E: JsEngine> (script: str) -> JsSandbox uses Allocator = {
    let sb = JsSandbox.new<E>(
        denied: [Net, FileSystem],
        allowed: [Js],
    );
    sb.eval(script:)?;
    sb
}
```

The type checker rejects at compile time any call from a `JsSandbox`-scoped block whose required capabilities are in `denied:`. This is the primary safety claim.

#### §7.2. Runtime Layer (Authoritative for Dynamic Code)

API surface (v1 — fixed; non-goal #9 hard-gates additions):

```ori
trait JsSandbox {
    @set_memory_limit (self, bytes: Size) uses Allocator;
    @set_cpu_quantum (self, ms: Duration) uses Clock;
    @set_capability_filter (self, allowed: [Capability], denied: [Capability]) uses Js;
    @set_module_resolver (self, resolver: ModuleResolver) uses Js;
    @set_network_policy (self, policy: NetworkPolicy) uses Js, Net;
    @disable_dynamic_imports (self) uses Js;
}
```

Authority and triggers:

- Authoritative for any code path the type checker cannot fully analyze.
- Dynamic JS module loads (`import()`, `require()`) bypass static visibility — the runtime layer is the only enforcement that fires for them.
- v1 default: `disable_dynamic_imports()` is invoked at sandbox construction; opt-out is explicit.
- Misconfigured capability metadata (sidecar lies) is caught at runtime when the engine attempts the disallowed operation.
- Memory/CPU quotas are purely runtime concerns.

Layered authority:

- Statically-visible JS (every call resolved from `use js` + `.d.ts`): compile-time primary; runtime defense-in-depth.
- Dynamically-loaded JS (explicit opt-out of `disable_dynamic_imports`): runtime primary; compile-time provides no guarantee.
- Default keeps the compile-time claim whole; developers relaxing it accept the degradation explicitly.

---

## Worked Example: `lodash.chunk`

### Source

```ori
use js "lodash" { chunk };

type User = { id: int, email: str };

@batch_process (users: [User], size: int) -> Result<[[User]], JsError> uses Js = {
    chunk(users, size:)?
}
```

### Compiler steps

1. **Resolution (parse).** `use js "lodash"` parsed into `JsImportNode` AST entry; 2-token lookahead disambiguates from regular `use lodash` import.
2. **Type translation (typeck).** Reads `node_modules/lodash/index.d.ts`, finds `function chunk<T>(array: T[], size?: number): T[][]`. Translates to `<T> (array: [T], size: int = 1) -> Result<[[T]], JsError> uses Js`. The lodash sidecar declares `chunk.size` as `@oriType: int` for the integer parameter; the default `Result<T, JsError>` wrapping applies (no `@throws: never` declared, conservative-by-default per §3.4). The caller uses `?` to propagate JS exceptions. Cached at `target/js-bindings/<sha256>.ori-types`.
3. **Capability check.** `batch_process` declares `uses Js`. `chunk`'s `Js` capability is satisfied. lodash heuristic-known list classifies it as `Js`-only (no `Net`, no `FileSystem`).
4. **AIMS lowering.**
   - `users: [User]` is `Sendable` (User has only Value fields).
   - At call site: `users` is converted to a JS array proxy. `JsRef = Borrowed` for the proxy (caller still owns the Ori `[User]`).
   - `chunk` returns a new JS array; result handle enters Ori as `JsRef = Owned`, `Locality = HeapEscaping` (CN-9 enforced), RC = 1.
   - `js_handle_balance` for `chunk` call: +1 (one new handle).
5. **Drop scheduling.** At `batch_process` return, AIMS schedules `JS_FreeValue` on the result handle. If caller iterates the result and copies elements out, AIMS converts `JsRef = Owned` → `Borrowed` for elements, preserving the parent Owned until iteration completes.
6. **Codegen.** LLVM IR calls `ori_js_call(ctx, lodash_chunk_handle, [users_handle, size_value])`, returns `JsValue*`. Ori-side trampoline decrements the handle on drop per AIMS schedule.

### Lowered IR sketch (informal)

```
%users_arr = call @ori_js_to_array_proxy(%ctx, %users_ptr, %users_len)  ; JsRef=Borrowed
%size_val  = call @ori_js_from_int(%ctx, %size)
%chunk_fn  = load @lodash_chunk_handle
%result    = call @ori_js_call(%ctx, %chunk_fn, [%users_arr, %size_val])  ; JsRef=Owned, RC=1
; ... callers consume %result; AIMS schedules dec_ref at last use
call @ori_js_dec_ref(%ctx, %result)  ; balance: 0
```

---

## Alternatives Considered

### Alt 1: Compile Ori to JS (Kotlin/JS / Fable model)

Rejected. Architectural shift makes Ori a JS-hosted language, losing native performance and systems-programming use cases. The design trade is real and intentional in Kotlin/Fable; for Ori it would invalidate the AIMS performance claims.

### Alt 2: Manual binding (napi-rs / Neon model)

Rejected. Boundary becomes visible at every call site; npm ecosystem too large to hand-bind comprehensively. Existing UX failure mode in production Rust+Node stacks.

### Alt 3: WASM-only (wasm-bindgen model)

Rejected. Type-restricted across the boundary (no shared objects, no closures-with-state); JS-side requires bundler integration; per-call performance penalty.

### Alt 4: Polyglot VM (GraalVM model)

Rejected. All code pays VM tax; native performance lost. GraalVM's `HostAccess` is a runtime policy mechanism; this proposal's compile-time effect propagation is a fundamentally different safety story.

### Alt 5: Ad-hoc FFI to a JS engine without `.d.ts`

Rejected. Reduces UX to napi-rs level. The `.d.ts` corpus is the load-bearing asset.

### Alt 6: Single-engine commitment

- V8-only: prohibitive embedding cost; prevents embedded environments.
- QuickJS-only: ceiling on long-term performance.
Rejected; pluggable contract is the durable path.

### Alt 7: Reuse existing `extern "js"` syntax

Rejected for `.d.ts`-driven case. The existing `extern "js" from "lib" { ... }` is for hand-bound symbols (per `ori-syntax.md §FFI`). Auto-generated bindings have different ergonomic and lifecycle requirements; layering them over `extern "js"` would conflate two distinct binding mechanisms.

---

## Purity Analysis

| Component | Compiler-required? | Pure-Ori-possible? | Recommended location |
|---|---|---|---|
| `use js "..."` grammar | YES (parser) | NO | `ori_parse` |
| `.d.ts` parser (TS syntax → AST) | NO | YES | stdlib `library/std/js/dts_parser.ori` |
| `.d.ts` AST → Ori type translation | YES (must integrate with `TypeRegistry`) | NO | `ori_types` reads cached `.ori-types` files at typeck time |
| Capability propagation rules | YES (typeck + capability system) | NO | `ori_types` |
| AIMS `JsRef` 8th dimension | YES (ARC pass) | NO | `ori_arc` |
| Engine FFI binding (QuickJS) | NO (uses approved Deep FFI) | YES | stdlib `library/std/js/quickjs.ori` |
| `JsSandbox` runtime API | NO | YES | stdlib `library/std/js/sandbox.ori` |

**Recommendation:**

- The `.d.ts` parser is **pure Ori in stdlib** (`library/std/js/dts_parser.ori`). The grammar, type translation integration, capability propagation, and AIMS extension MUST live in compiler crates. The runtime engine binding and sandbox API live in stdlib (pure Ori on top of approved Deep FFI capabilities).

#### Const-Eval Bridge Design

The `.d.ts` parser is invoked at build time by `oric` through a new const-eval bridge. The bridge extends Ori's existing const-evaluation pipeline (which today consumes `CanExpr`) with three new steps:

| Step | Owner | Input | Output |
|---|---|---|---|
| 1. Discovery | `ori_parse` | source files | per-package `JsImportNode` AST entries |
| 2. File-read gateway | `oric` (impure crate per `canon.md §5`) | `JsImportNode.package_path` | `.d.ts` source string + `tsconfig.json` content |
| 3. Const-eval invocation | `oric` → `ori_eval` (pure) | `.d.ts` source string passed as a `str` argument to the stdlib parser entry point | serialized Ori type declarations as a `str` (canonical s-expression or JSON-like format defined in `library/std/js/dts_serial.ori`) — NOT a Rust `TypeDecl` (the stdlib parser cannot construct compiler-internal Rust types) |
| 4a. Deserialization | `oric` (gateway) | serialized `str` from step 3 | internal `[ori_ir::TypeDecl]` Rust struct, validated for invariants (no `Tag::Var` leaks, etc.) |
| 4b. TypeRegistry injection | `oric` → `ori_types` | `[ori_ir::TypeDecl]` from step 4a | populated `TypeRegistry` entries scoped to the importing module |
| 5. Cache write | `oric` | `[TypeDecl]` + cache key | `target/js-bindings/<sha256>.ori-types` |

Properties:

- IO is confined to `oric` per phase purity (`canon.md §5`); the parser sees only string content, never disk.
- Const-eval limits (1M steps / 1000 depth / 100MB / 10s — same as existing const-functions) bound parser cost.
- Cache hits skip steps 2–4 entirely; subsequent builds with unchanged `.d.ts` + tsconfig + parser version pay zero parse cost.
- No new compiler API surface beyond the existing const-eval entry point and `TypeRegistry` injection (which already exists for inherent built-in types).

---

## Spec & Grammar Impact

Spec clause assignments per `compiler_repo/docs/ori_lang/v2026/spec/README.md` index:

| Spec target | Change |
|---|---|
| `grammar.ebnf` (Annex A) | Add `js_import` to `source_file` alternation; document 2-token lookahead disambiguation; define `js_import_list`, `js_import_item` productions |
| Clause 18 (Modules) | New sub-clause — JS imports, resolution from `package.json` types, capability inference precedence |
| Clauses 8/9 (Type System) | New sub-clause — `.d.ts` translation surface, `JsAny` semantics, variance + callback capability propagation |
| Clause 20 (Capabilities) | Add `Js` capability; add `UnknownEffects` non-deniable marker; document `without` interaction with E1260–E1263 |
| Clause 21 (Memory Model) | New sub-clause — `JsRef` 8th lattice dimension; CN-9, CN-6 (amended), CN-11, CN-12, CN-13; intra-procedural per-SSA tracking via `AimsStateMap.js_handles`; interprocedural boundary tracking via `MemoryContract.js_param_handles` / `js_return_handles` |
| Clause 26 (FFI) | Cross-reference: `extern "js"` is the hand-bound form; `use js` is the `.d.ts`-driven auto-bound form |
| Capability Catalogue (in spec capability annex per spec layout) | New entries for `Js`, `UnknownEffects` |
| Diagnostic codes | E1500–E1519 (JS resolution); E1260 (deny-check, reused from `negative-effect-without`); E1261 (deny-no-override, reused from `negative-effect-without`); E2055 (callback capability requirement exceeds caller scope); E2058 (sandbox capability deny); E4030 (`JsRef = Borrowed` violates `#borrow_from(p)` lifetime); E4031 (intra-procedural strong cycle); E4032 (cross-thread JS handle transfer); W1500 (bivariant method signature normalized to `JsAny`); W1501 (non-`Value` generic instantiation lacks marshaller). E1262 / E1263 are NOT reused — every JS-side denial reuses the authoritative E1260 / E1261 codes from the `negative-effect-without` proposal |

Note: clause numbers reference the v2026 spec index. Sub-clause numbers will be assigned by the spec maintainer at proposal-approval time.

---

## Prior Art

| System | Compile-time vs runtime | Capability angle | `.d.ts` facade generator | What this proposal differs on |
|---|---|---|---|---|
| napi-rs | Compile-time macros for type marshalling; runtime ref-counting via N-API; Rust borrow checker manages `Env` lifetime | None (Node has no capability system) | Direction-inverted: napi-rs PRODUCES `.d.ts` from Rust `#[napi]` annotations (auto-generated declarations for the binding consumer); does NOT CONSUME existing package `.d.ts` as a binding source | Direction matters: this proposal's `.d.ts`-driven imports CONSUME existing TS declarations to drive auto-binding; napi-rs's flow goes the other way |
| Neon | Same as napi-rs, older API | None | None | Same |
| deno_core | Compile-time static types via `#[op]` macro for ops boundary; low-level V8 ops in Rust | Runtime permission system at Deno level | None | Compile-time effect propagation across the boundary (deno_core types each op manually; this proposal types entire packages from `.d.ts`) |
| wasm-bindgen | Macro-driven, type-restricted | None | Partial — manual `typescript_type` per-parameter annotation for imported JS types and `typescript_custom_section` for ad-hoc TS declarations ([wasm-bindgen.github.io/wasm-bindgen/reference/attributes/on-js-imports/typescript_type.html](https://wasm-bindgen.github.io/wasm-bindgen/reference/attributes/on-js-imports/typescript_type.html)); does NOT parse `.d.ts` files | Native engine, no WASM penalty; automated package-wide `.d.ts` parsing; richer types cross the boundary |
| Embind (C++) | Template-driven type binding; smooth UX | None | None | `.d.ts`-driven (no template gymnastics); compile-time effects |
| Pyodide | Dynamic | None | None | Static type-check via `.d.ts` |
| GraalVM polyglot | Runtime `HostAccess` policy | Runtime policy | None | Compile-time effect propagation; no shared-VM tax |
| Kotlin/JS + Dukat | Full static types on JS side; Dukat generates Kotlin facades from `.d.ts` | None | **YES — Dukat** ([github.com/Kotlin/dukat](https://github.com/Kotlin/dukat)) | Architectural difference: Kotlin/JS hosts on JS VM; Ori is cross-runtime with native host. AIMS + capability tracking are net-new; `.d.ts` facade generation is precedented |
| Scala.js + ScalablyTyped | Full static types on JS side; ScalablyTyped generates Scala.js facades from `.d.ts` | None | **YES — ScalablyTyped** ([scala-js.org/doc/tutorial/scalablytyped.html](https://www.scala-js.org/doc/tutorial/scalablytyped.html)) | Same architectural difference as Kotlin/JS; `.d.ts` facade generation is precedented |
| Fable F# + ts2fable | Compiles F# to JS; ts2fable generates F# facades from `.d.ts` | None | **YES — ts2fable** ([github.com/fable-compiler/ts2fable](https://github.com/fable-compiler/ts2fable)) | Same; precedent for `.d.ts` facade generation |
| Cloudflare Workers Bindings | Per-binding TS types; runtime permission | Runtime, per-Cloudflare-binding | None | Compile-time, language-level, generalizable |
| Bun FFI | Runtime FFI to C (not JS) | None | None | Different direction; Bun IS JS, this proposal calls JS from a native language |
| React Native Codegen / JSI / Hermes | Compile-time TypeScript/Flow spec → native interface generation; JSI provides direct JS↔native references | Runtime per-module permission | **YES — Codegen** ([reactnative.dev/docs/turbo-native-modules-introduction](https://reactnative.dev/docs/turbo-native-modules-introduction)) | Native cross-runtime precedent (closest analog architecturally). React Native Codegen consumes spec files; this proposal consumes `.d.ts` directly. Capability tracking + AIMS lifetime invariants are net-new |

**Refined novelty claim**: `.d.ts`-driven facade generation is **precedented** by Dukat, ScalablyTyped, and ts2fable for compile-to-JS host languages. This proposal's novelty is the **combination** of `.d.ts` facade generation with **(a) native cross-runtime execution** (Ori is not hosted on JS VM), **(b) compile-time effect propagation** through the boundary, and **(c) AIMS-managed cross-runtime memory invariants** with compile-time strong-cycle rejection. Each individual leg has prior art; the combination is unprecedented.

**Closest published research and adjacent type-import systems:**

- Lutze et al., "Effects with Subtraction" (ICFP 2023) — `without` clause foundation.
- Pyodide papers — UX precedent for cross-runtime interop, dynamic-typed.
- Bierman et al., "Understanding TypeScript" (ECOOP 2014) — TS gradual typing.
- Cloudflare Workers V8 isolate model — capability-sandboxed JS precedent.
- Reticulated Python (Vitousek 2014) — gradual type migration, comparable to `JsAny`-fallback strategy.
- TypeScript `allowJs` / `checkJs` ([typescriptlang.org/tsconfig/allowJs.html](https://www.typescriptlang.org/tsconfig/allowJs.html), [/checkJs.html](https://www.typescriptlang.org/tsconfig/checkJs.html)) — TS's own gradual-typing flow for plain JS files via JSDoc.
- Sorbet RBI ([sorbet.org/docs/rbi](https://sorbet.org/docs/rbi)) — Ruby type signatures consumed from external `.rbi` files; same authoritative-external-types pattern as `.d.ts`.
- mypy stubs / `.pyi` ([mypy.readthedocs.io/en/stable/stubs.html](https://mypy.readthedocs.io/en/stable/stubs.html)) — Python type stubs in separate files; PEP 484 / 561 distribution.
- AWS jsii ([aws.github.io/jsii](https://aws.github.io/jsii/overview/features/)) — TypeScript classes published as multi-language libraries; Ori is the consumer-side analog (consume TS as another language).

---

## Prerequisites

### Hard prerequisites (block implementation)

| ID | Prerequisite | Connection |
|---|---|---|
| 1 | `capability-propagation-completion-proposal.md` | Without complete `uses` propagation through imported declarations, JS-side capability tracking has no plumbing |
| 2 | `negative-effect-without-proposal.md` | `without` clause MUST work for the safety claims in §3.3 |
| 3 | `deep-ffi-proposal.md` (approved) — Phase 1 (extern blocks + `#error` annotations) AND Phase 2 (ownership annotations: `owned` / `borrowed` / `#free(JS_FreeValue)`) | FFI substrate for the QuickJS embedding (§5). Phase 1 covers `JSRuntime`/`JSContext`/`JSValue` opaque handles + primitives. Phase 2 is required for `JS_FreeValue` / `JS_DupValue` / `JS_NewObject` lifecycle: the engine binding annotates `owned CPtr` returns with `#free(JS_FreeValue)` and `borrowed CPtr` for transient handles |
| 4 | `ffi-boundary-safety-proposal.md` (approved) — Phase 1 (`Locality::Borrowed(p)` + `#borrow_from`) AND Phase 2 (callback `ZeroCaps` default) | Boundary safety rules apply to the JS engine FFI; CN-11 (§4.4) cites Phase 1's `#borrow_from(p)` mechanism; callback capability default rule (§2.4) cites Phase 2 |

Items 1+2 collectively constitute "Deep Safety Phase 0." Items 3+4 are the FFI substrate.

### Soft prerequisites (improve quality but do not block)

| ID | Prerequisite | Connection |
|---|---|---|
| 5 | `unsafe-operation-gating-proposal.md` | Reclassified soft. `Js` itself is a regular capability, NOT one of the five `Unsafe`-gated operations (raw pointer deref, pointer arith, mutable statics, transmute, C variadic). The proposal connection is narrow: QuickJS engine FFI internally uses `Unsafe` operations (raw pointer deref to JSValue, transmute for tagged pointer encoding); those internal uses gate via this proposal. JS interop at the user surface does NOT depend on it |
| 6 | AIMS `verify_arc` extension | Tracks `js_handle_balance` and `js_handle_count` per §4.7 |
| 7 | Compile-time reflection on `.d.ts` types | Would let sidecar metadata be generated rather than hand-written |
| 8 | `deep-ffi-proposal.md` Phase 3 (`[byte]` length elision) | Soft. Useful for data-transfer APIs (string conversion buffers, large object marshalling) but NOT required for core engine embedding |

---

## Non-Goals

1. **Not a full JS platform.** No bundler, no transpiler, no npm CLI. `package.json` resolution is the only npm-ecosystem concession. The embedded JS engine + sandbox API IS a JS runtime by literal definition (it runs JS) — the non-goal is the broader platform infrastructure (vite-style bundler, language-server protocol for JS, npm CLI, dependency resolver, etc.).
2. **Not a primary execution language.** Ori-native server is the hot path; JS exists for ecosystem access, not throughput. The runtime is configured to make this easy (compile-time-rejected JS in hot loops via lint, runtime memory/CPU quotas).
3. **Not full TypeScript type-system support in v1.** The constructs falling back to `JsAny` (mapped types, conditional types, template literal types, higher-kinded generics) are accepted v1 losses. v2 feasibility study is deferred — these features are pervasive in popular `.d.ts` files (lodash, React, Express), so a future expansion of supported constructs is plausible work, just not committed.
4. **Not multi-engine concurrent.** v1 ships one engine per build. Per-`Nursery`-worker contexts are v2.
5. **Not bidirectional source-level interop.** This proposal covers Ori → JS. JS → Ori (Node addon written in Ori) is a separate proposal.
6. **Not for browser deployment.** WASM target with JS interop is `wasm-playground-proposal.md` (approved).
7. **Not formal verification of cross-runtime memory safety.** AIMS structural verification (matched alloc/dealloc, per-handle obligation tracking, intra-procedural strong-cycle compile-time rejection) is in scope; full formal cross-runtime data-race-freedom proofs are research follow-up.
9. **Not unbounded sandbox API growth.** v1's `JsSandbox` API surface (`set_memory_limit`, `set_cpu_quantum`, `set_capability_filter`, `set_module_resolver`, `set_network_policy`, `disable_dynamic_imports`) is fixed. Adding a new sandbox method post-v1 requires an approved amendment proposal that re-evaluates the runtime-vs-platform boundary defined in non-goal #1. Hard gate against scope drift.
8. **Not silent leakage of intra-procedural strong cycles.** v1 rejects intra-procedural strong cross-runtime cycles at compile time per CN-13. Cross-procedural strong cycles cannot be detected by intra-procedural AIMS analysis and DO leak in v1 (documented limitation in §4.6); v2 adds runtime cycle collection (periodic sweep at quiescence). The compile-time claim is bounded to what AIMS can see, NOT a blanket "no leakage" guarantee.

---

## Open Questions — Resolved

The proposal author and review process resolve each open question inline below. Implementations may revisit if conditions change, but these resolutions are baseline.

| OQ | Question | Resolution |
|---|---|---|
| 1 | `JsRef` as new sub-state vs. orthogonal dimension? | **Orthogonal 8th dimension** (§4). Avoids breaking `Locality` chain finite-height (Gemini F1, Codex F1, Opencode F2 convergence). Side-table for handle pointers (analogous to ReprPlan). |
| 2 | `UnknownEffects` default too aggressive? | **Hard-error in v1** (per §3.2). Migration support is per-package opt-in: a vendor can publish `oriCapabilities` with `"@compatibility": "warn-only"` for one stable cycle before flipping to authoritative. The language-level safety claim is consistent with §3.3 `[DENY-CHECK]`; warning-only would silently bypass denied capabilities and was rejected for that reason. |
| 3 | `.d.ts` parser as Ori stdlib vs compiler crate? | **Pure Ori in stdlib** (`library/std/js/dts_parser.ori`). Produces `.ori-types` cache files. Type checker reads them via existing import resolution. Demonstrates "everything in stdlib" mission. |
| 4 | Single `JsContext` constraint v1? | **Yes for v1**, multi-context per `Nursery` worker is v2. Single-context simplifies the `Sendable` matrix (§6) and unblocks v1 ship. |
| 5 | TypeScript `number` translation? | **Default `float`**; sidecar `"@oriType": "int"` per-parameter override (§2.1). Preserves f64 semantics by default; integer-API ergonomics via opt-in. |
| 6 | Auto-wrap every JS call in `Result<T, JsError>` vs only when `.d.ts` declares throwing? | **Auto-wrap by default**; sidecar `"@throws": "never"` opts out (§3.4). Conservative-by-default, less surprise. |
| 7 | `async` propagation via `Suspend`? | **Tied to binding-site resolution, not Promise creation**. JS functions returning `Promise<T>` synchronously (Promise creation does not suspend) carry no `Suspend` requirement at the call site. `Suspend` propagates only at the **binding site** where Ori code resolves the `JsPromise<T>` to a `T` (per `ori-syntax.md` "Async WASM: `JsPromise<T>` implicitly resolved at binding sites"). Passing `JsPromise<T>` through Ori code without binding the value (e.g., forwarding to a `parallel(tasks:)` block, returning to a JS `.then()` chain) carries no `Suspend` requirement; only the resolution-to-`T` site does. |
| 8 | Cache invalidation key? | **Content hash + tsconfig + translator + compiler version** (§2.5). Hash invalidation handles all post-install drift. |
| 9 | LSP cache sharing with `oric`? | **Shared cache directory** at `target/js-bindings/`. LSP and `oric` both read the same `.ori-types` files; LSP triggers regeneration on `.d.ts` change via filesystem watch. |
| 10 | Sandbox capability filtering enforcement mechanism? | **Two layers** (§7). Compile-time in type checker (authoritative for static code); runtime in engine (defense for dynamic loads). Both required. |

---

## Implementation Sketch

### Prerequisite pipeline (must complete first)

| Phase | Source proposal | Estimated wall-clock |
|---|---|---|
| P-1 | capability-propagation-completion | 5–8 weeks |
| P-2 | negative-effect-without | 5–7 weeks |
| **Total prerequisite pipeline** | | **2.5–3.5 months** assuming serial execution; ~6–8 weeks with overlap |

`unsafe-operation-gating-proposal.md` is a soft prerequisite (engine-internal `Unsafe` ops only) and lands independently — does NOT block JS-interop work.

### JS-interop work (post-prerequisite)

Phase ordering: AIMS `JsRef` dimension lands BEFORE QuickJS embedding so the engine binding code is written against the safety invariants from day one (no need to retrofit handle tracking after the fact).

| Phase | Deliverable | Estimated effort (solo + AI) |
|---|---|---|
| 1 | Grammar + parser for `js_import` with 2-token lookahead | 2 weeks |
| 2 | AIMS `JsRef` 8th dimension (lattice extension + 5 new CN rules + amended CN-6) + `AimsStateMap.js_handles` per-SSA + `MemoryContract.js_param_handles` / `js_return_handles` boundary + AIMS consumer updates across CN-3 / DP-5 / DP-6 / `realize_cow` / RL-11 / VF-6 / in-place mutation + end-to-end verification per `aims-rules.md §VF-5` | 8–10 weeks (verification iteration cycles dominate; risk buffer in §Total wall-clock acknowledges further variance) |
| 3 | `.d.ts` parser in stdlib (Ori) | 4 weeks |
| 4 | `.d.ts` → Ori type translation + variance + callback capability + const-eval bridge | 5–6 weeks |
| 5 | QuickJS embedding via Deep FFI (now with AIMS handle invariants in place) | 4 weeks |
| 6 | Capability propagation rules + sidecar metadata loading + `UnknownEffects` hard-error gate | 3 weeks |
| 7 | Sendable / Value trait interaction with JS values + thread-local `JsContext` enforcement | 2 weeks |
| 8 | Compile-time + runtime sandbox layers + `disable_dynamic_imports` default | 3 weeks |
| 9 | Worked-example integration + spec sync + docs | 2 weeks |
| **Total JS-interop phases** | | **~7–9 months solo + AI** |

### Total wall-clock from today

**~10–12 months baseline** (prerequisites + JS-interop), or **~7–8 months from prerequisite-completion**. The earlier "6–7 month" estimate was JS-interop only; the dependency-timeline row was missing.

**Risk buffer**: AIMS `JsRef` 8th dimension + canonicalization rule additions + cross-runtime contract field touches the most invariant-sensitive part of the compiler. Realistic window with 2-4 month risk buffer for AIMS soundness iteration: **~12-16 months baseline**. The estimate assumes prerequisite proposals land cleanly without their own iteration cycles; if any prerequisite needs revision after an early `/tpr-review` round, add the corresponding delay.

V8/JSC pluggable engine is post-v1.

---

## Citations

- Lutze, N., et al. "Effects with Subtraction." *ICFP 2023*. <https://dl.acm.org/doi/10.1145/3607832>
- Bellard, F. "QuickJS Embedding Guide." <https://bellard.org/quickjs/>
- "V8 Embedder's Guide." <https://v8.dev/docs/embed>
- Riggs, R., et al. "Pyodide: Bringing the scientific Python stack to the browser." 2021.
- Bierman, G., et al. "Understanding TypeScript." *ECOOP 2014*.
- Vitousek, M., et al. "Reticulated Python: A Retrofitted Type System for Python." 2014.
- "Cloudflare Workers Bindings." <https://developers.cloudflare.com/workers/runtime-apis/bindings/>
- "Deno Permissions." <https://docs.deno.com/runtime/manual/basics/permissions>
- "GraalVM HostAccess." <https://www.graalvm.org/latest/reference-manual/embed-languages/>
- "TypeScript Strict Function Types." <https://www.typescriptlang.org/docs/handbook/release-notes/typescript-2-6.html>

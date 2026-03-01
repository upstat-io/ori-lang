---
section: "01"
title: "Complete File List"
status: not-started
---

# Complete File List

Every file to be rewritten, grouped by section. Check off as completed.

## Root

- [x] `index.md` — Full rewrite: book-style intro, dark Mermaid, removed source path tables, removed statistics

## Section 01: Architecture (4 files)

- [x] `01-architecture/index.md` — Full book treatment: conceptual foundations, classical approaches, prior art with links, dark Mermaid
- [x] `01-architecture/pipeline.md` — Full book treatment: push vs pull, fork point design, prior art (Rust/GHC/Go/Zig)
- [x] `01-architecture/salsa-integration.md` — Full book treatment: incremental computation foundations, design tradeoffs, prior art (rust-analyzer/Roslyn/Make)
- [x] `01-architecture/data-flow.md` — Full book treatment: progressive refinement, parallel arrays, ownership flow, design tradeoffs

## Section 02: Intermediate Representation (5 files)

- [x] `02-intermediate-representation/index.md` — Full book treatment: conceptual foundations (what are IRs, classical approaches), three-tier design, dark Mermaid, prior art (Zig/rustc/Lean/GHC/Roslyn)
- [x] `02-intermediate-representation/flat-ast.md` — Full book treatment: tree vs flat, ExprId/ExprRange design, SoA memory layout, prior art (Zig/Roslyn/Swift/Tree-sitter)
- [x] `02-intermediate-representation/arena-allocation.md` — Full book treatment: region-based memory (Tofte & Talpin), bump/typed/ID-based arenas, SoA cache math, prior art (Zig/rustc/V8/LLVM/ECS)
- [x] `02-intermediate-representation/string-interning.md` — Full book treatment: LISP symbol tables, sharding strategies, Name encoding, prior art (rustc/Go/Java/Lua/V8)
- [x] `02-intermediate-representation/type-representation.md` — Full book treatment: enum trees vs interned pools, Tag range architecture, Merkle hashing, TypeFlags propagation, prior art (Zig InternPool/rustc TyKind/GHC/Lean 4)

## Section 03: Lexer (2 files)

- [x] `03-lexer/index.md` — Full book treatment: conceptual foundations (what is lexical analysis, classical approaches, generated vs hand-written), two-layer architecture, template literals, greater-than splitting, dark Mermaid, prior art (Rust/Go/Zig/TS/Clang/GHC)
- [x] `03-lexer/token-design.md` — Full book treatment: conceptual foundations (what are tokens, representation design space), TokenKind enum design, TokenFlags, TokenList parallel arrays, literal design, prior art (Rust/Zig/TS/Roslyn/GHC)

## Section 04: Parser (5 files)

- [x] `04-parser/index.md` — Full book treatment: conceptual foundations (what is parsing, classical approaches — recursive descent, LR, Pratt, PEG, combinators), Ori's distinctive choices, dark Mermaid, prior art with links (Rust/Go/Zig/TS/Elm/Roc/Roslyn/tree-sitter/GHC/Clang)
- [x] `04-parser/pratt-parser.md` — Full book treatment: conceptual foundations (operator precedence problem, recursive descent chain, Pratt's 1973 insight, shunting-yard), binding power model with worked examples, associativity encoding, static OPER_TABLE, compound operator synthesis, prior art
- [x] `04-parser/error-recovery.md` — Full book treatment: conceptual foundations (error recovery problem, panic mode, phrase-level, Burke-Fisher, progress tracking), four-way ParseOutcome, backtracking macros, TokenSet bitfield, synchronization, prior art (Elm/Parsec/Roc/Rust/Roslyn/tree-sitter/Clang)
- [x] `04-parser/grammar-modules.md` — Full book treatment: conceptual foundations (parser organization, monolithic vs modular), module-per-construct design, series combinator, soft keywords, return type conventions, cross-module dependency diagram, prior art
- [x] `04-parser/incremental-parsing.md` — Full book treatment: conceptual foundations (IDE problem, granularity spectrum, red-green trees, tree-sitter, Salsa), declaration-level reuse algorithm, arena independence, Salsa composition, prior art (tree-sitter/Roslyn/TS/rust-analyzer/Zig/GHC)

## Section 05: Type System (6 files)

- [x] `05-type-system/index.md` — Full book treatment: conceptual foundations (type system design space, HM history, inference strategies), what makes Ori distinctive, dark Mermaid architecture + multi-pass diagrams, prior art with links (Damas-Milner/Zig/rustc/GHC/Lean 4/Elm/Roc), design tradeoffs
- [x] `05-type-system/pool-architecture.md` — Full book treatment: conceptual foundations (type interning, hash-consing tradition, SoA vs AoS), Merkle hash propagation, Item/extra layout, VarState lifecycle, TypeFlags propagation, prior art with links (Zig InternPool/rustc TyKind/GHC Uniques/V8/ECS), design tradeoffs
- [x] `05-type-system/type-inference.md` — Full book treatment: conceptual foundations (Curry/Hindley/Milner/Damas history, Algorithm W vs J, bidirectional checking, constraint-based), InferEngine architecture, worked examples (let binding, let-polymorphism, collections, match), capability tracking, prior art with links (Damas-Milner/Pierce-Turner/Kiselyov/Elm/Roc), design tradeoffs
- [x] `05-type-system/unification.md` — Full book treatment: conceptual foundations (Robinson 1965, union-find, occurs check, substitution maps vs linking), core algorithm walkthrough, flag-gated occurs check, rank system with generalization/instantiation, Never/Error handling, prior art with links (Robinson/Tarjan/Martelli-Montanari/Kiselyov/OCaml/GHC), design tradeoffs
- [x] `05-type-system/type-environment.md` — Full book treatment: conceptual foundations (symbol tables, scope chain approaches, functional vs mutable), Rc-linked design with CoW, shadowing, polymorphic bindings, scope usage patterns, edit-distance suggestions, prior art with links (ML/OCaml/GHC/rustc/Elm), design tradeoffs
- [x] `05-type-system/type-registry.md` — Full book treatment: conceptual foundations (nominal vs structural typing, typeclasses, method resolution), three registries architecture, TypeKind/TraitEntry/ImplEntry, object safety, method resolution order, registration ordering, prior art with links (Haskell/Rust/Swift/Go), design tradeoffs

## Section 06: Pattern System (5 files)

- [x] `06-pattern-system/index.md` — Full book treatment: conceptual foundations (compiler-level patterns, partitioning problem, classical approaches), what makes Ori distinctive, dark Mermaid, match pattern system, prior art with links (Lisp/Rust/Haskell/Zig/Go/Koka), design tradeoffs
- [x] `06-pattern-system/pattern-trait.md` — Full book treatment: conceptual foundations (trait-based abstraction for compiler constructs, interface design), focused trait hierarchy, context types, scoped bindings, worked examples (recurse, timeout), prior art with links (GHC/Rust/Clang/Zig), design tradeoffs
- [x] `06-pattern-system/pattern-registry.md` — Full book treatment: conceptual foundations (dispatch strategies — HashMap, trait object, visitor, enum, function pointers), enum dispatch design, registry architecture, prior art with links (Rust/GHC/Zig/Roslyn), design tradeoffs
- [x] `06-pattern-system/pattern-fusion.md` — Full book treatment: conceptual foundations (fusion/deforestation history — Wadler 1988, Gill 1993, stream fusion 2007), fusible combinations, data structures, prior art with links (GHC/Rust/Java/Futhark/Polly), design tradeoffs
- [x] `06-pattern-system/adding-patterns.md` — Full book treatment: conceptual foundations (when should something be a pattern, construct boundaries), step-by-step walkthrough with worked example, prior art with links (Rust/GHC/Zig), design tradeoffs

## Section 07: Canonicalization (4 files)

- [x] `07-canonicalization/index.md` — Full book treatment: conceptual foundations (IR spectrum, when to canonicalize, classical approaches), what makes it distinctive (single-pass, type-level sugar elimination, Arc-shared trees, TypeRef phase boundary), architecture with dark Mermaid, crate organization, prior art with links (Roc/Elm/GHC/Rust/Zig), design tradeoffs
- [x] `07-canonicalization/desugaring.md` — Full book treatment: conceptual foundations, prior art, theory-to-implementation
- [x] `07-canonicalization/pattern-compilation.md` — Full book treatment: conceptual foundations (Augustsson 1985, Wadler 1987, Maranget 2008, backtracking vs decision trees vs DAGs), two-phase flatten/compile architecture, worked example with guard, path-based navigation, nested type tracking, prior art with links (Wadler/Augustsson/Maranget/Rust/Elm/GHC/Gleam/Koka), design tradeoffs
- [x] `07-canonicalization/constant-folding.md` — Full book treatment: conceptual foundations (partial evaluation, Futamura 1971, compile-time evaluation spectrum), inline vs separate pass, constness classification, pure operators, integer/float/duration/size folding with overflow handling, dead branch elimination, ConstantPool with content addressing and bit-pattern floats, prior art with links (GCC/LLVM/Zig/Roc/Rust/Elm), design tradeoffs

## Section 08: Evaluator (5 files)

- [x] `08-evaluator/index.md` — Full book treatment: conceptual foundations (what is evaluation, execution strategies, tree-walking vs bytecode vs JIT vs AOT), what makes Ori distinctive (canonical IR input, Salsa-free core, pre-interned names, strategy-based derives, arena threading), dark Mermaid architecture + evaluation flow, method dispatch chain diagram, evaluation modes table, prior art (Roc/Lean 4/Zig/GHC/Ruby YARV), design tradeoffs
- [x] `08-evaluator/tree-walking.md` — Full book treatment: conceptual foundations (tree-walking history from McCarthy 1960, execution strategy spectrum table), copy-out pattern with borrow analysis, exhaustive dispatch on 54+ variants by category, decision tree evaluation, function call protocol, literal/binary/operator dispatch tiers, prior art (Roc/GHC Core/Zig comptime/Lua), design tradeoffs
- [x] `08-evaluator/environment.md` — Full book treatment: conceptual foundations (environments in PL theory, scope approaches table, closure capture strategies), Rc/RefCell design rationale, FxHashMap for Name keys, parent chain vs scope stack, RAII guards with Drop, mutability model, prior art (OCaml/Miri/Roc/GHC STG/V8), design tradeoffs
- [x] `08-evaluator/value-system.md` — Full book treatment: conceptual foundations (representation design space — tagged unions, object hierarchies, unboxed, NaN-boxing), Value enum by allocation strategy, Heap<T> wrapper with pub(super) enforcement, ScalarInt checked arithmetic, Cow strings for zero-copy, split Option/Result variants, prior art (Lua TValue/CPython PyObject/V8/Roc/GHC), design tradeoffs
- [x] `08-evaluator/module-loading.md` — Full book treatment: conceptual foundations (module systems, static vs dynamic), two-crate split, import resolution Mermaid flow, 8-step loading pipeline, module alias/namespace registration, test module access, circular import detection via Salsa, prior art (Rust/Python/Go/Haskell/Node.js), design tradeoffs

## Section 09: ARC System (10 files)

- [ ] `09-arc-system/index.md`
- [ ] `09-arc-system/arc-ir.md`
- [ ] `09-arc-system/lowering.md`
- [ ] `09-arc-system/borrow-inference.md`
- [ ] `09-arc-system/liveness.md`
- [ ] `09-arc-system/rc-insertion.md`
- [ ] `09-arc-system/reset-reuse.md`
- [ ] `09-arc-system/rc-elimination.md`
- [ ] `09-arc-system/drop-descriptors.md`
- [ ] `09-arc-system/decision-trees.md`

## Section 10: LLVM Backend (7 files)

- [ ] `10-llvm-backend/index.md`
- [ ] `10-llvm-backend/aot.md`
- [ ] `10-llvm-backend/closures.md`
- [ ] `10-llvm-backend/user-types.md`
- [ ] `10-llvm-backend/arc-emitter.md`
- [ ] `10-llvm-backend/builtins-codegen.md`
- [ ] `10-llvm-backend/codegen-verification.md`

## Section 11: Runtime (5 files)

- [ ] `11-runtime/index.md`
- [ ] `11-runtime/reference-counting.md`
- [ ] `11-runtime/collections-cow.md`
- [ ] `11-runtime/string-sso.md`
- [ ] `11-runtime/data-structures.md`

## Section 12: Formatter (4 files)

- [ ] `12-formatter/index.md`
- [ ] `12-formatter/spacing.md`
- [ ] `12-formatter/packing.md`
- [ ] `12-formatter/rules.md`

## Section 13: Diagnostics (4 files)

- [ ] `13-diagnostics/index.md`
- [ ] `13-diagnostics/problem-types.md`
- [ ] `13-diagnostics/code-fixes.md`
- [ ] `13-diagnostics/emitters.md`

## Section 14: Testing (3 files)

- [ ] `14-testing/index.md`
- [ ] `14-testing/test-discovery.md`
- [ ] `14-testing/test-runner.md`

## Section 15: Platform Targets (4 files)

- [ ] `15-platform-targets/index.md`
- [ ] `15-platform-targets/conditional-compilation.md`
- [ ] `15-platform-targets/wasm-target.md`
- [ ] `15-platform-targets/recursion-limits.md`

## Appendices (5 files)

- [ ] `appendices/A-salsa-patterns.md`
- [ ] `appendices/B-compiler-memory.md`
- [ ] `appendices/C-error-codes.md`
- [ ] `appendices/D-debugging.md`
- [ ] `appendices/E-coding-guidelines.md`

---

**Total: 76 files** (37 done, 39 remaining)

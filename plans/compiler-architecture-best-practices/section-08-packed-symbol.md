---
section: "08"
title: "Packed Symbol Representation"
status: not-started
reviewed: false
goal: "Design and implement a Symbol = (ModuleId, Name) type that encodes module provenance alongside identifier identity, enabling O(1) cross-module resolution without secondary lookups"
success_criteria:
  - "A ModuleId type exists in ori_ir with a compact packed representation"
  - "A Symbol = (ModuleId, Name) type exists with O(1) equality and hashing"
  - "Cross-module name resolution uses Symbol instead of Name + context lookup"
  - "Option<Symbol> has niche optimization (no extra bytes vs Symbol)"
  - "impl-hygiene.md §Aspirational Patterns updated to mark Packed Symbol as 'implemented'"
inspired_by:
  - "Roc Symbol = (ModuleId, IdentId) packed in u64 (crates/compiler/module/src/symbol.rs)"
  - "Rust DefId = (CrateNum, DefIndex) for cross-crate identity"
  - "Zig InternPool.Index — single u32 encoding module + local identity"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "Design ModuleId & Symbol Types"
    status: not-started
  - id: "08.2"
    title: "Implement Symbol in ori_ir"
    status: not-started
  - id: "08.3"
    title: "Migrate Cross-Module Resolution"
    status: not-started
  - id: "08.4"
    title: "Cleanup & Documentation"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Packed Symbol Representation

**Status:** Not Started
**Goal:** Implement the aspirational packed symbol pattern: a `Symbol = (ModuleId, Name)` type that eliminates secondary lookups for cross-module name resolution.

**Success Criteria:**
- [ ] `ModuleId` and `Symbol` types exist — satisfies mission criterion "Symbol encodes module provenance"
- [ ] Cross-module lookups are O(1) — satisfies mission criterion "without secondary Name→Module maps"
- [ ] `Option<Symbol>` is niche-optimized — satisfies mission criterion for compact representation
- [ ] All tests pass with the new symbol type

**Context:** Research found:
- `Name(u32)` in `ori_ir/src/name/mod.rs` (84 lines): 32-bit sharded layout (4-bit shard + 28-bit local index). O(1) equality. NO module provenance.
- Zero hits for `ModuleId` anywhere in the compiler — module provenance doesn't exist as a type.
- Cross-module resolution works through: `StringInterner` for Name→string, `TypeRegistry` for nominal type lookup by `Name`, `check/imports.rs` for import resolution. All require secondary lookups.
- The aspirational pattern from `impl-hygiene.md` proposes `Symbol = (ModuleId, Name)` with niche optimization for `Option<Symbol>`.

**Design considerations** (from Roc prior art):
- Roc packs `Symbol` into a single `u64`: upper 32 bits = ModuleId, lower 32 bits = IdentId
- This gives O(1) equality and perfect hashing without indirection
- Ori's `Name` is already 32 bits — a `Symbol = (ModuleId, Name)` as `u64` would follow the same pattern
- `ModuleId` needs to be assigned during module loading/import resolution — the driver owns this

**Depends on:** Section 01 (policy language). Independent of Sections 02-07.

---

## 08.1 Design ModuleId & Symbol Types

**File(s):** design document (not code yet)

Design the packed representation before implementing. This subsection produces a design, not code.

- [ ] Design `ModuleId`:
  - Compact integer type — `u16` (65K modules) or `u32` (4B modules)?
  - `u16` is likely sufficient and keeps `Symbol` at 48 bits (fits in 8 bytes with padding, or 6 bytes packed)
  - `ModuleId(0)` = current module (or prelude?). Assignment order: prelude first, then imported modules in declaration order
  - Consider: `ModuleId::PRELUDE`, `ModuleId::CURRENT`, `ModuleId::UNKNOWN` sentinels

- [ ] Design `Symbol`:
  - `Symbol { module: ModuleId, name: Name }` — total 48 bits (u16 + u32), stored as u64 for alignment
  - Or pack into u64: `(module as u64) << 32 | name.raw() as u64`
  - O(1) equality: integer comparison on the packed u64
  - O(1) hashing: hash the u64 directly
  - `Option<Symbol>`: use `u64::MAX` as niche (no valid ModuleId + Name combination)
  - Derive: `Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug`

- [ ] Design migration path:
  - Where is `ModuleId` assigned? (driver/`oric` during module loading)
  - Where does `Symbol` replace `Name`? (type registry, trait registry, method registry, import resolution)
  - Where does `Name` remain? (string interning, lexer output, parser AST — module identity not yet known at lex/parse time)
  - The boundary: parser produces `Name`, checker converts to `Symbol` after import resolution

- [ ] Validate design against `/tp-help` — present to Codex + Gemini for architectural review

- [ ] **Subsection close-out (08.1)** — MANDATORY before starting 08.2:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 08.2 Implement Symbol in ori_ir

**File(s):** `compiler/ori_ir/src/symbol.rs` (new), `compiler/ori_ir/src/lib.rs`

- [ ] Write tests first:
  - `Symbol` equality: same module + same name = equal
  - `Symbol` inequality: different module + same name = not equal
  - `Symbol` hashing: consistent with equality
  - `Option<Symbol>` size: `size_of::<Option<Symbol>>() == size_of::<Symbol>()`
  - `ModuleId` sentinels work correctly

- [ ] Implement `ModuleId`:
  ```rust
  #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
  #[repr(transparent)]
  pub struct ModuleId(u16);
  
  impl ModuleId {
      pub const PRELUDE: ModuleId = ModuleId(0);
      pub const CURRENT: ModuleId = ModuleId(1);
      // User modules start at 2
  }
  ```

- [ ] Implement `Symbol`:
  ```rust
  #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
  #[repr(transparent)]
  pub struct Symbol(u64);
  
  impl Symbol {
      pub const fn new(module: ModuleId, name: Name) -> Self {
          Symbol((module.0 as u64) << 32 | name.raw() as u64)
      }
      pub const fn module(self) -> ModuleId { ModuleId((self.0 >> 32) as u16) }
      pub const fn name(self) -> Name { Name::from_raw(self.0 as u32) }
  }
  ```

- [ ] Add `size_of` compile-time assertions:
  ```rust
  const _: () = assert!(size_of::<Symbol>() == 8);
  const _: () = assert!(size_of::<Option<Symbol>>() == 8); // niche optimization
  ```

- [ ] Export from `ori_ir` crate root

- [ ] **Subsection close-out (08.2)** — MANDATORY before starting 08.3:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 08.3 Migrate Cross-Module Resolution

**File(s):** `compiler/ori_types/src/registry/`, `compiler/ori_types/src/check/imports.rs`, `compiler/oric/src/`

Migrate cross-module name resolution from `Name` + context lookup to `Symbol`-based lookup. This is the largest subsection — it touches registries and import resolution.

- [ ] Add `ModuleId` assignment to the driver layer (`oric`):
  - Assign `ModuleId` to each module during loading
  - Prelude gets `ModuleId::PRELUDE` (0)
  - Current module gets `ModuleId::CURRENT` (1) — or a freshly assigned ID

- [ ] Migrate `TypeRegistry` to use `Symbol` as the key for cross-module type lookups:
  - Current: `TypeRegistry` indexes by `Name` — same-named types in different modules can collide
  - Target: index by `Symbol` — `(module, name)` uniquely identifies a type across all modules
  - Note: local lookups (within the same module) can still use `Name` for convenience — `Symbol::new(CURRENT, name)` wraps it

- [ ] Migrate import resolution (`check/imports.rs`) to produce `Symbol` values:
  - `use "./math" { add }` → resolves `add` to `Symbol(math_module_id, add_name)`
  - Store the resolved `Symbol` in the environment, not just `Name`

- [ ] Verify: all existing tests pass — the migration should be behavioral no-op for single-module programs (the common case in tests)

- [ ] Matrix testing:
  - Single-module programs (most tests) — should work with `CURRENT` module ID
  - Multi-file programs (import tests in `tests/spec/`) — should resolve cross-module symbols correctly
  - Name collisions across modules (if test exists) — should now be resolvable

- [ ] **Subsection close-out (08.3)** — MANDATORY before starting 08.4:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 08.4 Cleanup & Documentation

**File(s):** `.claude/rules/impl-hygiene.md`, `.claude/rules/ir.md`, `.claude/rules/types.md`

- [ ] Update `impl-hygiene.md` §Aspirational Patterns → Packed Symbol: change from "aspirational" to "implemented"

- [ ] Add `Symbol` and `ModuleId` to `ir.md` §Name Interning (or new §Symbol section)

- [ ] Update `types.md` §RG-1 (TypeRegistry) to document `Symbol`-based indexing

- [ ] Add `Symbol` to `CLAUDE.md` §Key Paths or relevant section

- [ ] **Subsection close-out (08.4)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 08.R Third Party Review Findings

- None.

---

## 08.N Completion Checklist

- [ ] All subsections (08.1-08.4) complete
- [ ] `Symbol` and `ModuleId` types exist with correct size/niche
- [ ] Cross-module resolution uses `Symbol`
- [ ] All tests pass (single-module AND multi-file)
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean
- [ ] **`/improve-tooling`** — section-close sweep
- [ ] **`/sync-claude`** — section-close doc sync

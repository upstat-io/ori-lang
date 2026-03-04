---
section: "07"
title: "Constant Deduplication"
status: not-started
goal: "Identical string constants share a single global — zero duplicates in emitted IR"
inspired_by:
  - "Rust rustc_codegen_llvm/common.rs — interns all string constants via const_str()"
  - "LLVM LangRef — unnamed_addr + linkonce_odr for constant merging"
depends_on: []
sections:
  - id: "07.1"
    title: "String Constant Interning"
    status: not-started
---

# Section 07: Constant Deduplication

**Status:** Not Started
**Goal:** Each unique string constant (e.g., `"integer overflow on addition\00"`) is emitted as a single LLVM global, shared across all use sites. Zero duplicate constant strings in emitted IR.

**Context:** The codegen emits identical overflow message strings as separate globals for each overflow check site. J2 has 2 duplicates, J7 has 6, J9 has 7, J12 has 6. While LLVM's linker may merge `unnamed_addr` constants at link time, the IR is unnecessarily verbose and the duplicate creation wastes module-level resources.

**Journeys affected:** J2, J3, J4, J6, J7, J8, J9, J10, J11, J12. (10 of 12 journeys — this is the single most pervasive finding.)

**Reference implementations:**
- **Rust** `rustc_codegen_llvm/common.rs`: Uses `const_str()` which interns all string constants — same string → same global.
- **LLVM** itself: `unnamed_addr` constants with identical content are candidates for COMDAT folding at link time, but emitting one global from the start is strictly better.

---

## 07.1 String Constant Interning

**File(s):** `compiler/ori_llvm/src/codegen/ir_builder/constants.rs`

The duplicate globals come from `build_global_string_ptr()`, not `const_string()`. `const_string()` creates inline byte arrays; `build_global_string_ptr()` creates named global string pointers — the ones that duplicate. It's called from:
- `compiler/ori_llvm/src/codegen/ir_builder/arithmetic.rs` — overflow panic messages (primary source)
- `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs` — string literal emission
- `compiler/ori_llvm/src/codegen/derive_codegen/string_helpers.rs` — derive codegen string construction

Add a string constant cache to the IR builder. When `build_global_string_ptr` is called, first check if an identical string has already been emitted; if so, return the existing global pointer.

```rust
// In IrBuilder or a module-level state accessible from IrBuilder:
global_strings: HashMap<String, PointerValue<'ctx>>,

pub fn build_global_string_ptr(&mut self, value: &str, name: &str) -> ValueId {
    if let Some(&existing) = self.global_strings.get(value) {
        return self.arena.push_value(existing.into());
    }
    let v = self.builder
        .build_global_string_ptr(value, name)
        .expect("build_global_string_ptr")
        .as_pointer_value();
    self.global_strings.insert(value.to_string(), v);
    self.arena.push_value(v.into())
}
```

- [ ] Add a `HashMap<String, PointerValue<'ctx>>` to the IR builder or module-level codegen state
- [ ] Modify `build_global_string_ptr()` in `constants.rs` to check cache before creating globals
- [ ] Cache key uses full byte content (including terminating `\0` contract), not just a display label
- [ ] Mark deduplicated globals with `unnamed_addr` to enable linker-level COMDAT folding
- [ ] Verify: J7 IR has exactly 1 `"integer overflow on addition\00"` global (not 6)
- [ ] Verify: J9 IR has exactly 1 of each overflow message (not 7)
- [ ] Count: Total global reduction across all 12 journeys

### 07.1 Completion Checklist

- [ ] String constant cache implemented in IR builder (or module-level state)
- [ ] `build_global_string_ptr()` deduplicates by content, not by name
- [ ] Count of global definitions for each overflow string is 1 per module
- [ ] No duplicate `@.str.*` globals with identical content
- [ ] J7 IR has exactly 1 `"integer overflow on addition\00"` global
- [ ] J9 IR has exactly 1 of each overflow message
- [ ] Deduplicated globals have `unnamed_addr` for linker-level folding
- [ ] IR test: program with 3 overflow sites has 1 overflow message global (not 3)
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## Section 07 Exit Criteria

For any program, `ORI_DUMP_AFTER_LLVM=1` shows at most one global per unique string value. No duplicated string constants in emitted IR for any of the 12 code journeys.

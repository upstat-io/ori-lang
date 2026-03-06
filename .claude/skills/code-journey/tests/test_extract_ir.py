"""Tests for extract_ir_from_results.py — LLVM IR extraction from results markdown."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from extract_ir_from_results import extract_ir, extract_frontmatter


SAMPLE_MD = """\
---
journey: 1
slug: arithmetic
expected: 33
eval_result: 33
aot_result: 33
score: 8.9
---

# Journey 1

## Source

```ori
@main () -> int = 42;
```

### Backend: LLVM Codegen

#### Generated LLVM IR

```llvm
; ModuleID = 'test'
define fastcc i64 @_ori_main() #0 {
bb0:
  ret i64 42
}
attributes #0 = { nounwind uwtable }
```

#### Disassembly

```asm
main:
  ret
```

### 7. Optimal IR Comparison

```llvm
; ideal version
define fastcc i64 @_ori_main() {
  ret i64 42
}
```
"""


class TestExtractIr:
    def test_extracts_first_llvm_block(self):
        ir = extract_ir(SAMPLE_MD)
        assert ir is not None
        assert "define fastcc i64 @_ori_main()" in ir
        assert "ret i64 42" in ir

    def test_skips_ideal_comparison_block(self):
        ir = extract_ir(SAMPLE_MD)
        assert ir is not None
        # The ideal block has "; ideal version" — should NOT be in extracted IR
        assert "; ideal version" not in ir

    def test_includes_attributes(self):
        ir = extract_ir(SAMPLE_MD)
        assert ir is not None
        assert 'attributes #0 = { nounwind uwtable }' in ir

    def test_includes_module_id(self):
        ir = extract_ir(SAMPLE_MD)
        assert ir is not None
        assert "; ModuleID = 'test'" in ir

    def test_no_header_returns_none(self):
        md = "# No LLVM IR here\nJust text."
        assert extract_ir(md) is None

    def test_header_but_no_block_returns_none(self):
        md = "#### Generated LLVM IR\n\nNo code block follows."
        assert extract_ir(md) is None

    def test_empty_input(self):
        assert extract_ir("") is None

    def test_unclosed_fence(self):
        md = "#### Generated LLVM IR\n\n```llvm\ndefine void @f() {\n"
        assert extract_ir(md) is None


class TestExtractFrontmatter:
    def test_extracts_basic_fields(self):
        fm = extract_frontmatter(SAMPLE_MD)
        assert fm["journey"] == "1"
        assert fm["slug"] == "arithmetic"
        assert fm["expected"] == "33"
        assert fm["eval_result"] == "33"
        assert fm["aot_result"] == "33"
        assert fm["score"] == "8.9"

    def test_no_frontmatter(self):
        fm = extract_frontmatter("# Just a heading\nSome text.")
        assert fm == {}

    def test_empty_input(self):
        fm = extract_frontmatter("")
        assert fm == {}


class TestRealJourney1:
    """Test against the actual journey 1 results file if available."""

    RESULTS = Path(__file__).resolve().parent.parent.parent.parent.parent / \
        "plans/code-journeys/01-arithmetic-results.md"

    def test_extracts_real_ir(self):
        if not self.RESULTS.exists():
            return  # Skip if results file not available
        md = self.RESULTS.read_text()
        ir = extract_ir(md)
        assert ir is not None
        assert "@_ori_add" in ir
        assert "@_ori_main" in ir
        assert "llvm.sadd.with.overflow" in ir

    def test_real_frontmatter(self):
        if not self.RESULTS.exists():
            return
        fm = extract_frontmatter(self.RESULTS.read_text())
        assert fm["expected"] == "33"
        assert fm["slug"] == "arithmetic"

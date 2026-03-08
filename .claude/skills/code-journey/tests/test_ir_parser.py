"""Tests for ir_parser.py — LLVM IR parser for Code Journey metrics."""

import sys
import textwrap
from pathlib import Path

# Add parent directory to path so we can import ir_parser
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from ir_parser import Module, Function, BasicBlock, Instruction, parse_module

GOLDEN_DIR = Path(__file__).resolve().parent / "golden"


def _load_journey1_ir() -> str:
    return (GOLDEN_DIR / "journey1_ir.txt").read_text()


# ---------------------------------------------------------------------------
# Module-level parsing
# ---------------------------------------------------------------------------

class TestParseModule:
    def test_parses_without_errors(self):
        m = parse_module(_load_journey1_ir())
        assert m.parse_errors == [], f"Parse errors: {m.parse_errors}"

    def test_function_count(self):
        m = parse_module(_load_journey1_ir())
        # 3 definitions + 4 declarations = 7 functions
        assert len(m.functions) == 7

    def test_globals_count(self):
        m = parse_module(_load_journey1_ir())
        assert len(m.globals) == 3  # ovf.msg, ovf.msg.1, ovf.msg.2

    def test_attribute_groups(self):
        m = parse_module(_load_journey1_ir())
        assert 0 in m.attribute_groups
        assert 1 in m.attribute_groups
        assert 2 in m.attribute_groups
        assert 3 in m.attribute_groups
        assert m.attribute_groups[0] == {"nounwind", "uwtable"}
        assert "cold" in m.attribute_groups[2]
        assert "noreturn" in m.attribute_groups[2]
        assert m.attribute_groups[3] == {"nounwind"}

    def test_attribute_group_1_memory(self):
        """memory(none) is a single attribute with parens."""
        m = parse_module(_load_journey1_ir())
        attrs = m.attribute_groups[1]
        assert "memory(none)" in attrs
        assert "nounwind" in attrs
        assert "nocallback" in attrs


# ---------------------------------------------------------------------------
# Function classification
# ---------------------------------------------------------------------------

class TestFunctionClassification:
    def test_user_functions(self):
        m = parse_module(_load_journey1_ir())
        user = m.user_functions()
        names = {f.raw_name for f in user}
        assert names == {"@_ori_add", "@_ori_main"}

    def test_user_functions_exclude_main_wrapper(self):
        m = parse_module(_load_journey1_ir())
        user = m.user_functions()
        assert all(f.raw_name != "@main" for f in user)

    def test_runtime_declarations(self):
        m = parse_module(_load_journey1_ir())
        rt = m.runtime_declarations()
        names = {f.raw_name for f in rt}
        assert names == {"@ori_panic_cstr"}

    def test_llvm_intrinsics(self):
        m = parse_module(_load_journey1_ir())
        intrinsics = m.llvm_intrinsics()
        names = {f.raw_name for f in intrinsics}
        assert "@llvm.sadd.with.overflow.i64" in names
        assert "@llvm.smul.with.overflow.i64" in names
        assert "@llvm.ssub.with.overflow.i64" in names
        assert len(names) == 3

    def test_entry_wrapper(self):
        m = parse_module(_load_journey1_ir())
        main = m.functions["@main"]
        assert m.is_entry_wrapper(main)
        assert not m.is_entry_wrapper(m.functions["@_ori_add"])

    def test_entry_called(self):
        m = parse_module(_load_journey1_ir())
        ori_main = m.functions["@_ori_main"]
        assert ori_main.is_entry_called
        assert not m.functions["@_ori_add"].is_entry_called


# ---------------------------------------------------------------------------
# @_ori_add function details
# ---------------------------------------------------------------------------

class TestOriAdd:
    def test_calling_convention(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        assert f.calling_convention == "fastcc"

    def test_return_type(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        assert f.return_type == "i64"

    def test_attributes_resolved(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        assert "nounwind" in f.attributes
        assert "uwtable" in f.attributes
        assert "noundef" in f.attributes  # inline on return type

    def test_param_types(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        assert f.param_types == ["i64", "i64"]

    def test_param_attributes(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        assert len(f.param_attributes) == 2
        assert "noundef" in f.param_attributes[0]
        assert "noundef" in f.param_attributes[1]

    def test_block_count(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        assert len(f.blocks) == 3  # bb0, add.ok, add.ovf_panic

    def test_block_labels(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        labels = [b.label for b in f.blocks]
        assert labels == ["bb0", "add.ok", "add.ovf_panic"]

    def test_instruction_count(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        assert f.instruction_count == 7  # 4 + 1 + 2

    def test_bb0_instructions(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        bb0 = f.blocks[0]
        assert len(bb0.instructions) == 4
        assert bb0.instructions[0].opcode == "call"
        assert bb0.instructions[0].result == "%add"
        assert bb0.instructions[1].opcode == "extractvalue"
        assert bb0.instructions[2].opcode == "extractvalue"
        assert bb0.instructions[3].opcode == "br"
        assert bb0.instructions[3].result is None

    def test_panic_block(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_add"]
        panic = f.blocks[2]  # add.ovf_panic
        assert panic.label == "add.ovf_panic"
        assert len(panic.instructions) == 2
        assert panic.instructions[0].opcode == "call"
        assert "ori_panic_cstr" in panic.instructions[0].text
        assert panic.instructions[1].opcode == "unreachable"


# ---------------------------------------------------------------------------
# @_ori_main function details
# ---------------------------------------------------------------------------

class TestOriMain:
    def test_calling_convention(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_main"]
        assert f.calling_convention == "ccc"  # entry-called, no fastcc

    def test_instruction_count(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_main"]
        assert f.instruction_count == 14

    def test_block_count(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@_ori_main"]
        assert len(f.blocks) == 5  # bb0, mul.ok, mul.ovf_panic, sub.ok, sub.ovf_panic


# ---------------------------------------------------------------------------
# @main wrapper
# ---------------------------------------------------------------------------

class TestMainWrapper:
    def test_calling_convention(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@main"]
        assert f.calling_convention == "ccc"

    def test_return_type(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@main"]
        assert f.return_type == "i32"

    def test_instruction_count(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@main"]
        assert f.instruction_count == 3

    def test_attributes(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@main"]
        assert "nounwind" in f.attributes
        assert "uwtable" not in f.attributes  # Only #3, not #0


# ---------------------------------------------------------------------------
# Declarations
# ---------------------------------------------------------------------------

class TestDeclarations:
    def test_ori_panic_cstr(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@ori_panic_cstr"]
        assert not f.is_definition
        assert f.return_type == "void"
        assert "cold" in f.attributes
        assert "noreturn" in f.attributes
        assert f.param_types == ["ptr"]

    def test_intrinsic_return_type(self):
        m = parse_module(_load_journey1_ir())
        f = m.functions["@llvm.sadd.with.overflow.i64"]
        assert "i64" in f.return_type  # { i64, i1 }
        assert not f.is_definition
        assert f.param_types == ["i64", "i64"]


# ---------------------------------------------------------------------------
# Edge cases
# ---------------------------------------------------------------------------

class TestEdgeCases:
    def test_empty_input(self):
        m = parse_module("")
        assert len(m.functions) == 0
        assert len(m.parse_errors) == 1
        assert "Empty" in m.parse_errors[0]

    def test_whitespace_only(self):
        m = parse_module("   \n\n  \n")
        assert len(m.functions) == 0
        assert "Empty" in m.parse_errors[0]

    def test_comments_only(self):
        m = parse_module("; Just a comment\n; Another comment\n")
        assert len(m.functions) == 0
        assert len(m.parse_errors) == 0

    def test_declarations_only(self):
        ir = "declare void @foo()\n"
        m = parse_module(ir)
        assert len(m.functions) == 1
        assert "@foo" in m.functions
        assert not m.functions["@foo"].is_definition

    def test_function_name_property(self):
        m = parse_module(_load_journey1_ir())
        assert m.functions["@_ori_add"].name == "_ori_add"
        assert m.functions["@ori_panic_cstr"].name == "ori_panic_cstr"
        assert m.functions["@main"].name == "main"

    def test_is_user_function(self):
        m = parse_module(_load_journey1_ir())
        assert m.functions["@_ori_add"].is_user_function
        assert m.functions["@_ori_main"].is_user_function
        assert not m.functions["@main"].is_user_function
        assert not m.functions["@ori_panic_cstr"].is_user_function

    def test_is_runtime_decl(self):
        m = parse_module(_load_journey1_ir())
        assert m.functions["@ori_panic_cstr"].is_runtime_decl
        assert not m.functions["@_ori_add"].is_runtime_decl
        assert not m.functions["@main"].is_runtime_decl

    def test_is_llvm_intrinsic(self):
        m = parse_module(_load_journey1_ir())
        assert m.functions["@llvm.sadd.with.overflow.i64"].is_llvm_intrinsic
        assert not m.functions["@_ori_add"].is_llvm_intrinsic


# ---------------------------------------------------------------------------
# Quoted function names (Section 04.1)
# ---------------------------------------------------------------------------

_QUOTED_IR = textwrap.dedent("""\
    define fastcc i64 @"_ori_first$24m$24int_int"(i64 %0) #0 {
    entry:
      ret i64 %0
    }

    declare void @"ori_str_from_raw"(ptr, ptr, i64)

    attributes #0 = { nounwind }
""")


class TestQuotedFunctionNames:
    def test_no_parse_errors(self):
        m = parse_module(_QUOTED_IR)
        assert not m.parse_errors, f"Parse errors: {m.parse_errors}"

    def test_quoted_definition_parsed(self):
        m = parse_module(_QUOTED_IR)
        assert '@"_ori_first$24m$24int_int"' in m.functions

    def test_quoted_name_property(self):
        m = parse_module(_QUOTED_IR)
        func = m.functions['@"_ori_first$24m$24int_int"']
        # .name should be the clean identifier without @ or quotes
        assert func.name == '_ori_first$24m$24int_int'

    def test_quoted_raw_name_property(self):
        m = parse_module(_QUOTED_IR)
        func = m.functions['@"_ori_first$24m$24int_int"']
        assert func.raw_name == '@"_ori_first$24m$24int_int"'

    def test_quoted_is_user_function(self):
        m = parse_module(_QUOTED_IR)
        func = m.functions['@"_ori_first$24m$24int_int"']
        # Starts with _ori_ so it's a user function
        assert func.is_user_function

    def test_quoted_declaration_parsed(self):
        m = parse_module(_QUOTED_IR)
        assert '@"ori_str_from_raw"' in m.functions

    def test_quoted_decl_is_runtime(self):
        m = parse_module(_QUOTED_IR)
        func = m.functions['@"ori_str_from_raw"']
        assert func.is_runtime_decl

    def test_quoted_function_blocks(self):
        m = parse_module(_QUOTED_IR)
        func = m.functions['@"_ori_first$24m$24int_int"']
        assert len(func.blocks) == 1
        assert func.blocks[0].label == "entry"
        assert func.instruction_count == 1

    def test_mixed_bare_and_quoted(self):
        ir = textwrap.dedent("""\
            define fastcc i64 @_ori_bare(i64 %0) {
            entry:
              ret i64 %0
            }
            define fastcc i64 @"_ori_quoted$gen"(i64 %0) {
            entry:
              ret i64 %0
            }
        """)
        m = parse_module(ir)
        assert "@_ori_bare" in m.functions
        assert '@"_ori_quoted$gen"' in m.functions
        user = m.user_functions()
        assert len(user) == 2


# ---------------------------------------------------------------------------
# Invoke instructions (Section 04.2)
# ---------------------------------------------------------------------------

_INVOKE_IR = textwrap.dedent("""\
    define fastcc void @_ori_f(ptr %0) #0 {
    entry:
      %result = invoke fastcc i64 @_ori_g(i64 42) to label %normal unwind label %cleanup

    normal:
      ret void

    cleanup:
      %lp = landingpad { ptr, i32 } cleanup
      resume { ptr, i32 } %lp
    }

    declare fastcc i64 @_ori_g(i64)
    attributes #0 = { nounwind uwtable }
""")


class TestInvokeInstructions:
    def test_invoke_parsed_as_instruction(self):
        m = parse_module(_INVOKE_IR)
        f = m.functions["@_ori_f"]
        entry = f.blocks[0]
        invoke_instr = entry.instructions[0]
        assert invoke_instr.opcode == "invoke"
        assert invoke_instr.result == "%result"

    def test_invoke_branch_targets(self):
        from ir_utils import extract_branch_targets
        m = parse_module(_INVOKE_IR)
        f = m.functions["@_ori_f"]
        entry = f.blocks[0]
        invoke_instr = entry.instructions[0]
        targets = extract_branch_targets(invoke_instr.text)
        assert set(targets) == {"normal", "cleanup"}


_SWITCH_IR = textwrap.dedent("""\
    define fastcc i64 @_ori_to_code(i64 %0) #0 {
    bb0:
      switch i64 %0, label %bb4 [
        i64 0, label %bb1
        i64 1, label %bb2
        i64 2, label %bb3
      ]

    bb1:
      ret i64 0

    bb2:
      ret i64 1

    bb3:
      ret i64 2

    bb4:
      unreachable
    }

    attributes #0 = { nounwind }
""")


def _parse_switch_ir():
    return parse_module(_SWITCH_IR)


class TestMultiLineSwitch:
    """Regression test: multi-line switch instructions must be joined."""

    def test_no_parse_errors(self):
        m = _parse_switch_ir()
        assert not m.parse_errors

    def test_block_count(self):
        m = _parse_switch_ir()
        f = m.functions["@_ori_to_code"]
        assert len(f.blocks) == 5

    def test_switch_is_single_instruction(self):
        """The multi-line switch should be ONE instruction, not split."""
        m = _parse_switch_ir()
        f = m.functions["@_ori_to_code"]
        bb0 = f.blocks[0]
        assert len(bb0.instructions) == 1
        assert bb0.instructions[0].opcode == "switch"

    def test_switch_contains_all_labels(self):
        """All case labels must be present in the joined instruction text."""
        m = _parse_switch_ir()
        f = m.functions["@_ori_to_code"]
        bb0 = f.blocks[0]
        text = bb0.instructions[0].text
        assert "bb1" in text
        assert "bb2" in text
        assert "bb3" in text
        assert "bb4" in text

    def test_extract_branch_targets_from_switch(self):
        """extract_branch_targets must find ALL switch targets."""
        from ir_utils import extract_branch_targets
        m = _parse_switch_ir()
        f = m.functions["@_ori_to_code"]
        bb0 = f.blocks[0]
        targets = extract_branch_targets(bb0.instructions[0].text)
        assert set(targets) == {"bb1", "bb2", "bb3", "bb4"}

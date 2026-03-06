"""Tests for binary_metrics.py — binary quality scoring."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from binary_metrics import compute_binary_metrics, parse_section_sizes


class TestJourney1:
    def test_correct_results(self):
        bm = compute_binary_metrics(eval_exit=33, aot_exit=33, expected=33)
        assert bm.eval_correct is True
        assert bm.aot_correct is True
        assert bm.paths_match is True
        assert bm.hard_fail is False
        assert bm.defects == 0

    def test_wrong_aot(self):
        bm = compute_binary_metrics(eval_exit=33, aot_exit=34, expected=33)
        assert bm.eval_correct is True
        assert bm.aot_correct is False
        assert bm.paths_match is False
        assert bm.hard_fail is True
        assert bm.defects >= 1

    def test_eval_aot_mismatch(self):
        bm = compute_binary_metrics(eval_exit=33, aot_exit=34, expected=33)
        assert bm.hard_fail is True
        # Wrong AOT (1) + mismatch (3) = 4
        assert bm.defects >= 4

    def test_crash_detection(self):
        bm = compute_binary_metrics(eval_exit=33, aot_exit=139, expected=33)
        assert bm.hard_fail is True
        assert bm.defects >= 3  # Crash = 3

    def test_compilation_failure(self):
        bm = compute_binary_metrics(eval_exit=33, aot_exit=None, expected=33)
        assert bm.hard_fail is True
        assert bm.aot_correct is False

    def test_no_section_sizes(self):
        bm = compute_binary_metrics(eval_exit=33, aot_exit=33, expected=33)
        assert bm.binary_size is None
        assert bm.text_size is None


class TestSectionSizes:
    def test_parse_size_output(self):
        text = """.text    1234
.rodata   567
.bss      128
.debug_info 4096
Total     5921
"""
        sizes = parse_section_sizes(text)
        assert sizes[".text"] == 1234
        assert sizes[".rodata"] == 567
        assert sizes[".bss"] == 128

    def test_section_sizes_in_metrics(self):
        sections_text = ".text    1000\n.rodata   500\n"
        bm = compute_binary_metrics(
            eval_exit=0, aot_exit=0, expected=0,
            sections_text=sections_text,
        )
        assert bm.text_size == 1000
        assert bm.rodata_size == 500
        assert bm.binary_size == 1500

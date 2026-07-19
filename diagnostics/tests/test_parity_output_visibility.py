import subprocess
from pathlib import Path


DIAGNOSTICS = Path(__file__).resolve().parents[1]


def test_render_captured_stream_prints_every_line_and_empty_marker(tmp_path):
    stream = tmp_path / "stream.txt"
    stream.write_text("\n".join(f"line-{index}" for index in range(1, 9)) + "\n")
    empty = tmp_path / "empty.txt"
    empty.touch()

    command = (
        f'source "{DIAGNOSTICS / "_common.sh"}"; '
        f'render_captured_stream stdout "{stream}"; '
        f'render_captured_stream stderr "{empty}"'
    )
    result = subprocess.run(
        ["bash", "-c", command],
        check=True,
        capture_output=True,
        text=True,
    )

    for index in range(1, 9):
        assert f"  │ line-{index}" in result.stdout
    assert "  stderr:\n  │ (empty)\n" in result.stdout
    assert "truncated" not in result.stdout


def test_parity_comparators_use_full_stream_renderer():
    dual = (DIAGNOSTICS / "dual-exec-debug.sh").read_text()
    debug_release = (DIAGNOSTICS / "debug-release-compare.sh").read_text()

    assert "head -5" not in dual
    assert "head -20" not in dual
    assert dual.count("render_captured_stream") >= 5
    assert debug_release.count("render_captured_stream") >= 8

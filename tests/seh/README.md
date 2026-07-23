# SEH entry-point witness kit

Manual, Windows-only verification of the `@main` entry-point wrapper under the
SEH (`x86_64-pc-windows-msvc`) exception-handling model. Not wired into
`test-all` — SEH execution requires a native Windows process, so this kit is run
deliberately against a native `ori.exe`.

Full workflow, toolchain prerequisites, and exit-code interpretation:
`docs/development/testing-seh-from-wsl.md`.

## Contents

| Path | Role |
|---|---|
| `witnesses/*.ori` | four programs covering both SEH exit sites plus a borrowed control |
| `expected.json` | the contract a conforming compiler must satisfy, plus the known defect signature |
| `record-witnesses.ps1` | compiles and runs each witness, recording exit codes and streams as JSONL |
| `verify-results.py` | checks a recorded JSONL against `expected.json`; fails closed |

## Run

```powershell
.\record-witnesses.ps1 -OriExe <path\to\ori.exe> -Commit <sha> -Arm cured -ResultPath .\results.jsonl
```

```bash
python3 verify-results.py results.jsonl --arm cured
```

`PASS` means every witness matched the contract. A witness exiting `0xC0000374`
(`STATUS_HEAP_CORRUPTION`) indicates a double free in the entry-point wrapper's
ownership handling.

## Fail-closed guarantees

The kit is built so that a broken or absent measurement cannot read as success:

- The recorder deletes each destination executable before compiling and requires
  BOTH a zero build exit status AND a newly produced file. A failed compile
  records `build_ok=false` rather than running a previous run's binary.
- The results file is truncated per run unless `-AppendResults` is passed, so a
  recording cannot inherit rows from an earlier one.
- The verifier rejects duplicate `(arm, witness)` records instead of collapsing
  them, and fails on a missing file, an empty file, an absent arm, or any
  witness with `build_ok=false`.

## Regression arm

To bind a fix to a native red -> green, record a second arm from a pre-fix
compiler built in its own worktree and target directory, then verify it: the
pre-fix arm is expected to FAIL, which is what proves the check has teeth.

```bash
python3 verify-results.py results.jsonl --arm precure   # expected to FAIL
```

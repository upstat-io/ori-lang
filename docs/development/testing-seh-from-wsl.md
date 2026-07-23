# Testing the SEH / MSVC exception-handling path from WSL2

Ori selects its exception-handling model from the target triple
(`EhModel::from_triple`): a GNU/Linux target uses the Itanium model
(`invoke` / `landingpad`), while an MSVC target
(`x86_64-pc-windows-msvc`) uses Structured Exception Handling — the runtime
`ori_try_call` plus a `__try` / `__except` frame.

A Linux-hosted `ori` can *emit* SEH IR but cannot *execute* it: the produced
object is a PE/COFF artifact whose `__try` frame only runs under a Windows
process. To exercise the SEH path end to end you compile with a native Windows
toolchain and run the resulting `.exe`. This guide runs both legs from a single
WSL2 session using Windows interop (`powershell.exe` / a produced `.exe` are
directly runnable from WSL when interop is enabled).

## Prerequisites (Windows side)

| Component | Purpose |
|---|---|
| Native Windows `rustup` toolchain (`x86_64-pc-windows-msvc`) | builds a native `ori.exe` / `ori_rt.lib` |
| Visual Studio Build Tools (MSVC `link.exe`) | links the PE/COFF artifact |
| LLVM matching `llvm-sys-<NNN>` (e.g. LLVM 21.1.x, win64) | satisfies the `llvm-sys` build |

Set the LLVM prefix explicitly. The checked-in `.cargo/config.toml` hardcodes a
Linux LLVM path in its `[env]` block, so a Windows build must override it:

```powershell
$env:LLVM_SYS_211_PREFIX = "<LLVM_DIR>"   # win64 LLVM matching llvm-sys-211
```

## Leg 1 — fast structural check (Linux, seconds)

The Linux compiler emits real SEH IR when the target is threaded through the
`--target=<triple>` form (the `=`-form threads the triple into the build; a bare
`--target <triple>` does not on the default `ori build` path). This verifies the
entry-wrapper lowering that SEH will execute, without a Windows rebuild:

```bash
ORI_DUMP_ENTRY_OWNERSHIP=1 ori build <program>.ori \
  --target=x86_64-pc-windows-msvc --emit=llvm-ir -o out.ll
```

The diagnostic banner reports `eh_model=Seh, can_unwind=true` and marks the
`seh_success` / `seh_caught` exit sites `active`; the emitted IR contains
`call ... @ori_try_call(...)`. This is the SEH structural companion to the
native run — it proves *what* will execute, not *that* it executes correctly.

## Leg 2 — native execution (Windows via WSL interop)

Build a native `ori.exe` and `ori_rt.lib` in a Windows checkout, using the LLVM
override plus an explicit target directory:

```powershell
$env:LLVM_SYS_211_PREFIX = "<LLVM_DIR>"
$env:CARGO_TARGET_DIR    = "<TARGET_DIR>"
cargo build --release -p oric -p ori_rt
```

Runtime discovery finds `ori_rt.lib` beside the compiler binary, so keep them in
the same `release` directory. Compile a program and run it, capturing the three
observables that matter — stdout, stderr, and the exact process exit code:

```powershell
& "<TARGET_DIR>\release\ori.exe" build program.ori -o program.exe
$p = Start-Process -FilePath program.exe -ArgumentList @("arg1","arg2") `
     -NoNewWindow -PassThru -Wait `
     -RedirectStandardOutput out.txt -RedirectStandardError err.txt
"exit={0} (0x{1:X8})" -f $p.ExitCode, ($p.ExitCode -band 0xFFFFFFFF)
```

## Regression arm (red -> green on real SEH)

To bind a fix to a native red -> green, build two isolated arms from distinct
commits and run the same programs through each. Use two **detached** worktrees
with **distinct** target directories so an incremental artifact from one arm
cannot contaminate the other:

```powershell
git worktree add --detach <WORKTREE_BASE> <baseline-commit>
git worktree add --detach <WORKTREE_FIX>  <fix-commit>
# build each with its own CARGO_TARGET_DIR, then run the same programs through both
```

## Interpreting exit codes

Record the exact NT status, never a generic "nonzero" — the specific code names
the failure:

| Exit code | Meaning |
|---|---|
| `0` | normal completion |
| small positive integer | an `@main () -> int` returning that value (e.g. `args.len()`) |
| `1` | an Ori `panic` reached the process boundary (message on stderr) |
| `0xC0000005` | access violation |
| `0xC0000374` | heap corruption (a double-free or freed-block metadata corruption) |

A clean SEH result is a small deterministic exit code plus the expected
stdout/stderr; a memory-management defect surfaces as a `0xCxxxxxxx` NT status.

## Witness programs

A runnable kit lives at `tests/seh/` — the four witness sources, an
`expected.json` contract, a recorder that persists exit codes and streams as
JSONL, and `verify-results.py` which checks a recording against the contract and
fails closed. See `tests/seh/README.md`.

The witnesses exercise both SEH exit sites of the `@main` entry-point wrapper:

```ori
// normal completion — exercises the SEH success exit
@main (args: [str]) -> void = for arg in args do print(msg: arg);

// panic mid-iteration — exercises the SEH caught exit after the loop starts
@main (args: [str]) -> void = for arg in args do panic(msg: arg);

// panic before the loop — exercises the SEH caught exit before iteration
@main (args: [str]) -> void = {
    if args.len() > 0 then panic(msg: "before-loop");
    for arg in args do print(msg: arg);
}

// borrowed control — the entry wrapper retains ownership; exit tracks the value
@main (args: [str]) -> int = args.len();
```

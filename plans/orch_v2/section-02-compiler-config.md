---
section: "02"
title: CompilerConfig
status: not-started
tier: 0
goal: Validate all configuration upfront before compilation begins
sections:
  - id: "2.1"
    title: RawOptions Parser
    status: not-started
  - id: "2.2"
    title: CompilerConfig::resolve()
    status: not-started
  - id: "2.3"
    title: Command-Specific Config
    status: not-started
  - id: "2.4"
    title: ConfigError Diagnostics
    status: not-started
  - id: "2.5"
    title: Section Completion Checklist
    status: not-started
---

# Section 02: CompilerConfig

**Status:** 📋 Planned
**Goal:** Validate all CLI options upfront before any compilation begins. Invalid or conflicting options are caught immediately with clear error messages.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Module 2
> **Inspired by**: Zig's `Config.resolve()` pattern
> **Location**: `compiler/oric/src/config.rs`

---

## 2.1 RawOptions Parser

Parse CLI arguments into unvalidated raw options. Replaces manual per-command parsing.

- [ ] **Implement**: `RawOptions` struct in `compiler/oric/src/config.rs`
  - [ ] `command: String` — the subcommand name
  - [ ] `positional: Vec<String>` — positional arguments
  - [ ] `flags: HashMap<String, Option<String>>` — flags with optional values
  - [ ] `RawOptions::from_args(args: &[String])` — parse from CLI args
  - [ ] Handle `-o <value>` (flag with space-separated value)
  - [ ] Handle `--key=value` (flag with `=` separator)
  - [ ] Handle `--flag` (boolean flags)
  - [ ] Handle `-v` (short flags)
  - [ ] **Rust Tests**: `compiler/oric/src/config.rs`
    - [ ] `test_parse_simple_command` — `ori check file.ori`
    - [ ] `test_parse_flags` — `ori build --release --opt=2`
    - [ ] `test_parse_flag_with_space` — `ori build -o output`
    - [ ] `test_parse_short_flags` — `ori run -c`
    - [ ] `test_parse_no_command` — empty args
    - [ ] `test_parse_unknown_positional` — extra positionals

---

## 2.2 CompilerConfig::resolve()

Validate raw options into a consistent, immutable configuration.

- [ ] **Implement**: `CompilerConfig` struct (no `Default` impl — must go through `resolve()`)
  - [ ] `command: Command` — validated command enum
  - [ ] `input: InputSource` — validated input path(s)
  - [ ] `output_format: OutputFormat` — Terminal / JSON / SARIF
  - [ ] `color_mode: ColorMode` — Auto / Always / Never
  - [ ] `verbosity: Verbosity` — Normal / Verbose / Quiet
  - [ ] `build: Option<BuildConfig>` — only for `build` command
  - [ ] `test: Option<TestConfig>` — only for `test` command
  - [ ] `format: Option<FormatConfig>` — only for `fmt` command

- [ ] **Implement**: `CompilerConfig::resolve(raw: RawOptions) -> Result<Self, Vec<ConfigError>>`
  - [ ] Validate command name (with "did you mean?" suggestion)
  - [ ] Validate required positional arguments per command
  - [ ] Validate flag applicability (e.g., `--release` only for `build`)
  - [ ] Detect conflicting flags (e.g., `--lib` + `--dylib`)
  - [ ] Validate option values (e.g., `--opt=7` is invalid)
  - [ ] Accumulate ALL errors before returning (not just first)
  - [ ] **Rust Tests**: `compiler/oric/src/config.rs`
    - [ ] `test_resolve_check` — valid check command
    - [ ] `test_resolve_build_release` — valid build with --release
    - [ ] `test_resolve_unknown_command` — error with suggestion
    - [ ] `test_resolve_missing_file` — error for missing required arg
    - [ ] `test_resolve_conflicting_flags` — `--lib` + `--dylib` error
    - [ ] `test_resolve_invalid_opt_level` — `--opt=7` error
    - [ ] `test_resolve_multiple_errors` — accumulates all errors
    - [ ] `test_resolve_inapplicable_flag` — `--release` for check command

- [ ] **Implement**: `Command` enum
  - [ ] Variants: `Run`, `Build`, `Check`, `Test`, `Fmt`, `Parse`, `Lex`, `Target`, `Targets`, `Demangle`, `Explain`, `Help`, `Version`
  - [ ] `Command::parse(name: &str) -> Option<Command>`
  - [ ] `Command::handler(&self) -> fn(&mut Session) -> Outcome<()>` (for Section 10)

- [ ] **Implement**: `InputSource` enum
  - [ ] `SingleFile(PathBuf)` — single file commands (check, run, build)
  - [ ] `Directory(PathBuf)` — directory commands (test, fmt)
  - [ ] `None` — commands that don't need input (version, help, targets)

---

## 2.3 Command-Specific Config

Typed configuration for commands that have complex options.

- [ ] **Implement**: `BuildConfig` — migrate from existing `BuildOptions`
  - [ ] Reuse types: `OptLevel`, `DebugLevel`, `EmitType`, `LinkMode`, `LtoMode`
  - [ ] Validate conflicting options during `resolve()`
  - [ ] `release: bool`, `target: Option<String>`, `opt_level`, `debug_level`
  - [ ] `output: Option<PathBuf>`, `emit: Option<EmitType>`
  - [ ] `lib: bool`, `dylib: bool`, `wasm: bool`
  - [ ] `lto: LtoMode`, `verbose: bool`

- [ ] **Implement**: `TestConfig` — migrate from existing `TestRunnerConfig`
  - [ ] `filter: Option<String>`, `verbose: bool`, `parallel: bool`
  - [ ] `coverage: bool`, `backend: Backend`

- [ ] **Implement**: `FormatConfig`
  - [ ] `check: bool`, `diff: bool`

- [ ] **Rust Tests**:
  - [ ] `test_build_config_defaults` — default build options
  - [ ] `test_build_config_release` — release overrides opt + debug
  - [ ] `test_test_config_defaults` — default test options
  - [ ] `test_format_config_check_mode` — check mode from flag

---

## 2.4 ConfigError Diagnostics

Human-readable error messages for configuration problems.

- [ ] **Implement**: `ConfigError` enum in `compiler/oric/src/config.rs`
  - [ ] `UnknownCommand { name: String, similar: Option<String> }`
  - [ ] `MissingRequiredArg { command: String, arg_name: String }`
  - [ ] `InvalidFlagValue { flag: String, value: String, expected: String }`
  - [ ] `ConflictingFlags { flag_a: String, flag_b: String }`
  - [ ] `InapplicableFlag { flag: String, command: String }`
  - [ ] `UnknownFlag { flag: String, similar: Option<String> }`

- [ ] **Implement**: `Display` for `ConfigError` with actionable messages
  - [ ] `UnknownCommand`: "unknown command 'buidl'. Did you mean 'build'?"
  - [ ] `ConflictingFlags`: "cannot use '--lib' and '--dylib' together"
  - [ ] `InapplicableFlag`: "'--release' is only valid for 'build' command"

- [ ] **Implement**: `suggest_command(name: &str) -> Option<String>` — Levenshtein distance
  - [ ] Use existing `suggest.rs` infrastructure if available
  - [ ] Threshold: edit distance <= 2

- [ ] **Rust Tests**: `compiler/oric/src/config.rs`
  - [ ] `test_config_error_display` — verify all error messages are actionable
  - [ ] `test_suggest_command` — "chekc" → "check", "bild" → "build"

---

## 2.5 Section Completion Checklist

- [ ] `RawOptions::from_args` parses all current CLI patterns
- [ ] `CompilerConfig::resolve` validates all current commands
- [ ] All existing CLI flags are supported
- [ ] Error messages include "did you mean?" suggestions
- [ ] Conflicting flags are detected
- [ ] No regressions: `./test-all.sh` passes
- [ ] Public API documented with `///` doc comments
- [ ] Module added to `compiler/oric/src/lib.rs` exports

**Exit Criteria:** `CompilerConfig::resolve()` can parse and validate every CLI invocation that `main.rs` currently handles, producing clear errors for invalid inputs.

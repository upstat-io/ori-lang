---
section: "08"
title: Command Table
status: not-started
tier: 2
goal: Declarative command definitions with auto-generated help and completion
sections:
  - id: "8.1"
    title: Command Definition Types
    status: not-started
  - id: "8.2"
    title: Command Table
    status: not-started
  - id: "8.3"
    title: Help Generation
    status: not-started
  - id: "8.4"
    title: Shell Completion
    status: not-started
  - id: "8.5"
    title: Section Completion Checklist
    status: not-started
---

# Section 08: Command Table

**Status:** 📋 Planned
**Goal:** Define commands as data structures rather than procedural dispatch, enabling auto-generated help text, shell completion, and consistent argument validation.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Module 6
> **Inspired by**: Elm's declarative command list
> **Location**: `compiler/oric/src/command_table.rs`
> **Depends on**: Section 2 (CompilerConfig)

---

## 8.1 Command Definition Types

- [ ] **Implement**: `CommandDef` struct in `compiler/oric/src/command_table.rs`
  - [ ] `name: &'static str` — command name (e.g., "check", "run")
  - [ ] `summary: &'static str` — one-line help text
  - [ ] `description: &'static str` — detailed description
  - [ ] `args: &'static [ArgDef]` — expected positional arguments
  - [ ] `flags: &'static [FlagDef]` — accepted flags
  - [ ] `examples: &'static [(&'static str, &'static str)]` — usage examples
  - [ ] `hidden: bool` — hide from help (for debug commands)

- [ ] **Implement**: `ArgDef` struct
  - [ ] `name: &'static str` — argument name for help text
  - [ ] `required: bool` — whether argument is required
  - [ ] `description: &'static str`

- [ ] **Implement**: `FlagDef` struct
  - [ ] `long: &'static str` — long form (e.g., "release")
  - [ ] `short: Option<char>` — short form (e.g., 'c')
  - [ ] `takes_value: bool` — whether flag accepts a value
  - [ ] `value_name: Option<&'static str>` — display name for value (e.g., "LEVEL")
  - [ ] `description: &'static str`

- [ ] **Rust Tests**: `compiler/oric/src/command_table.rs`
  - [ ] `test_command_def_construction` — create static command defs
  - [ ] `test_arg_def` — required and optional args
  - [ ] `test_flag_def` — flags with/without values

---

## 8.2 Command Table

Single source of truth for all CLI commands.

- [ ] **Implement**: `static COMMANDS: &[CommandDef]` — all commands
  - [ ] `run` — Run/evaluate an Ori program
  - [ ] `build` — Compile to native executable (AOT)
  - [ ] `test` — Run tests
  - [ ] `check` — Type check a file
  - [ ] `fmt` — Format Ori source files
  - [ ] `target` — Manage cross-compilation targets
  - [ ] `targets` — List supported compilation targets
  - [ ] `demangle` — Demangle an Ori symbol name
  - [ ] `explain` — Explain an error code
  - [ ] `parse` — Parse and display AST info (hidden)
  - [ ] `lex` — Tokenize and display tokens (hidden)
  - [ ] `help` — Show help message
  - [ ] `version` — Show version information

- [ ] **Implement**: `fn find_command(name: &str) -> Option<&'static CommandDef>`
  - [ ] Linear search through COMMANDS (few commands, fast enough)

- [ ] **Implement**: `fn suggest_command(name: &str) -> Option<&'static str>`
  - [ ] Levenshtein distance for "did you mean?" on typos

- [ ] **Rust Tests**: `compiler/oric/src/command_table.rs`
  - [ ] `test_find_command` — find each command by name
  - [ ] `test_find_unknown_command` — returns None
  - [ ] `test_suggest_command` — "chekc" → "check"
  - [ ] `test_all_commands_have_summaries` — no empty summaries
  - [ ] `test_all_commands_have_examples` — each has at least one example

---

## 8.3 Help Generation

Auto-generate help text from command definitions.

- [ ] **Implement**: `fn print_usage(commands: &[CommandDef])`
  - [ ] Header with version info
  - [ ] "Usage: ori <command> [options]"
  - [ ] Command list with aligned summaries
  - [ ] Skip hidden commands
  - [ ] Match current `print_usage()` output format

- [ ] **Implement**: `fn print_command_help(cmd: &CommandDef)`
  - [ ] "Usage: ori <cmd> <args> [flags]"
  - [ ] Description
  - [ ] Arguments section
  - [ ] Flags section with aligned descriptions
  - [ ] Examples section

- [ ] **Rust Tests**: `compiler/oric/src/command_table.rs`
  - [ ] `test_print_usage_contains_all_visible` — all non-hidden commands present
  - [ ] `test_print_usage_excludes_hidden` — hidden commands not shown
  - [ ] `test_print_command_help` — per-command help is complete
  - [ ] `test_help_output_matches_current` — identical to current `print_usage()` output

---

## 8.4 Shell Completion

Generate completion scripts from command definitions.

- [ ] **Implement**: `fn generate_bash_completion(commands: &[CommandDef]) -> String`
  - [ ] Complete command names
  - [ ] Complete flag names per command
  - [ ] File completion for positional args

- [ ] **Implement**: `fn generate_zsh_completion(commands: &[CommandDef]) -> String`
  - [ ] Same features as bash but with zsh syntax

- [ ] **Implement**: `ori completions <shell>` command
  - [ ] Outputs completion script to stdout
  - [ ] Supported shells: bash, zsh, fish

- [ ] **Rust Tests**: `compiler/oric/src/command_table.rs`
  - [ ] `test_bash_completion_contains_commands` — all command names present
  - [ ] `test_zsh_completion_contains_flags` — flags for build command present

---

## 8.5 Section Completion Checklist

- [ ] All current commands defined in COMMANDS table
- [ ] Help output matches or improves on current `print_usage()`
- [ ] Per-command help works with `ori <cmd> --help`
- [ ] "Did you mean?" suggestions for unknown commands
- [ ] Shell completion scripts generated for bash/zsh
- [ ] No regressions: `./test-all.sh` passes

**Exit Criteria:** The manually-written `print_usage()` and per-command help strings are fully replaced by auto-generated output from the command table.

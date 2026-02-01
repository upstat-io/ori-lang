# Phase 1: Manifest & Lock File

**Goal**: Parse and validate oripk.toml and oripk.lock

**Status**: ⬜ Not Started

---

## 1.1 Manifest Parsing (oripk.toml)

### Project Section

- [ ] **Implement**: Parse `[project]` section
  - [ ] `name` — scoped package name (@scope/name)
  - [ ] `version` — strict semver (major.minor.patch)
  - [ ] `description` — required for publish
  - [ ] `license` — SPDX expression
  - [ ] `authors` — array of strings
  - [ ] `repository` — URL
  - [ ] `keywords` — array
  - [ ] `categories` — array
  - [ ] `funding` — optional URL
  - [ ] `edition` — Ori edition
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/project.rs`

- [ ] **Implement**: Parse `ori` version requirement
  - [ ] Required field
  - [ ] Strict semver format
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/ori_version.rs`

- [ ] **Implement**: Parse `[project.entry]`
  - [ ] `lib` — library entry point
  - [ ] `main` — binary entry point
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/entry.rs`

### Dependencies Section

- [ ] **Implement**: Parse `[dependencies]`
  - [ ] Stdlib deps: `std.json` (no version)
  - [ ] Exact versions: `@scope/name = "1.0.0"`
  - [ ] With features: `[dependencies.@scope/name]`
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

- [ ] **Implement**: Parse git dependencies
  - [ ] `git = "url"` + `rev = "hash"`
  - [ ] Mark as non-publishable
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

- [ ] **Implement**: Parse path dependencies
  - [ ] `path = "../local"`
  - [ ] Mark as non-publishable
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

- [ ] **Implement**: Parse `[dev-dependencies]`
  - [ ] Same format as dependencies
  - [ ] Isolated feature resolution
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

- [ ] **Implement**: Parse `[target."cfg(...)".dependencies]`
  - [ ] Platform-specific deps
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

### Features Section

- [ ] **Implement**: Parse `[features]`
  - [ ] `default` — default features
  - [ ] Feature enables deps
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/features.rs`

### Other Sections

- [ ] **Implement**: Parse `[registries]`
  - [ ] Private registry URLs
  - [ ] Auth token reference
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/registries.rs`

- [ ] **Implement**: Parse `[scripts]`
  - [ ] Simple string commands
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/scripts.rs`

- [ ] **Implement**: Parse `[publish]`
  - [ ] `registry` — target registry
  - [ ] `include` / `exclude` — file patterns
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/publish.rs`

---

## 1.2 Manifest Validation

- [ ] **Implement**: Package name validation
  - [ ] Lowercase a-z, 0-9, hyphens only
  - [ ] Max 64 characters
  - [ ] Reserved names rejected
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/validate.rs`

- [ ] **Implement**: Version format validation
  - [ ] Strict major.minor.patch
  - [ ] No build metadata
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/validate.rs`

- [ ] **Implement**: Required fields validation
  - [ ] `name`, `version`, `ori` required
  - [ ] `description` required for publish
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/validate.rs`

- [ ] **Implement**: Unknown fields handling
  - [ ] Warn and ignore
  - [ ] Forward compatibility
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/validate.rs`

---

## 1.3 Lock File Parsing (oripk.lock)

The lock file contains **only checksums** for security verification, not for version locking.

- [ ] **Implement**: Parse `[checksums]` section
  - [ ] Format: `"@scope/name:version" = "sha256:..."`
  - [ ] Validate checksum format
  - [ ] **Rust Tests**: `ori_pkg/src/lock/checksums.rs`

---

## 1.4 Lock File Generation

- [ ] **Implement**: Generate lock file from resolution
  - [ ] Alphabetical ordering by package:version
  - [ ] SHA256 checksums only
  - [ ] **Rust Tests**: `ori_pkg/src/lock/generate.rs`

- [ ] **Implement**: Staleness detection
  - [ ] Detect when oripk.toml deps changed
  - [ ] Auto-regenerate on `ori sync` (no error)
  - [ ] **Rust Tests**: `ori_pkg/src/lock/staleness.rs`

---

## 1.5 Manifest Formatting

- [ ] **Implement**: Format oripk.toml via `ori fmt`
  - [ ] Integrate with existing formatter
  - [ ] Consistent key ordering
  - [ ] **Rust Tests**: `ori_fmt/src/toml.rs`

---

## 1.6 Phase Completion Checklist

- [ ] oripk.toml parsing complete
- [ ] oripk.lock parsing complete (checksums only)
- [ ] Validation with helpful errors
- [ ] Unknown fields warn, don't error
- [ ] Formatting integration
- [ ] Run full test suite: `./test-all`

**Exit Criteria**: Can parse and validate oripk.toml and oripk.lock files

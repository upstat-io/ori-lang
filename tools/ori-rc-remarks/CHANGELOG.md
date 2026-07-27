# ori-rc-remarks

## 2026.7.27-alpha.1

BREAKING (public API). Two transitions are disposed together rather than shipped
as successive raw shapes.

- `IngestError` is `#[non_exhaustive]`. An external consumer branching on error
  kind now requires a wildcard arm; a future failure kind is no longer a
  breaking change.
- Each variant carries one named payload — `JsonFailure`, `UnsupportedVersion`,
  `MissingHeader` — whose fields are PRIVATE. Destructuring
  `Json { line, source }` no longer compiles; use `line()`, `source()`,
  `found()`, or the enum-level `line()`. Reshaping a payload field is no longer
  a breaking change.
- This also disposes an EARLIER undeclared break: `IngestError` was a public
  struct with public `line` / `source` fields before it became a raw enum, and
  the crate version did not move. That transition is recorded here rather than
  left implicit.

Behavioral change in the same release:

- `ingest` refuses a remark that precedes any header
  (`IngestError::MissingStreamHeader`). Previously such a stream was admitted
  and analyzed under an assumed schema version, so an unversioned record could
  reach a consumer's verdict. The only headerless stream now accepted is an
  empty one, and the summary reports it as `empty stream (no header, no remarks)`.

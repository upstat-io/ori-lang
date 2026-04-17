<!-- GENERATED from scripts/plan_corpus/ — do not edit -->

# Plan Schema Reference

Auto-generated from Python dataclass definitions in `scripts/plan_corpus/schemas.py`.

## Status Enums

- **Plan statuses**: active, not-started, queued, research, resolved
- **Section statuses**: complete, in-progress, not-started
- **Overview statuses**: complete, in-progress, not-started, research
- **Fix statuses**: complete, in-progress, not-started
- **TPR statuses**: clean, findings, none, resolved
- **Completed plan statuses**: resolved

## File Classes

### Plan Index
**Pattern**: `plans/*/index.md`
**Required**: `name`, `full_name`, `status`
**Optional**: `inspired_by`, `order`, `parallel`, `references`, `reroute`, `reviewed`, `supersedes`

### Plan Section
**Pattern**: `plans/*/section-*.md`
**Required**: `section`, `title`, `status`, `reviewed`, `goal`, `success_criteria`, `sections`, `third_party_review`
**Optional**: `depends_on`, `inspired_by`, `touches`

### Roadmap Section
**Pattern**: `plans/roadmap/section-*.md`
**Required**: `section`, `title`, `status`, `reviewed`, `goal`, `sections`
**Optional**: `depends_on`, `last_verified`, `spec`, `third_party_review`, `tier`, `tpr_findings`, `verification_summary`

### Overview
**Pattern**: `plans/*/00-overview.md`
**Required**: `plan`, `title`, `status`
**Optional**: `references`, `reviewed`, `supersedes`

### Bug Tracker Section
**Pattern**: `plans/bug-tracker/section-*.md`
**Required**: `section`, `title`, `status`, `goal`
**Optional**: `sections`

### Fix Bug
**Pattern**: `plans/bug-tracker/fix-BUG-*.md`
**Required**: `bug`, `title`, `severity`, `status`, `goal`, `success_criteria`, `subsystem`, `found`, `source`, `third_party_review`
**Optional**: `depends_on`, `sections`, `touches`

### Completed Index
**Pattern**: `plans/completed/*/index.md`
**Required**: `name`, `full_name`, `status`
**Optional**: `order`, `parallel`, `reroute`

## Finding Severity

Impact classification — answers "how bad is this?". Set by the emitter at `Finding` construction time based on the finding's real-world significance. Ordered for comparison (IntEnum).

- `low` (ordinal 0)
- `medium` (ordinal 1)
- `high` (ordinal 2)
- `critical` (ordinal 3)

## Finding Outcome

Enforcement channel — answers "does this gate the check?". INDEPENDENT of Severity and set independently at emit time. `scripts/plan_corpus check` exits 1 iff any finding has `outcome == ERROR`; `WARNING` findings print but do not gate.

- `warning`
- `error`

`Finding.outcome` defaults to `ERROR` — pre-existing call sites (schema violations, parse errors) gate CI unchanged. The `_check_intel_recon_block` body validator explicitly opts into `WARNING` for `status: not-started` / `in-progress` PLAN_SECTION findings; `--strict-recon` escalates `not-started` WARNINGs to ERRORs. `outcome` is NOT included in `Finding.id` hash — backward-compat with saved reports.

## Finding Categories

### parse_error
- `crlf_boundary_drift`
- `duplicate_key`
- `invalid_utf8_bytes`
- `missing_opening_dashes`
- `multi_document`
- `non_mapping_root`
- `unclosed_frontmatter`
- `utf8_bom`
- `yaml_anchor`
- `yaml_merge_key`
- `yaml_syntax_error`
- `zero_width_before_fm`

### schema_violation
- `cross_field_invariant`
- `dep_id_full_path`
- `dep_id_malformed`
- `dep_id_unknown_name`
- `duplicate_plan_name`
- `enum_out_of_range`
- `missing_required_field`
- `unknown_field`
- `wrong_type`

### status_contradiction
- `cross_edge_temporal_drift`
- `fm_declared_vs_body_derived`
- `plan_active_all_sections_not_started`
- `plan_complete_with_open_sections`
- `tpr_stale_vs_edit`
- `tpr_status_none_with_date`
- `tpr_status_without_date`

### dag_conflict
- `blocked`
- `conflict`
- `cycle`
- `missing_dependency`
- `orphaned_plan`
- `redundant_dependency`
- `superseded`

### dead_reference
- `cross_plan_name_not_found`
- `plan_directory_not_found`
- `section_file_not_found`
- `spec_file_not_found`

### item_verification
- `hygiene_violation`
- `incomplete_checkbox`
- `missing_matrix_coverage`
- `missing_negative_pin`
- `missing_semantic_pin`
- `scope_gap`
- `weak_test`

### gap
- `leak_swallowed_error`
- `missing_index_md`
- `missing_recon_block`
- `recon_graph_unavailable`
- `unclassified_directory`
- `validation_bypass`

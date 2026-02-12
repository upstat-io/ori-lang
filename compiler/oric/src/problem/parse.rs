//! Parse-time problem definitions.
//!
//! Parse errors in Ori are rendered directly by `ori_parse::ParseError::to_queued_diagnostic()`.
//! This module is intentionally empty — it existed previously for a `ParseProblem` enum
//! that duplicated the rendering in `ori_parse`, creating a dual rendering path.
//! The canonical rendering now lives solely in `ori_parse::error`.

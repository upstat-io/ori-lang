//! Text Editing Infrastructure
//!
//! Provides tools for tracking and applying text edits to source code.

mod tracker;

pub use tracker::{ChangeTracker, TextEdit, EditConflict};

//! Tracing initialization for the Ori compiler.
//!
//! Controlled by environment variables:
//! - `ORI_LOG`: Filter string (`RUST_LOG` syntax). Falls back to `RUST_LOG`.
//! - `ORI_LOG_TREE`: Set to any value to enable hierarchical tree output.
//!
//! When neither `ORI_LOG` nor `RUST_LOG` is set, defaults to `warn` level.
//!
//! If `ORI_LOG` is set but contains a malformed filter directive, a warning
//! is printed to stderr and the fallback chain (`RUST_LOG` → `warn` default)
//! takes over. The warning ensures the user knows their explicit configuration
//! was not applied — silent swallowing of parse errors is a correctness
//! violation per CLAUDE.md.

use std::sync::OnceLock;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

static INIT: OnceLock<()> = OnceLock::new();

/// Initialize the tracing subscriber.
///
/// Safe to call multiple times — only the first call takes effect.
/// Uses `try_init()` instead of `init()` to tolerate a subscriber
/// that was already set by another component (e.g., test harness).
pub fn init() {
    INIT.get_or_init(|| {
        let filter = build_filter();

        let use_tree = std::env::var("ORI_LOG_TREE").is_ok();

        if use_tree {
            let _ = Registry::default()
                .with(
                    tracing_tree::HierarchicalLayer::new(2)
                        .with_targets(true)
                        .with_indent_lines(true)
                        .with_deferred_spans(true)
                        .with_bracketed_fields(true)
                        .with_writer(std::io::stderr),
                )
                .with(filter)
                .try_init();
        } else {
            let _ = Registry::default()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_writer(std::io::stderr)
                        .compact(),
                )
                .with(filter)
                .try_init();
        }
    });
}

/// Build the tracing filter from environment variables.
///
/// Priority: `ORI_LOG` → `RUST_LOG` → `"warn"` (default).
///
/// When `ORI_LOG` is set but malformed, warns to stderr and falls through
/// to the next source. This distinguishes "user explicitly set a broken
/// config" (warn + fallback) from "user didn't set any config" (silent
/// fallback).
fn build_filter() -> EnvFilter {
    // Try ORI_LOG first.
    match EnvFilter::try_from_env("ORI_LOG") {
        Ok(filter) => return filter,
        Err(e) => {
            // Distinguish "env var not set" (silent) from "parse error" (warn).
            // FromEnvError's kind field is private, so we check whether the
            // env var actually exists to determine which case we're in.
            if std::env::var("ORI_LOG").is_ok() {
                eprintln!(
                    "warning: ORI_LOG parse error: {e}; \
                     falling back to RUST_LOG or default filter"
                );
            }
        }
    }

    // Fall through to RUST_LOG, then to the "warn" default.
    EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("warn"))
}

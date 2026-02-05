//! Tracing initialization for the Ori compiler.
//!
//! Controlled by environment variables:
//! - `ORI_LOG`: Filter string (`RUST_LOG` syntax). Falls back to `RUST_LOG`.
//! - `ORI_LOG_TREE`: Set to any value to enable hierarchical tree output.
//!
//! When neither `ORI_LOG` nor `RUST_LOG` is set, defaults to `warn` level.

use std::sync::OnceLock;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

static INIT: OnceLock<()> = OnceLock::new();

/// Initialize the tracing subscriber.
///
/// Safe to call multiple times — only the first call takes effect.
pub fn init() {
    INIT.get_or_init(|| {
        let filter = EnvFilter::try_from_env("ORI_LOG")
            .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
            .unwrap_or_else(|_| EnvFilter::new("warn"));

        let use_tree = std::env::var("ORI_LOG_TREE").is_ok();

        if use_tree {
            Registry::default()
                .with(
                    tracing_tree::HierarchicalLayer::new(2)
                        .with_targets(true)
                        .with_indent_lines(true)
                        .with_deferred_spans(true)
                        .with_bracketed_fields(true)
                        .with_writer(std::io::stderr),
                )
                .with(filter)
                .init();
        } else {
            Registry::default()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_writer(std::io::stderr)
                        .compact(),
                )
                .with(filter)
                .init();
        }
    });
}

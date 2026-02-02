//! Tests for shape tracking.

use super::*;
use crate::context::{FormatConfig, TrailingCommas};

mod shape_tests {
    use super::*;

    #[test]
    fn shape_default() {
        let shape = Shape::default();
        assert_eq!(shape.width, 100);
        assert_eq!(shape.indent, 0);
        assert_eq!(shape.offset, 0);
    }

    #[test]
    fn shape_new() {
        let shape = Shape::new(80);
        assert_eq!(shape.width, 80);
        assert_eq!(shape.indent, 0);
        assert_eq!(shape.offset, 0);
    }

    #[test]
    fn shape_from_config() {
        let config = FormatConfig::with_max_width(120);
        let shape = Shape::from_config(&config);
        assert_eq!(shape.width, 120);
    }

    #[test]
    fn shape_consume() {
        let shape = Shape::new(100);
        let after = shape.consume(10);
        assert_eq!(after.width, 90);
        assert_eq!(after.offset, 10);
        assert_eq!(after.indent, 0); // Unchanged
    }

    #[test]
    fn shape_consume_saturates() {
        let shape = Shape::new(10);
        let after = shape.consume(20);
        assert_eq!(after.width, 0);
        assert_eq!(after.offset, 20);
    }

    #[test]
    fn shape_indent() {
        let shape = Shape::new(100);
        let indented = shape.indent(4);
        assert_eq!(indented.indent, 4);
        assert_eq!(indented.width, 96);
        assert_eq!(indented.offset, 0); // Unchanged
    }

    #[test]
    fn shape_indent_multiple() {
        let shape = Shape::new(100).indent(4).indent(4);
        assert_eq!(shape.indent, 8);
        assert_eq!(shape.width, 92);
    }

    #[test]
    fn shape_dedent() {
        let shape = Shape::new(100).indent(8);
        let dedented = shape.dedent(4);
        assert_eq!(dedented.indent, 4);
        assert_eq!(dedented.width, 96);
    }

    #[test]
    fn shape_dedent_saturates() {
        let shape = Shape::new(100).indent(4);
        let dedented = shape.dedent(8);
        assert_eq!(dedented.indent, 0);
    }

    #[test]
    fn shape_fits() {
        let shape = Shape::new(100).consume(90);
        assert!(shape.fits(10));
        assert!(!shape.fits(11));
        assert!(shape.fits(0));
    }

    #[test]
    fn shape_fits_str() {
        let shape = Shape::new(100).consume(95);
        assert!(shape.fits_str("hello"));
        assert!(!shape.fits_str("hello world"));
    }

    #[test]
    fn shape_next_line() {
        let shape = Shape::new(100).indent(4).consume(50);
        let next = shape.next_line(100);
        assert_eq!(next.width, 96); // 100 - 4 indent
        assert_eq!(next.offset, 4); // Reset to indent
        assert_eq!(next.indent, 4); // Preserved
    }

    #[test]
    fn shape_remaining() {
        let shape = Shape::new(100).consume(30);
        assert_eq!(shape.remaining(), 70);
    }

    #[test]
    fn shape_should_break() {
        let shape = Shape::new(100).consume(90);
        assert!(!shape.should_break(10));
        assert!(shape.should_break(11));
    }

    #[test]
    fn shape_for_nested() {
        let config = FormatConfig::default();
        let shape = Shape::new(100).indent(20).consume(30);
        let nested = shape.for_nested(&config);

        // Nested gets fresh width from indent, not from consumed position
        assert_eq!(nested.width, 80); // 100 - 20 indent
        assert_eq!(nested.indent, 20);
        assert_eq!(nested.offset, 20);
    }

    #[test]
    fn shape_for_block() {
        let config = FormatConfig::default();
        let shape = Shape::new(100).consume(20);
        let block = shape.for_block(&config);

        assert_eq!(block.indent, 4);
        assert_eq!(block.width, 96); // Fresh line with indent
        assert_eq!(block.offset, 4);
    }

    #[test]
    fn shape_for_continuation() {
        let config = FormatConfig::default();
        let shape = Shape::new(100).indent(8).consume(50);
        let cont = shape.for_continuation(&config);

        assert_eq!(cont.indent, 8);
        assert_eq!(cont.width, 92); // Fresh line
        assert_eq!(cont.offset, 8);
    }

    #[test]
    fn shape_after() {
        let shape = Shape::new(100);
        let after = shape.after("let x = ");
        assert_eq!(after.width, 92);
        assert_eq!(after.offset, 8);
    }

    #[test]
    fn shape_with_offset() {
        let shape = Shape::new(100);
        let aligned = shape.with_offset(20);
        assert_eq!(aligned.offset, 20);
        assert_eq!(aligned.width, 100); // Width unchanged
    }

    #[test]
    fn shape_at_line_start() {
        let shape = Shape::new(100);
        assert!(shape.at_line_start());

        let consumed = shape.consume(5);
        assert!(!consumed.at_line_start());

        let indented = Shape::new(100).indent(4);
        assert!(!indented.at_line_start()); // offset=0, indent=4

        let next = indented.next_line(100);
        assert!(next.at_line_start()); // offset=4, indent=4
    }
}

mod config_tests {
    use super::*;

    #[test]
    fn config_default() {
        let config = FormatConfig::default();
        assert_eq!(config.max_width, 100);
        assert_eq!(config.indent_size, 4);
        assert_eq!(config.trailing_commas, TrailingCommas::Always);
    }

    #[test]
    fn config_with_max_width() {
        let config = FormatConfig::with_max_width(80);
        assert_eq!(config.max_width, 80);
        assert_eq!(config.indent_size, 4); // Default
    }

    #[test]
    fn config_with_indent_size() {
        let config = FormatConfig::with_indent_size(2);
        assert_eq!(config.indent_size, 2);
        assert_eq!(config.max_width, 100); // Default
    }

    #[test]
    fn config_add_trailing_comma_always() {
        let config = FormatConfig {
            trailing_commas: TrailingCommas::Always,
            ..Default::default()
        };

        // Multi-line: add
        assert!(config.add_trailing_comma(true, false));
        assert!(config.add_trailing_comma(true, true));

        // Single-line: don't add
        assert!(!config.add_trailing_comma(false, false));
        assert!(!config.add_trailing_comma(false, true));
    }

    #[test]
    fn config_add_trailing_comma_never() {
        let config = FormatConfig {
            trailing_commas: TrailingCommas::Never,
            ..Default::default()
        };

        // Never add
        assert!(!config.add_trailing_comma(true, false));
        assert!(!config.add_trailing_comma(true, true));
        assert!(!config.add_trailing_comma(false, false));
        assert!(!config.add_trailing_comma(false, true));
    }

    #[test]
    fn config_add_trailing_comma_preserve() {
        let config = FormatConfig {
            trailing_commas: TrailingCommas::Preserve,
            ..Default::default()
        };

        // Preserve: only if had trailing AND multiline
        assert!(!config.add_trailing_comma(true, false));
        assert!(config.add_trailing_comma(true, true));
        assert!(!config.add_trailing_comma(false, false));
        assert!(!config.add_trailing_comma(false, true));
    }
}

mod trailing_commas_tests {
    use super::*;

    #[test]
    fn trailing_commas_default() {
        assert_eq!(TrailingCommas::default(), TrailingCommas::Always);
    }

    #[test]
    fn trailing_commas_is_always() {
        assert!(TrailingCommas::Always.is_always());
        assert!(!TrailingCommas::Never.is_always());
        assert!(!TrailingCommas::Preserve.is_always());
    }

    #[test]
    fn trailing_commas_is_never() {
        assert!(!TrailingCommas::Always.is_never());
        assert!(TrailingCommas::Never.is_never());
        assert!(!TrailingCommas::Preserve.is_never());
    }

    #[test]
    fn trailing_commas_is_preserve() {
        assert!(!TrailingCommas::Always.is_preserve());
        assert!(!TrailingCommas::Never.is_preserve());
        assert!(TrailingCommas::Preserve.is_preserve());
    }
}

mod integration_tests {
    use super::*;

    /// Test the full nested formatting scenario from spec lines 93-95.
    #[test]
    fn nested_constructs_break_independently() {
        let config = FormatConfig::default();

        // Outer expression starts with some content
        let outer = Shape::from_config(&config).consume(15); // "let result = "
        assert_eq!(outer.remaining(), 85);

        // Outer breaks (run block)
        let run_body = outer.for_block(&config);
        assert_eq!(run_body.indent, 4);
        assert_eq!(run_body.remaining(), 96);

        // Consume some prefix
        let after_prefix = run_body.after("process(");
        assert_eq!(after_prefix.remaining(), 88);

        // Inner expression (nested call) gets fresh width check
        let inner = after_prefix.for_nested(&config);
        assert_eq!(inner.remaining(), 96); // Full width minus indent

        // If inner content fits in 96 chars, it stays inline
        // even though outer had to break
        assert!(inner.fits(50));
    }

    /// Test complex nesting scenario.
    #[test]
    fn deeply_nested_shapes() {
        let config = FormatConfig::default();

        let level0 = Shape::from_config(&config);
        let level1 = level0.for_block(&config);
        let level2 = level1.for_block(&config);
        let level3 = level2.for_block(&config);

        assert_eq!(level0.indent, 0);
        assert_eq!(level1.indent, 4);
        assert_eq!(level2.indent, 8);
        assert_eq!(level3.indent, 12);

        assert_eq!(level0.remaining(), 100);
        assert_eq!(level1.remaining(), 96);
        assert_eq!(level2.remaining(), 92);
        assert_eq!(level3.remaining(), 88);
    }
}

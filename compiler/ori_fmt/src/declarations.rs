//! Declaration Formatting
//!
//! Formatting for top-level declarations: functions, types, traits, impls, imports, and constants.
//!
//! # Design
//!
//! Declaration formatting builds on the expression formatter by adding:
//! - Function signature formatting (params, generics, return type, capabilities)
//! - Type definition formatting (structs, sum types, newtypes)
//! - Module-level structure (imports, constants, functions, tests)
//! - Blank line handling between items
//! - Comment preservation and doc comment reordering

use crate::comments::{format_comment, CommentIndex};
use crate::context::{FormatConfig, FormatContext};
use crate::emitter::StringEmitter;
use crate::formatter::Formatter;
use crate::width::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::ast::items::{
    ConfigDef, Function, Module, Param, TestDef, TypeDecl, TypeDeclKind, UseDef, UseItem,
};
use ori_ir::ast::items::{ImplDef, TraitBound, TraitDef, TraitItem, WhereClause};
use ori_ir::{CommentList, ExprArena, ExprId, ParsedType, StringLookup, TypeId, Visibility};

/// Format a complete module to a string with default config.
pub fn format_module<I: StringLookup>(module: &Module, arena: &ExprArena, interner: &I) -> String {
    format_module_with_config(module, arena, interner, FormatConfig::default())
}

/// Format a complete module to a string with custom config.
pub fn format_module_with_config<I: StringLookup>(
    module: &Module,
    arena: &ExprArena,
    interner: &I,
    config: FormatConfig,
) -> String {
    let mut formatter = ModuleFormatter::with_config(arena, interner, config);
    formatter.format_module(module);
    formatter.ctx.finalize()
}

/// Format a complete module with comment preservation and default config.
///
/// This function preserves comments from the source, associating them with
/// the declarations they precede. Doc comments are reordered to canonical order.
pub fn format_module_with_comments<I: StringLookup>(
    module: &Module,
    comments: &CommentList,
    arena: &ExprArena,
    interner: &I,
) -> String {
    format_module_with_comments_and_config(
        module,
        comments,
        arena,
        interner,
        FormatConfig::default(),
    )
}

/// Format a complete module with comment preservation and custom config.
///
/// This function preserves comments from the source, associating them with
/// the declarations they precede. Doc comments are reordered to canonical order.
pub fn format_module_with_comments_and_config<I: StringLookup>(
    module: &Module,
    comments: &CommentList,
    arena: &ExprArena,
    interner: &I,
    config: FormatConfig,
) -> String {
    let mut formatter = ModuleFormatter::with_config(arena, interner, config);

    // Collect all item positions for comment association
    let positions = collect_module_positions(module);
    let mut comment_index = CommentIndex::new(comments, &positions);

    formatter.format_module_with_comments(module, comments, &mut comment_index);
    formatter.ctx.finalize()
}

/// Collect all start positions of items in a module.
fn collect_module_positions(module: &Module) -> Vec<u32> {
    let mut positions = Vec::new();

    for import in &module.imports {
        positions.push(import.span.start);
    }
    for config in &module.configs {
        positions.push(config.span.start);
    }
    for type_decl in &module.types {
        positions.push(type_decl.span.start);
    }
    for trait_def in &module.traits {
        positions.push(trait_def.span.start);
        // Also collect positions for items inside traits
        for item in &trait_def.items {
            positions.push(item.span().start);
        }
    }
    for impl_def in &module.impls {
        positions.push(impl_def.span.start);
        // Also collect positions for items inside impl blocks
        for assoc in &impl_def.assoc_types {
            positions.push(assoc.span.start);
        }
        for method in &impl_def.methods {
            positions.push(method.span.start);
        }
    }
    for func in &module.functions {
        positions.push(func.span.start);
    }
    for test in &module.tests {
        positions.push(test.span.start);
    }

    positions.sort_unstable();
    positions
}

/// Formatter for module-level declarations.
pub struct ModuleFormatter<'a, I: StringLookup> {
    arena: &'a ExprArena,
    interner: &'a I,
    ctx: FormatContext<StringEmitter>,
    width_calc: WidthCalculator<'a, I>,
    config: FormatConfig,
}

impl<'a, I: StringLookup> ModuleFormatter<'a, I> {
    /// Create a new module formatter with default config.
    pub fn new(arena: &'a ExprArena, interner: &'a I) -> Self {
        Self::with_config(arena, interner, FormatConfig::default())
    }

    /// Create a new module formatter with custom config.
    pub fn with_config(arena: &'a ExprArena, interner: &'a I, config: FormatConfig) -> Self {
        Self {
            arena,
            interner,
            ctx: FormatContext::with_config(config),
            width_calc: WidthCalculator::new(arena, interner),
            config,
        }
    }

    /// Finish formatting and return the result string.
    pub fn finish(self) -> String {
        self.ctx.finalize()
    }

    /// Format a complete module.
    pub fn format_module(&mut self, module: &Module) {
        let mut first_item = true;

        // Imports first
        if !module.imports.is_empty() {
            self.format_imports(&module.imports);
            first_item = false;
        }

        // Constants
        if !module.configs.is_empty() {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_configs(&module.configs);
            first_item = false;
        }

        // Type definitions
        for type_decl in &module.types {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_type_decl(type_decl);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Traits
        for trait_def in &module.traits {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_trait(trait_def);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Impls
        for impl_def in &module.impls {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_impl(impl_def);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Functions
        for func in &module.functions {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_function(func);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Tests
        for test in &module.tests {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_test(test);
            self.ctx.emit_newline();
            first_item = false;
        }
    }

    /// Format a complete module with comment preservation.
    pub fn format_module_with_comments(
        &mut self,
        module: &Module,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        let mut first_item = true;

        // Imports first
        if !module.imports.is_empty() {
            self.format_imports_with_comments(&module.imports, comments, comment_index);
            first_item = false;
        }

        // Constants
        if !module.configs.is_empty() {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_configs_with_comments(&module.configs, comments, comment_index);
            first_item = false;
        }

        // Type definitions
        for type_decl in &module.types {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before_type(type_decl, comments, comment_index);
            self.format_type_decl(type_decl);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Traits
        for trait_def in &module.traits {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before(trait_def.span.start, comments, comment_index);
            self.format_trait_with_comments(trait_def, comments, comment_index);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Impls
        for impl_def in &module.impls {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before(impl_def.span.start, comments, comment_index);
            self.format_impl_with_comments(impl_def, comments, comment_index);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Functions
        for func in &module.functions {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before_function(func, comments, comment_index);
            self.format_function(func);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Tests
        for test in &module.tests {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before(test.span.start, comments, comment_index);
            self.format_test(test);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Emit any trailing comments
        self.emit_trailing_comments(comments, comment_index);
    }

    /// Emit comments that should appear before a given position.
    pub fn emit_comments_before(
        &mut self,
        pos: u32,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        let indices = comment_index.take_comments_before(pos);
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }

    /// Emit comments that should appear before a function, with @param reordering.
    pub fn emit_comments_before_function(
        &mut self,
        func: &Function,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        // Get param names from the function
        let params_list = self.arena.get_params(func.params);
        let param_names: Vec<&str> = params_list
            .iter()
            .map(|p| self.interner.lookup(p.name))
            .collect();

        let indices = comment_index.take_comments_before_function(
            func.span.start,
            &param_names,
            comments,
            self.interner,
        );
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }

    /// Emit comments that should appear before a type, with @field reordering.
    pub fn emit_comments_before_type(
        &mut self,
        type_decl: &TypeDecl,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        // Get field names from struct type, if applicable
        let field_names: Vec<&str> = match &type_decl.kind {
            TypeDeclKind::Struct(fields) => fields
                .iter()
                .map(|f| self.interner.lookup(f.name))
                .collect(),
            _ => Vec::new(),
        };

        let indices = comment_index.take_comments_before_type(
            type_decl.span.start,
            &field_names,
            comments,
            self.interner,
        );
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }

    /// Emit any remaining comments at the end of the file.
    fn emit_trailing_comments(&mut self, comments: &CommentList, comment_index: &mut CommentIndex) {
        let indices = comment_index.remaining_indices();
        if !indices.is_empty() {
            // Add a blank line before trailing comments
            self.ctx.emit_newline();
            for idx in indices {
                let comment = &comments[idx];
                self.ctx.emit(&format_comment(comment, self.interner));
                self.ctx.emit_newline();
            }
        }
    }

    /// Format import declarations with comments.
    fn format_imports_with_comments(
        &mut self,
        imports: &[UseDef],
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        // Group imports: stdlib first, then relative
        let (stdlib, relative): (Vec<_>, Vec<_>) = imports
            .iter()
            .partition(|u| matches!(u.path, ori_ir::ast::items::ImportPath::Module(_)));

        // Format stdlib imports
        for import in &stdlib {
            self.emit_comments_before(import.span.start, comments, comment_index);
            self.format_use(import);
            self.ctx.emit_newline();
        }

        // Blank line between stdlib and relative if both exist
        if !stdlib.is_empty() && !relative.is_empty() {
            self.ctx.emit_newline();
        }

        // Format relative imports
        for import in &relative {
            self.emit_comments_before(import.span.start, comments, comment_index);
            self.format_use(import);
            self.ctx.emit_newline();
        }
    }

    /// Format constant definitions with comments.
    fn format_configs_with_comments(
        &mut self,
        configs: &[ConfigDef],
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        for config in configs {
            self.emit_comments_before(config.span.start, comments, comment_index);
            self.format_config(config);
            self.ctx.emit_newline();
        }
    }

    /// Format import declarations, grouping stdlib imports before relative imports.
    fn format_imports(&mut self, imports: &[UseDef]) {
        // Group imports: stdlib first, then relative
        let (stdlib, relative): (Vec<_>, Vec<_>) = imports
            .iter()
            .partition(|u| matches!(u.path, ori_ir::ast::items::ImportPath::Module(_)));

        // Format stdlib imports
        for import in &stdlib {
            self.format_use(import);
            self.ctx.emit_newline();
        }

        // Blank line between stdlib and relative if both exist
        if !stdlib.is_empty() && !relative.is_empty() {
            self.ctx.emit_newline();
        }

        // Format relative imports
        for import in &relative {
            self.format_use(import);
            self.ctx.emit_newline();
        }
    }

    fn format_use(&mut self, use_def: &UseDef) {
        // Visibility
        if use_def.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("use ");

        // Path
        match &use_def.path {
            ori_ir::ast::items::ImportPath::Relative(name) => {
                self.ctx.emit("\"");
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit("\"");
            }
            ori_ir::ast::items::ImportPath::Module(segments) => {
                for (i, seg) in segments.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(".");
                    }
                    self.ctx.emit(self.interner.lookup(*seg));
                }
            }
        }

        // Module alias or items
        if let Some(alias) = use_def.module_alias {
            self.ctx.emit(" as ");
            self.ctx.emit(self.interner.lookup(alias));
        } else if !use_def.items.is_empty() {
            self.ctx.emit(" { ");
            self.format_use_items(&use_def.items);
            self.ctx.emit(" }");
        }
    }

    fn format_use_items(&mut self, items: &[UseItem]) {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            if item.is_private {
                self.ctx.emit("::");
            }
            self.ctx.emit(self.interner.lookup(item.name));
            if let Some(alias) = item.alias {
                self.ctx.emit(" as ");
                self.ctx.emit(self.interner.lookup(alias));
            }
        }
    }

    /// Format constant/config definitions.
    fn format_configs(&mut self, configs: &[ConfigDef]) {
        for config in configs {
            self.format_config(config);
            self.ctx.emit_newline();
        }
    }

    fn format_config(&mut self, config: &ConfigDef) {
        if config.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }
        self.ctx.emit("$");
        self.ctx.emit(self.interner.lookup(config.name));
        self.ctx.emit(" = ");

        // Format the value expression
        // Pass current column so width decisions account for full line context
        let current_column = self.ctx.column();
        let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
            .with_starting_column(current_column);
        expr_formatter.format(config.value);
        // Get the output without trailing newline
        let expr_output = expr_formatter.ctx.as_str().trim_end();
        self.ctx.emit(expr_output);
    }

    /// Format a function declaration including signature and body.
    pub fn format_function(&mut self, func: &Function) {
        // Visibility
        if func.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        // Function name
        self.ctx.emit("@");
        self.ctx.emit(self.interner.lookup(func.name));

        // Generic parameters
        self.format_generic_params(func.generics);

        // Calculate trailing width (return type + capabilities + where + " = ")
        // so params can decide whether to break based on full signature
        let trailing_width = self.calculate_function_trailing_width(func);

        // Parameters
        self.ctx.emit(" ");
        self.format_params_with_trailing(func.params, trailing_width);

        // Return type
        if let Some(ref ret_ty) = func.return_ty {
            self.ctx.emit(" -> ");
            self.format_parsed_type(ret_ty);
        }

        // Capabilities
        if !func.capabilities.is_empty() {
            self.ctx.emit(" uses ");
            for (i, cap) in func.capabilities.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.ctx.emit(self.interner.lookup(cap.name));
            }
        }

        // Where clauses
        self.format_where_clauses(&func.where_clauses);

        // Body
        self.format_function_body(func.body);
    }

    /// Format a function body, breaking to new line if it doesn't fit after `= `.
    fn format_function_body(&mut self, body: ExprId) {
        // Calculate body width to determine if it fits inline
        let body_width = self.width_calc.width(body);

        // Check if body fits after " = " on current line
        let space_after_eq = 3; // " = "
        let fits_inline =
            body_width != ALWAYS_STACKED && self.ctx.fits(space_after_eq + body_width);

        if fits_inline {
            // Inline: " = body"
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_starting_column(current_column);
            expr_formatter.format(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
        } else if self.is_conditional(body) {
            // Conditionals break to new line: " =\n    if cond then ... else ..."
            self.ctx.emit(" =");
            self.ctx.emit_newline();
            self.ctx.indent();
            self.ctx.emit_indent();

            // Create formatter with indent level 1 for proper nested breaks
            // Use format_broken to prevent re-evaluation of fit at new position
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_indent_level(1)
                .with_starting_column(self.ctx.column());
            expr_formatter.format_broken(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
            self.ctx.dedent();
        } else {
            // Other constructs stay on same line, break internally: " = [...\n]"
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_starting_column(current_column);
            expr_formatter.format(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
        }
    }

    /// Check if an expression is a conditional (if-then-else).
    fn is_conditional(&self, body: ExprId) -> bool {
        matches!(self.arena.get_expr(body).kind, ori_ir::ExprKind::If { .. })
    }

    /// Format params without considering trailing content (for method params, etc.).
    fn format_params(&mut self, params: ori_ir::ParamRange) {
        self.format_params_with_trailing(params, 0);
    }

    /// Format params considering trailing content width (return type, capabilities, etc.).
    /// This ensures we break params if the full signature would exceed line width.
    fn format_params_with_trailing(&mut self, params: ori_ir::ParamRange, trailing_width: usize) {
        let params_list = self.arena.get_params(params);

        if params_list.is_empty() {
            self.ctx.emit("()");
            return;
        }

        // Calculate if params + trailing content fit on one line
        let inline_width = self.calculate_params_width(params_list);
        let total_width = inline_width + trailing_width;
        let fits_inline = self.ctx.fits(total_width);

        if fits_inline {
            self.ctx.emit("(");
            for (i, param) in params_list.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.format_param(param);
            }
            self.ctx.emit(")");
        } else {
            self.ctx.emit("(");
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, param) in params_list.iter().enumerate() {
                self.ctx.emit_indent();
                self.format_param(param);
                self.ctx.emit(",");
                if i < params_list.len() - 1 {
                    self.ctx.emit_newline();
                }
            }
            self.ctx.dedent();
            self.ctx.emit_newline_indent();
            self.ctx.emit(")");
        }
    }

    fn format_param(&mut self, param: &Param) {
        self.ctx.emit(self.interner.lookup(param.name));
        if let Some(ref ty) = param.ty {
            self.ctx.emit(": ");
            self.format_parsed_type(ty);
        }
    }

    fn calculate_params_width(&self, params: &[Param]) -> usize {
        let mut width = 2; // ()
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                width += 2; // ", "
            }
            width += self.interner.lookup(param.name).len();
            if let Some(ref ty) = param.ty {
                width += 2; // ": "
                width += self.calculate_type_width(ty);
            }
        }
        width
    }

    /// Calculate width of function trailing content (return type + caps + where + " = " + body).
    /// This is used to help params decide whether to break based on full signature width.
    ///
    /// Only includes body width if the body is short enough that breaking it would look ugly.
    /// Long bodies will break naturally at good points (else, operators, etc.), so we let them.
    fn calculate_function_trailing_width(&mut self, func: &Function) -> usize {
        const SHORT_BODY_THRESHOLD: usize = 20;
        let mut width = 0;

        // Return type: " -> Type"
        if let Some(ref ret_ty) = func.return_ty {
            width += 4; // " -> "
            width += self.calculate_type_width(ret_ty);
        }

        // Capabilities: " uses Cap1, Cap2"
        if !func.capabilities.is_empty() {
            width += 6; // " uses "
            for (i, cap) in func.capabilities.iter().enumerate() {
                if i > 0 {
                    width += 2; // ", "
                }
                width += self.interner.lookup(cap.name).len();
            }
        }

        // Where clauses: " where T: Trait"
        // For simplicity, estimate 20 chars if where clauses exist
        // (full calculation would be complex and rarely needed)
        if !func.where_clauses.is_empty() {
            width += 20;
        }

        // " = " prefix for body
        width += 3;

        // Only include body width if it's short enough that breaking it would be ugly.
        // Short expressions like `x + y` look bad when broken (`x\n+ y`), so we prefer
        // to break params first. Longer expressions will break at natural points
        // (conditionals at else, chains at method calls, etc.) which is fine.
        let body_width = self.width_calc.width(func.body);
        if body_width != ALWAYS_STACKED && body_width <= SHORT_BODY_THRESHOLD {
            width += body_width;
        }

        width
    }

    fn format_generic_params(&mut self, generics: ori_ir::GenericParamRange) {
        let generics_list = self.arena.get_generic_params(generics);
        if generics_list.is_empty() {
            return;
        }

        self.ctx.emit("<");
        for (i, param) in generics_list.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            self.ctx.emit(self.interner.lookup(param.name));
            if !param.bounds.is_empty() {
                self.ctx.emit(": ");
                self.format_trait_bounds(&param.bounds);
            }
        }
        self.ctx.emit(">");
    }

    fn format_trait_bounds(&mut self, bounds: &[TraitBound]) {
        for (i, bound) in bounds.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(" + ");
            }
            self.format_trait_bound(bound);
        }
    }

    fn format_trait_bound(&mut self, bound: &TraitBound) {
        self.ctx.emit(self.interner.lookup(bound.first));
        for seg in &bound.rest {
            self.ctx.emit(".");
            self.ctx.emit(self.interner.lookup(*seg));
        }
    }

    fn format_where_clauses(&mut self, where_clauses: &[WhereClause]) {
        if where_clauses.is_empty() {
            return;
        }

        self.ctx.emit(" where ");
        for (i, clause) in where_clauses.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            self.ctx.emit(self.interner.lookup(clause.param));
            if let Some(proj) = clause.projection {
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(proj));
            }
            self.ctx.emit(": ");
            self.format_trait_bounds(&clause.bounds);
        }
    }

    /// Format a test definition including attributes and body.
    pub fn format_test(&mut self, test: &TestDef) {
        // Skip attribute
        if let Some(reason) = test.skip_reason {
            self.ctx.emit("#skip(\"");
            self.ctx.emit(self.interner.lookup(reason));
            self.ctx.emit("\")");
            self.ctx.emit_newline();
        }

        // Compile fail attribute
        if !test.expected_errors.is_empty() {
            self.ctx.emit("#compile_fail");
            // Only emit details if there's a message
            if let Some(first_err) = test.expected_errors.first() {
                if let Some(msg) = first_err.message {
                    self.ctx.emit("(\"");
                    self.ctx.emit(self.interner.lookup(msg));
                    self.ctx.emit("\")");
                }
            }
            self.ctx.emit_newline();
        }

        // Fail attribute
        if let Some(expected) = test.fail_expected {
            self.ctx.emit("#fail(\"");
            self.ctx.emit(self.interner.lookup(expected));
            self.ctx.emit("\")");
            self.ctx.emit_newline();
        }

        // Test name
        self.ctx.emit("@");
        self.ctx.emit(self.interner.lookup(test.name));

        // Targets (only if there are any - free-floating tests have no targets clause)
        if !test.targets.is_empty() {
            for target in &test.targets {
                self.ctx.emit(" tests @");
                self.ctx.emit(self.interner.lookup(*target));
            }
        }

        // Parameters
        self.ctx.emit(" ");
        self.format_params(test.params);

        // Return type
        if let Some(ref ret_ty) = test.return_ty {
            self.ctx.emit(" -> ");
            self.format_parsed_type(ret_ty);
        }

        // Body - use similar logic to format_function_body
        self.format_test_body(test.body);
    }

    /// Format a test body, breaking to new line if it doesn't fit after `= `.
    fn format_test_body(&mut self, body: ExprId) {
        // Calculate body width to determine if it fits inline
        let body_width = self.width_calc.width(body);

        // Check if body fits after " = " on current line
        let space_after_eq = 3; // " = "
        let fits_inline =
            body_width != ALWAYS_STACKED && self.ctx.fits(space_after_eq + body_width);

        if fits_inline {
            // Inline: " = body"
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_starting_column(current_column);
            expr_formatter.format(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
        } else {
            // Body doesn't fit - always-stacked constructs (run/try/match) stay on same line
            // and break internally. Other constructs also stay on same line.
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_starting_column(current_column);
            expr_formatter.format(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
        }
    }

    /// Format a type declaration (struct, sum type, or newtype).
    pub fn format_type_decl(&mut self, type_decl: &TypeDecl) {
        // Derives
        if !type_decl.derives.is_empty() {
            self.ctx.emit("#derive(");
            for (i, derive) in type_decl.derives.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.ctx.emit(self.interner.lookup(*derive));
            }
            self.ctx.emit(")");
            self.ctx.emit_newline();
        }

        // Visibility
        if type_decl.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("type ");
        self.ctx.emit(self.interner.lookup(type_decl.name));

        // Generic parameters
        self.format_generic_params(type_decl.generics);

        // Where clauses
        self.format_where_clauses(&type_decl.where_clauses);

        self.ctx.emit(" = ");

        // Type body
        match &type_decl.kind {
            TypeDeclKind::Struct(fields) => {
                self.format_struct_fields(fields);
            }
            TypeDeclKind::Sum(variants) => {
                self.format_sum_variants(variants);
            }
            TypeDeclKind::Newtype(ty) => {
                self.format_parsed_type(ty);
            }
        }
    }

    fn format_struct_fields(&mut self, fields: &[ori_ir::ast::items::StructField]) {
        if fields.is_empty() {
            self.ctx.emit("{}");
            return;
        }

        // Calculate inline width
        let inline_width = self.calculate_struct_fields_width(fields);
        let fits_inline = self.ctx.fits(inline_width);

        if fits_inline {
            self.ctx.emit("{ ");
            for (i, field) in fields.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.ctx.emit(self.interner.lookup(field.name));
                self.ctx.emit(": ");
                self.format_parsed_type(&field.ty);
            }
            self.ctx.emit(" }");
        } else {
            self.ctx.emit("{");
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, field) in fields.iter().enumerate() {
                self.ctx.emit_indent();
                self.ctx.emit(self.interner.lookup(field.name));
                self.ctx.emit(": ");
                self.format_parsed_type(&field.ty);
                self.ctx.emit(",");
                if i < fields.len() - 1 {
                    self.ctx.emit_newline();
                }
            }
            self.ctx.dedent();
            self.ctx.emit_newline_indent();
            self.ctx.emit("}");
        }
    }

    fn calculate_struct_fields_width(&self, fields: &[ori_ir::ast::items::StructField]) -> usize {
        let mut width = 4; // "{ " + " }"
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                width += 2; // ", "
            }
            width += self.interner.lookup(field.name).len();
            width += 2; // ": "
            width += self.calculate_type_width(&field.ty);
        }
        width
    }

    fn format_sum_variants(&mut self, variants: &[ori_ir::ast::items::Variant]) {
        if variants.is_empty() {
            return;
        }

        // Calculate inline width
        let inline_width = self.calculate_sum_variants_width(variants);
        let fits_inline = self.ctx.fits(inline_width);

        if fits_inline && variants.len() <= 3 {
            for (i, variant) in variants.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(" | ");
                }
                self.format_variant(variant);
            }
        } else {
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, variant) in variants.iter().enumerate() {
                self.ctx.emit_indent();
                self.ctx.emit("| ");
                self.format_variant(variant);
                if i < variants.len() - 1 {
                    self.ctx.emit_newline();
                }
            }
            self.ctx.dedent();
        }
    }

    fn format_variant(&mut self, variant: &ori_ir::ast::items::Variant) {
        self.ctx.emit(self.interner.lookup(variant.name));
        if !variant.fields.is_empty() {
            self.ctx.emit("(");
            for (i, field) in variant.fields.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.ctx.emit(self.interner.lookup(field.name));
                self.ctx.emit(": ");
                self.format_parsed_type(&field.ty);
            }
            self.ctx.emit(")");
        }
    }

    fn calculate_sum_variants_width(&self, variants: &[ori_ir::ast::items::Variant]) -> usize {
        let mut width = 0;
        for (i, variant) in variants.iter().enumerate() {
            if i > 0 {
                width += 3; // " | "
            }
            width += self.interner.lookup(variant.name).len();
            if !variant.fields.is_empty() {
                width += 2; // "()"
                for (j, field) in variant.fields.iter().enumerate() {
                    if j > 0 {
                        width += 2; // ", "
                    }
                    width += self.interner.lookup(field.name).len();
                    width += 2; // ": "
                    width += self.calculate_type_width(&field.ty);
                }
            }
        }
        width
    }

    /// Format a trait definition including super traits and items.
    pub fn format_trait(&mut self, trait_def: &TraitDef) {
        if trait_def.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("trait ");
        self.ctx.emit(self.interner.lookup(trait_def.name));

        // Generic parameters
        self.format_generic_params(trait_def.generics);

        // Super traits
        if !trait_def.super_traits.is_empty() {
            self.ctx.emit(": ");
            self.format_trait_bounds(&trait_def.super_traits);
        }

        // Body
        if trait_def.items.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, item) in trait_def.items.iter().enumerate() {
                if i > 0 && trait_def.items.len() > 1 {
                    self.ctx.emit_newline();
                }
                self.ctx.emit_indent();
                self.format_trait_item(item);
                self.ctx.emit_newline();
            }
            self.ctx.dedent();
            self.ctx.emit_indent();
            self.ctx.emit("}");
        }
    }

    fn format_trait_item(&mut self, item: &TraitItem) {
        match item {
            TraitItem::MethodSig(sig) => {
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(sig.name));
                self.ctx.emit(" ");
                self.format_params(sig.params);
                self.ctx.emit(" -> ");
                self.format_parsed_type(&sig.return_ty);
            }
            TraitItem::DefaultMethod(method) => {
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(method.name));
                self.ctx.emit(" ");
                self.format_params(method.params);
                self.ctx.emit(" -> ");
                self.format_parsed_type(&method.return_ty);
                self.ctx.emit(" = ");

                // Pass current column and indent level so width decisions and
                // line breaks account for full context
                let current_column = self.ctx.column();
                let current_indent = self.ctx.indent_level();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, self.config)
                        .with_indent_level(current_indent)
                        .with_starting_column(current_column);
                expr_formatter.format(method.body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                self.ctx.emit(body_output);
            }
            TraitItem::AssocType(assoc) => {
                self.ctx.emit("type ");
                self.ctx.emit(self.interner.lookup(assoc.name));
            }
        }
    }

    /// Format a trait definition with comment preservation.
    pub fn format_trait_with_comments(
        &mut self,
        trait_def: &TraitDef,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        if trait_def.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("trait ");
        self.ctx.emit(self.interner.lookup(trait_def.name));

        // Generic parameters
        self.format_generic_params(trait_def.generics);

        // Super traits
        if !trait_def.super_traits.is_empty() {
            self.ctx.emit(": ");
            self.format_trait_bounds(&trait_def.super_traits);
        }

        // Body
        if trait_def.items.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, item) in trait_def.items.iter().enumerate() {
                if i > 0 && trait_def.items.len() > 1 {
                    self.ctx.emit_newline();
                }
                // Emit comments before this trait item
                self.emit_comments_before_indented(item.span().start, comments, comment_index);
                self.ctx.emit_indent();
                self.format_trait_item(item);
                self.ctx.emit_newline();
            }
            self.ctx.dedent();
            self.ctx.emit_indent();
            self.ctx.emit("}");
        }
    }

    /// Format an impl block (trait impl or inherent impl).
    pub fn format_impl(&mut self, impl_def: &ImplDef) {
        self.ctx.emit("impl");

        // Generic parameters
        self.format_generic_params(impl_def.generics);

        self.ctx.emit(" ");

        // Trait path (if trait impl)
        if let Some(ref trait_path) = impl_def.trait_path {
            for (i, seg) in trait_path.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(".");
                }
                self.ctx.emit(self.interner.lookup(*seg));
            }
            self.ctx.emit(" for ");
        }

        // Self type
        self.format_parsed_type(&impl_def.self_ty);

        // Where clauses
        self.format_where_clauses(&impl_def.where_clauses);

        // Body
        if impl_def.methods.is_empty() && impl_def.assoc_types.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();

            // Associated types
            for assoc in &impl_def.assoc_types {
                self.ctx.emit_indent();
                self.ctx.emit("type ");
                self.ctx.emit(self.interner.lookup(assoc.name));
                self.ctx.emit(" = ");
                self.format_parsed_type(&assoc.ty);
                self.ctx.emit_newline();
                self.ctx.emit_newline();
            }

            // Methods
            for (i, method) in impl_def.methods.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit_newline();
                }
                self.ctx.emit_indent();
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(method.name));
                self.ctx.emit(" ");
                self.format_params(method.params);
                self.ctx.emit(" -> ");
                self.format_parsed_type(&method.return_ty);
                self.ctx.emit(" = ");

                // Pass current column and indent level so width decisions and
                // line breaks account for full context
                let current_column = self.ctx.column();
                let current_indent = self.ctx.indent_level();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, self.config)
                        .with_indent_level(current_indent)
                        .with_starting_column(current_column);
                expr_formatter.format(method.body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                self.ctx.emit(body_output);
                self.ctx.emit_newline();
            }

            self.ctx.dedent();
            self.ctx.emit_indent();
            self.ctx.emit("}");
        }
    }

    /// Format an impl block with comment preservation.
    pub fn format_impl_with_comments(
        &mut self,
        impl_def: &ImplDef,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        self.ctx.emit("impl");

        // Generic parameters
        self.format_generic_params(impl_def.generics);

        self.ctx.emit(" ");

        // Trait path (if trait impl)
        if let Some(ref trait_path) = impl_def.trait_path {
            for (i, seg) in trait_path.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(".");
                }
                self.ctx.emit(self.interner.lookup(*seg));
            }
            self.ctx.emit(" for ");
        }

        // Self type
        self.format_parsed_type(&impl_def.self_ty);

        // Where clauses
        self.format_where_clauses(&impl_def.where_clauses);

        // Body
        if impl_def.methods.is_empty() && impl_def.assoc_types.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();

            // Associated types
            for assoc in &impl_def.assoc_types {
                // Emit comments before this associated type
                self.emit_comments_before_indented(assoc.span.start, comments, comment_index);
                self.ctx.emit_indent();
                self.ctx.emit("type ");
                self.ctx.emit(self.interner.lookup(assoc.name));
                self.ctx.emit(" = ");
                self.format_parsed_type(&assoc.ty);
                self.ctx.emit_newline();
                self.ctx.emit_newline();
            }

            // Methods
            for (i, method) in impl_def.methods.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit_newline();
                }
                // Emit comments before this method
                self.emit_comments_before_indented(method.span.start, comments, comment_index);
                self.ctx.emit_indent();
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(method.name));
                self.ctx.emit(" ");
                self.format_params(method.params);
                self.ctx.emit(" -> ");
                self.format_parsed_type(&method.return_ty);
                self.ctx.emit(" = ");

                // Pass current column and indent level so width decisions and
                // line breaks account for full context
                let current_column = self.ctx.column();
                let current_indent = self.ctx.indent_level();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, self.config)
                        .with_indent_level(current_indent)
                        .with_starting_column(current_column);
                expr_formatter.format(method.body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                self.ctx.emit(body_output);
                self.ctx.emit_newline();
            }

            self.ctx.dedent();
            self.ctx.emit_indent();
            self.ctx.emit("}");
        }
    }

    /// Emit comments that should appear before a given position, with indentation.
    fn emit_comments_before_indented(
        &mut self,
        pos: u32,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        let indices = comment_index.take_comments_before(pos);
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit_indent();
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }

    /// Format a parsed type expression.
    fn format_parsed_type(&mut self, ty: &ParsedType) {
        match ty {
            ParsedType::Primitive(type_id) => {
                self.ctx.emit(type_id_to_str(*type_id));
            }
            ParsedType::Named { name, type_args } => {
                self.ctx.emit(self.interner.lookup(*name));
                let args = self.arena.get_parsed_type_list(*type_args);
                if !args.is_empty() {
                    self.ctx.emit("<");
                    for (i, arg_id) in args.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        let arg = self.arena.get_parsed_type(*arg_id);
                        self.format_parsed_type(arg);
                    }
                    self.ctx.emit(">");
                }
            }
            ParsedType::List(elem) => {
                self.ctx.emit("[");
                let elem_ty = self.arena.get_parsed_type(*elem);
                self.format_parsed_type(elem_ty);
                self.ctx.emit("]");
            }
            ParsedType::Tuple(elems) => {
                self.ctx.emit("(");
                let elem_list = self.arena.get_parsed_type_list(*elems);
                for (i, elem_id) in elem_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let elem = self.arena.get_parsed_type(*elem_id);
                    self.format_parsed_type(elem);
                }
                self.ctx.emit(")");
            }
            ParsedType::Function { params, ret } => {
                self.ctx.emit("(");
                let param_list = self.arena.get_parsed_type_list(*params);
                for (i, param_id) in param_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let param = self.arena.get_parsed_type(*param_id);
                    self.format_parsed_type(param);
                }
                self.ctx.emit(") -> ");
                let ret_ty = self.arena.get_parsed_type(*ret);
                self.format_parsed_type(ret_ty);
            }
            ParsedType::Map { key, value } => {
                self.ctx.emit("{");
                let key_ty = self.arena.get_parsed_type(*key);
                self.format_parsed_type(key_ty);
                self.ctx.emit(": ");
                let value_ty = self.arena.get_parsed_type(*value);
                self.format_parsed_type(value_ty);
                self.ctx.emit("}");
            }
            ParsedType::Infer => {
                self.ctx.emit("_");
            }
            ParsedType::SelfType => {
                self.ctx.emit("Self");
            }
            ParsedType::AssociatedType { base, assoc_name } => {
                let base_ty = self.arena.get_parsed_type(*base);
                self.format_parsed_type(base_ty);
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(*assoc_name));
            }
        }
    }

    fn calculate_type_width(&self, ty: &ParsedType) -> usize {
        match ty {
            ParsedType::Primitive(type_id) => type_id_to_str(*type_id).len(),
            ParsedType::Named { name, type_args } => {
                let mut width = self.interner.lookup(*name).len();
                let args = self.arena.get_parsed_type_list(*type_args);
                if !args.is_empty() {
                    width += 2; // "<>"
                    for (i, arg_id) in args.iter().enumerate() {
                        if i > 0 {
                            width += 2; // ", "
                        }
                        let arg = self.arena.get_parsed_type(*arg_id);
                        width += self.calculate_type_width(arg);
                    }
                }
                width
            }
            ParsedType::List(elem) => {
                let elem_ty = self.arena.get_parsed_type(*elem);
                2 + self.calculate_type_width(elem_ty) // "[]"
            }
            ParsedType::Tuple(elems) => {
                let elem_list = self.arena.get_parsed_type_list(*elems);
                let mut width = 2; // "()"
                for (i, elem_id) in elem_list.iter().enumerate() {
                    if i > 0 {
                        width += 2; // ", "
                    }
                    let elem = self.arena.get_parsed_type(*elem_id);
                    width += self.calculate_type_width(elem);
                }
                width
            }
            ParsedType::Function { params, ret } => {
                let param_list = self.arena.get_parsed_type_list(*params);
                let mut width = 2; // "()"
                for (i, param_id) in param_list.iter().enumerate() {
                    if i > 0 {
                        width += 2; // ", "
                    }
                    let param = self.arena.get_parsed_type(*param_id);
                    width += self.calculate_type_width(param);
                }
                width += 4; // " -> "
                let ret_ty = self.arena.get_parsed_type(*ret);
                width += self.calculate_type_width(ret_ty);
                width
            }
            ParsedType::Map { key, value } => {
                let key_ty = self.arena.get_parsed_type(*key);
                let value_ty = self.arena.get_parsed_type(*value);
                2 + self.calculate_type_width(key_ty) + 2 + self.calculate_type_width(value_ty)
                // "{" + key + ": " + value + "}"
            }
            ParsedType::Infer => 1,    // "_"
            ParsedType::SelfType => 4, // "Self"
            ParsedType::AssociatedType { base, assoc_name } => {
                let base_ty = self.arena.get_parsed_type(*base);
                self.calculate_type_width(base_ty) + 1 + self.interner.lookup(*assoc_name).len()
            }
        }
    }
}

/// Convert a [`TypeId`] to its string representation.
fn type_id_to_str(id: TypeId) -> &'static str {
    match id {
        TypeId::INT => "int",
        TypeId::FLOAT => "float",
        TypeId::BOOL => "bool",
        TypeId::STR => "str",
        TypeId::CHAR => "char",
        TypeId::BYTE => "byte",
        TypeId::VOID => "void",
        TypeId::NEVER => "Never",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_id_to_str() {
        assert_eq!(type_id_to_str(TypeId::INT), "int");
        assert_eq!(type_id_to_str(TypeId::FLOAT), "float");
        assert_eq!(type_id_to_str(TypeId::BOOL), "bool");
        assert_eq!(type_id_to_str(TypeId::STR), "str");
        assert_eq!(type_id_to_str(TypeId::CHAR), "char");
        assert_eq!(type_id_to_str(TypeId::BYTE), "byte");
        assert_eq!(type_id_to_str(TypeId::VOID), "void");
        assert_eq!(type_id_to_str(TypeId::NEVER), "Never");
    }
}

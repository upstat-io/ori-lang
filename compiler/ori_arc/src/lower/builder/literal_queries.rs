//! Literal-value queries over the in-progress ARC instruction stream.

use crate::ir::{ArcInstr, ArcValue, ArcVarId, LitValue};

use super::ArcIrBuilder;

impl ArcIrBuilder {
    /// Look up whether `var` resolves to a literal integer constant.
    ///
    /// Traces through SSA definitions to find the ultimate literal value:
    /// - Direct: `Let { dst: var, value: Literal(Int(n)) }` → `Some(n)`
    /// - Through projection: `Project { dst: var, value: src, field: f }`
    ///   → `Construct { dst: src, args }` → `get_literal_int(args[f])`
    ///
    /// Used by range specialization to detect compile-time-constant step
    /// and inclusive flags, enabling single-instruction bounds checks at -O0.
    pub(crate) fn get_literal_int(&self, var: ArcVarId) -> Option<i64> {
        match self.definition(var)? {
            ArcInstr::Let {
                value: ArcValue::Literal(LitValue::Int(n)),
                ..
            } => Some(*n),
            ArcInstr::Project {
                value: source,
                field,
                ..
            } => self.get_construct_arg(*source, *field),
            _ => None,
        }
    }

    /// Trace a `Construct` instruction to get the literal int of one of its args.
    fn get_construct_arg(&self, construct_var: ArcVarId, field: u32) -> Option<i64> {
        let ArcInstr::Construct { args, .. } = self.definition(construct_var)? else {
            return None;
        };
        let arg = *args.get(field as usize)?;
        self.get_literal_int(arg)
    }

    /// Query whether a field of a constructed aggregate is a literal int,
    /// without emitting a `Project` instruction.
    ///
    /// Traces `base_var → Construct { args }` → `args[field]` → literal check.
    /// Used to detect compile-time constants (e.g., range step/inclusive flags)
    /// before deciding whether to extract the field.
    pub(crate) fn get_field_literal_int(&self, base_var: ArcVarId, field: u32) -> Option<i64> {
        self.get_construct_arg(base_var, field)
    }

    fn definition(&self, var: ArcVarId) -> Option<&ArcInstr> {
        let location = self.definitions.get(var.index())?;
        if !location.block.is_valid() {
            return None;
        }
        let instruction = usize::try_from(location.instruction).ok()?;
        self.blocks
            .get(location.block.index())?
            .body
            .get(instruction)
    }
}

//! Checked identities used by bytecode tables.

use super::TableKind;
use crate::CompileError;

macro_rules! table_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub(crate) struct $name(u32);

        impl $name {
            pub(crate) fn new(index: usize, table: TableKind) -> Result<Self, CompileError> {
                u32::try_from(index)
                    .map(Self)
                    .map_err(|_| CompileError::TableOverflow {
                        table,
                        count: index,
                    })
            }

            pub(crate) const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

table_id!(OperandListId);
table_id!(MoveListId);
table_id!(SwitchTableId);
table_id!(StringId);

/// Register index within one bytecode frame.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Register(u32);

impl Register {
    pub(crate) fn from_arc(value: ori_arc::ArcVarId) -> Self {
        Self(value.raw())
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) const fn raw(self) -> u32 {
        self.0
    }
}

/// Program counter within one bytecode function.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Pc(u32);

impl Pc {
    pub(crate) fn new(index: usize, function: ori_ir::Name) -> Result<Self, CompileError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| CompileError::FunctionTooLarge {
                function,
                count: index,
            })
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) const fn next_verified(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

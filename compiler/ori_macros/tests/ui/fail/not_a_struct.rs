//! Error: Diagnostic derive only supports structs.

use ori_macros::Diagnostic;

#[derive(Diagnostic)]
#[diag(E1001, "some error")]
pub enum NotAStruct {
    Variant,
}

fn main() {}

//! Stack-safe evaluation over canonical expressions.

mod control_flow;
mod dispatch;
mod function_exp;
mod literal;
mod operators;
mod property_lookup;
mod trace;

#[cfg(test)]
mod tests;

use super::Interpreter;

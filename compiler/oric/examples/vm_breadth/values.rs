//! Lossless evaluator and VM result records.

use serde::{Deserialize, Serialize};

use oric::ir::StringInterner;
use oric::EvalOutput;

/// Exact evaluator result, including semantic tags absent from VM values.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum EvaluatorValue {
    Int {
        decimal: String,
    },
    Float {
        bits_hex: String,
    },
    Bool {
        value: bool,
    },
    String {
        value: String,
    },
    Char {
        value: char,
    },
    Byte {
        decimal: String,
    },
    Unit,
    List {
        elements: Vec<Self>,
    },
    Tuple {
        elements: Vec<Self>,
    },
    Some {
        value: Box<Self>,
    },
    None,
    Ok {
        value: Box<Self>,
    },
    Err {
        value: Box<Self>,
    },
    Duration {
        nanoseconds_decimal: String,
    },
    Size {
        bytes_decimal: String,
    },
    Ordering {
        tag_decimal: String,
    },
    Range {
        start_decimal: String,
        end_decimal: Option<String>,
        step_decimal: String,
        inclusive: bool,
    },
    Function {
        description: String,
        arity_decimal: Option<String>,
    },
    Struct {
        description: String,
        field_count_decimal: Option<String>,
    },
    Variant {
        type_name: String,
        variant_name: String,
        fields: Vec<Self>,
    },
    Map {
        entries: Vec<EvaluatorMapEntry>,
    },
    Set {
        elements: Vec<Self>,
    },
    Error {
        message: String,
    },
}

/// One deterministically ordered evaluator map entry.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvaluatorMapEntry {
    pub(crate) key: EvaluatorValue,
    pub(crate) value: EvaluatorValue,
}

impl EvaluatorValue {
    pub(crate) fn from_output(output: &EvalOutput, interner: &StringInterner) -> Self {
        match output {
            EvalOutput::Int(value) => Self::Int {
                decimal: value.to_string(),
            },
            EvalOutput::Float(bits) => Self::Float {
                bits_hex: format!("{bits:016x}"),
            },
            EvalOutput::Bool(value) => Self::Bool { value: *value },
            EvalOutput::Str(value) => Self::String {
                value: value.clone(),
            },
            EvalOutput::Char(value) => Self::Char { value: *value },
            EvalOutput::Byte(value) => Self::Byte {
                decimal: value.to_string(),
            },
            EvalOutput::Void => Self::Unit,
            EvalOutput::List(elements) => Self::List {
                elements: convert_values(elements, interner),
            },
            EvalOutput::Tuple(elements) => Self::Tuple {
                elements: convert_values(elements, interner),
            },
            EvalOutput::Some(value) => Self::Some {
                value: Box::new(Self::from_output(value, interner)),
            },
            EvalOutput::None => Self::None,
            EvalOutput::Ok(value) => Self::Ok {
                value: Box::new(Self::from_output(value, interner)),
            },
            EvalOutput::Err(value) => Self::Err {
                value: Box::new(Self::from_output(value, interner)),
            },
            EvalOutput::Duration(value) => Self::Duration {
                nanoseconds_decimal: value.to_string(),
            },
            EvalOutput::Size(value) => Self::Size {
                bytes_decimal: value.to_string(),
            },
            EvalOutput::Ordering(value) => Self::Ordering {
                tag_decimal: value.to_string(),
            },
            EvalOutput::Range {
                start,
                end,
                step,
                inclusive,
            } => Self::Range {
                start_decimal: start.to_string(),
                end_decimal: end.map(|value| value.to_string()),
                step_decimal: step.to_string(),
                inclusive: *inclusive,
            },
            EvalOutput::Function { description, arity } => Self::Function {
                description: description.clone(),
                arity_decimal: arity.map(|value| value.to_string()),
            },
            EvalOutput::Struct {
                description,
                field_count,
            } => Self::Struct {
                description: description.clone(),
                field_count_decimal: field_count.map(|value| value.to_string()),
            },
            EvalOutput::Variant {
                type_name,
                variant_name,
                fields,
            } => Self::Variant {
                type_name: interner.lookup(*type_name).to_owned(),
                variant_name: interner.lookup(*variant_name).to_owned(),
                fields: convert_values(fields, interner),
            },
            EvalOutput::Map(entries) => Self::Map {
                entries: convert_map(entries, interner),
            },
            EvalOutput::Set(elements) => Self::Set {
                elements: convert_set(elements, interner),
            },
            EvalOutput::Error(message) => Self::Error {
                message: message.clone(),
            },
        }
    }
}

fn convert_values(values: &[EvalOutput], interner: &StringInterner) -> Vec<EvaluatorValue> {
    values
        .iter()
        .map(|value| EvaluatorValue::from_output(value, interner))
        .collect()
}

fn convert_map(
    entries: &[(EvalOutput, EvalOutput)],
    interner: &StringInterner,
) -> Vec<EvaluatorMapEntry> {
    let mut converted: Vec<_> = entries
        .iter()
        .map(|(key, value)| EvaluatorMapEntry {
            key: EvaluatorValue::from_output(key, interner),
            value: EvaluatorValue::from_output(value, interner),
        })
        .collect();
    converted.sort();
    converted
}

fn convert_set(values: &[EvalOutput], interner: &StringInterner) -> Vec<EvaluatorValue> {
    let mut converted = convert_values(values, interner);
    converted.sort();
    converted
}

/// Exact materialized bytecode-VM result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum VmValue {
    Unit,
    Int {
        decimal: String,
    },
    Bool {
        value: bool,
    },
    Float {
        bits_hex: String,
    },
    Char {
        value: char,
    },
    Null,
    String {
        value: String,
    },
    List {
        elements: Vec<Self>,
    },
    Aggregate {
        variant_decimal: String,
        fields: Vec<Self>,
    },
}

impl From<&ori_vm::ExitValue> for VmValue {
    fn from(value: &ori_vm::ExitValue) -> Self {
        match value {
            ori_vm::ExitValue::Unit => Self::Unit,
            ori_vm::ExitValue::Int(value) => Self::Int {
                decimal: value.to_string(),
            },
            ori_vm::ExitValue::Bool(value) => Self::Bool { value: *value },
            ori_vm::ExitValue::Float(value) => Self::Float {
                bits_hex: format!("{:016x}", value.to_bits()),
            },
            ori_vm::ExitValue::Char(value) => Self::Char { value: *value },
            ori_vm::ExitValue::Null => Self::Null,
            ori_vm::ExitValue::String(value) => Self::String {
                value: value.clone(),
            },
            ori_vm::ExitValue::List(elements) => Self::List {
                elements: elements.iter().map(Self::from).collect(),
            },
            ori_vm::ExitValue::Aggregate { variant, fields } => Self::Aggregate {
                variant_decimal: variant.to_string(),
                fields: fields.iter().map(Self::from).collect(),
            },
        }
    }
}

/// Lossless value-comparison result across the evaluator and VM schemas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValueComparison {
    Equal,
    Different,
    Unavailable,
}

pub(crate) fn compare(eval: &EvaluatorValue, vm: &VmValue) -> ValueComparison {
    match (
        ComparableValue::from_eval(eval),
        ComparableValue::from_vm(vm),
    ) {
        (Some(left), Some(right)) if left == right => ValueComparison::Equal,
        (Some(_), Some(_)) => ValueComparison::Different,
        _ => ValueComparison::Unavailable,
    }
}

#[derive(Debug, Eq, PartialEq)]
enum ComparableValue<'value> {
    Unit,
    Int(&'value str),
    Bool(bool),
    Float(&'value str),
    Char(char),
    String(&'value str),
    List(Vec<Self>),
}

impl<'value> ComparableValue<'value> {
    fn from_eval(value: &'value EvaluatorValue) -> Option<Self> {
        match value {
            EvaluatorValue::Unit => Some(Self::Unit),
            EvaluatorValue::Int { decimal } => Some(Self::Int(decimal)),
            EvaluatorValue::Bool { value } => Some(Self::Bool(*value)),
            EvaluatorValue::Float { bits_hex } => Some(Self::Float(bits_hex)),
            EvaluatorValue::Char { value } => Some(Self::Char(*value)),
            EvaluatorValue::String { value } => Some(Self::String(value)),
            EvaluatorValue::List { elements } => elements
                .iter()
                .map(Self::from_eval)
                .collect::<Option<Vec<_>>>()
                .map(Self::List),
            EvaluatorValue::Byte { .. }
            | EvaluatorValue::Tuple { .. }
            | EvaluatorValue::Some { .. }
            | EvaluatorValue::None
            | EvaluatorValue::Ok { .. }
            | EvaluatorValue::Err { .. }
            | EvaluatorValue::Duration { .. }
            | EvaluatorValue::Size { .. }
            | EvaluatorValue::Ordering { .. }
            | EvaluatorValue::Range { .. }
            | EvaluatorValue::Function { .. }
            | EvaluatorValue::Struct { .. }
            | EvaluatorValue::Variant { .. }
            | EvaluatorValue::Map { .. }
            | EvaluatorValue::Set { .. }
            | EvaluatorValue::Error { .. } => None,
        }
    }

    fn from_vm(value: &'value VmValue) -> Option<Self> {
        match value {
            VmValue::Unit => Some(Self::Unit),
            VmValue::Int { decimal } => Some(Self::Int(decimal)),
            VmValue::Bool { value } => Some(Self::Bool(*value)),
            VmValue::Float { bits_hex } => Some(Self::Float(bits_hex)),
            VmValue::Char { value } => Some(Self::Char(*value)),
            VmValue::String { value } => Some(Self::String(value)),
            VmValue::List { elements } => elements
                .iter()
                .map(Self::from_vm)
                .collect::<Option<Vec<_>>>()
                .map(Self::List),
            VmValue::Null | VmValue::Aggregate { .. } => None,
        }
    }
}

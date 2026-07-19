//! Runtime-test-free decomposition of tuple and struct pattern columns.

use super::collect_consumed_bindings;
use super::Specialized;
use ori_ir::canon::tree::{FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath};
use ori_ir::Name;
use rustc_hash::FxHashSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SingleConstructorColumn {
    Tuple { fields: usize },
    Struct { field_names: Vec<Name> },
}

impl SingleConstructorColumn {
    fn field_count(&self) -> usize {
        match self {
            Self::Tuple { fields } => *fields,
            Self::Struct { field_names } => field_names.len(),
        }
    }

    fn path_instruction(&self, index: usize) -> PathInstruction {
        match self {
            Self::Tuple { .. } => {
                let Ok(index) = u32::try_from(index) else {
                    panic!("tuple pattern field index {index} exceeds the path index range");
                };
                PathInstruction::TupleIndex(index)
            }
            Self::Struct { field_names } => PathInstruction::StructField(field_names[index]),
        }
    }
}

/// Returns the common tuple or struct shape when every concrete pattern shares it.
#[must_use = "the absence of a value must be handled"]
pub(super) fn single_constructor_column(
    matrix: &PatternMatrix,
    col: usize,
) -> Option<SingleConstructorColumn> {
    let mut shape = None;
    for row in matrix {
        let pat = unwrap_at_or(&row.patterns[col]);
        let candidate = match pat {
            FlatPattern::Tuple(elements) => Some(SingleConstructorColumn::Tuple {
                fields: elements.len(),
            }),
            FlatPattern::Struct { fields } => Some(SingleConstructorColumn::Struct {
                field_names: fields.iter().map(|(name, _)| *name).collect(),
            }),
            FlatPattern::Wildcard | FlatPattern::Binding(_) => None,
            _ => return None,
        };
        if let Some(candidate) = candidate {
            match &shape {
                Some(existing) if existing != &candidate => return None,
                Some(_) => {}
                None => shape = Some(candidate),
            }
        }
    }
    shape
}

fn unwrap_at_or(pat: &FlatPattern) -> &FlatPattern {
    match pat {
        FlatPattern::At { inner, .. } => unwrap_at_or(inner),
        _ => pat,
    }
}

/// Expands a tuple or struct column without emitting a runtime test.
pub(super) fn decompose_single_constructor(
    matrix: &PatternMatrix,
    col: usize,
    paths: &[ScrutineePath],
    base_path: &ScrutineePath,
    shape: &SingleConstructorColumn,
) -> Specialized {
    let sub_count = shape.field_count();

    let mut new_paths = Vec::with_capacity(paths.len() - 1 + sub_count);
    new_paths.extend_from_slice(&paths[..col]);
    for i in 0..sub_count {
        let mut sub_path = base_path.clone();
        sub_path.push(shape.path_instruction(i));
        new_paths.push(sub_path);
    }
    new_paths.extend_from_slice(&paths[col + 1..]);

    let new_matrix = matrix
        .iter()
        .map(|row| {
            let mut bindings = row.bindings.clone();
            bindings.extend(collect_consumed_bindings(&row.patterns[col], base_path));

            let sub_pats = decompose_single_ctor_pattern(&row.patterns[col], sub_count);
            let mut discard_paths = row.discard_paths.clone();
            let mut seen_discard_paths: FxHashSet<_> = discard_paths.iter().cloned().collect();
            if matches!(
                unwrap_at_or(&row.patterns[col]),
                FlatPattern::Tuple(_) | FlatPattern::Struct { .. }
            ) {
                for (sub_pattern, sub_path) in
                    sub_pats.iter().zip(new_paths[col..col + sub_count].iter())
                {
                    if matches!(sub_pattern, FlatPattern::Wildcard)
                        && seen_discard_paths.insert(sub_path.clone())
                    {
                        discard_paths.push(sub_path.clone());
                    }
                }
            }
            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1 + sub_pats.len());
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend(sub_pats);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
                bindings,
                discard_paths,
            }
        })
        .collect();

    Specialized {
        matrix: new_matrix,
        paths: new_paths,
    }
}

fn decompose_single_ctor_pattern(pat: &FlatPattern, sub_count: usize) -> Vec<FlatPattern> {
    match pat {
        FlatPattern::Tuple(elements) => elements.clone(),
        FlatPattern::Struct { fields } => fields.iter().map(|(_, sub)| sub.clone()).collect(),
        FlatPattern::Wildcard | FlatPattern::Binding(_) => {
            vec![FlatPattern::Wildcard; sub_count]
        }
        FlatPattern::At { inner, .. } => decompose_single_ctor_pattern(inner, sub_count),
        FlatPattern::Or(alts) => {
            if let Some(first) = alts.first() {
                decompose_single_ctor_pattern(first, sub_count)
            } else {
                vec![FlatPattern::Wildcard; sub_count]
            }
        }
        _ => vec![FlatPattern::Wildcard; sub_count],
    }
}

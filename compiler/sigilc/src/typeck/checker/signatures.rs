//! Function Signature Inference
//!
//! Handles inferring function signatures from declarations.

use std::collections::HashMap;
use crate::ir::{Name, Function};
use crate::types::Type;
use super::TypeChecker;
use super::types::{FunctionType, GenericBound};

impl<'a> TypeChecker<'a> {
    /// Infer function signature from declaration.
    ///
    /// For generic functions, creates a fresh type variable for each generic parameter
    /// and uses it consistently across all parameter type annotations that reference
    /// that generic. This enables proper constraint checking at call sites.
    pub(crate) fn infer_function_signature(&mut self, func: &Function) -> FunctionType {
        // Step 1: Create fresh type variables for each generic parameter
        let generic_params = self.arena.get_generic_params(func.generics);
        let mut generic_type_vars: HashMap<Name, Type> = HashMap::new();

        for gp in generic_params {
            let type_var = self.ctx.fresh_var();
            generic_type_vars.insert(gp.name, type_var);
        }

        // Step 2: Collect generic bounds with their type variables
        let mut generics = Vec::new();
        for gp in generic_params {
            let bounds: Vec<Vec<Name>> = gp.bounds.iter()
                .map(|b| b.path.clone())
                .collect();
            let type_var = generic_type_vars.get(&gp.name).cloned()
                .unwrap_or_else(|| self.ctx.fresh_var());
            generics.push(GenericBound {
                param: gp.name,
                bounds,
                type_var,
            });
        }

        // Step 3: Merge where clause bounds
        for wc in &func.where_clauses {
            if let Some(gb) = generics.iter_mut().find(|g| g.param == wc.param) {
                // Add bounds from where clause to existing generic
                for bound in &wc.bounds {
                    gb.bounds.push(bound.path.clone());
                }
            } else {
                // Where clause for a param not in generic list - create new entry
                let bounds: Vec<Vec<Name>> = wc.bounds.iter()
                    .map(|b| b.path.clone())
                    .collect();
                let type_var = generic_type_vars.get(&wc.param).cloned()
                    .unwrap_or_else(|| self.ctx.fresh_var());
                generics.push(GenericBound {
                    param: wc.param,
                    bounds,
                    type_var,
                });
            }
        }

        // Step 4: Convert parameter types, using generic type vars when applicable
        let params: Vec<Type> = self.arena.get_params(func.params)
            .iter()
            .map(|p| {
                // Check if this param's type annotation refers to a generic parameter
                if let Some(type_name) = p.type_name {
                    if let Some(type_var) = generic_type_vars.get(&type_name) {
                        return type_var.clone();
                    }
                }
                // Fall back to regular type conversion
                match p.ty {
                    Some(type_id) => self.type_id_to_type(type_id),
                    None => self.ctx.fresh_var(),
                }
            })
            .collect();

        // Step 5: Handle return type (TODO: also check for generic return types)
        let return_type = match func.return_ty {
            Some(type_id) => self.type_id_to_type(type_id),
            None => self.ctx.fresh_var(),
        };

        FunctionType {
            name: func.name,
            generics,
            params,
            return_type,
        }
    }
}

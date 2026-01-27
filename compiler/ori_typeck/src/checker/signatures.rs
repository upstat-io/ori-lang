//! Function Signature Inference
//!
//! Handles inferring function signatures from declarations.

use super::types::{FunctionType, GenericBound, WhereConstraint};
use super::TypeChecker;
use ori_ir::{Function, Name, TypeId};
use ori_types::Type;
use std::collections::HashMap;

impl TypeChecker<'_> {
    /// Infer function signature from declaration.
    ///
    /// For generic functions, creates a fresh type variable for each generic parameter
    /// and uses it consistently across all parameter type annotations that reference
    /// that generic. This enables proper constraint checking at call sites.
    pub(crate) fn infer_function_signature(&mut self, func: &Function) -> FunctionType {
        // Step 1: Create fresh type variables for each generic parameter
        let generic_params = self.context.arena.get_generic_params(func.generics);
        let mut generic_type_vars: HashMap<Name, Type> = HashMap::new();
        let mut generic_type_var_ids: HashMap<Name, TypeId> = HashMap::new();

        for gp in generic_params {
            let type_var = self.inference.ctx.fresh_var();
            let type_var_id = type_var.to_type_id(self.inference.env.interner());
            generic_type_vars.insert(gp.name, type_var);
            generic_type_var_ids.insert(gp.name, type_var_id);
        }

        // Step 2: Collect generic bounds with their type variables
        let mut generics = Vec::new();
        for gp in generic_params {
            let bounds: Vec<Vec<Name>> = gp.bounds.iter().map(ori_ir::TraitBound::path).collect();
            let type_var_id = generic_type_var_ids
                .get(&gp.name)
                .copied()
                .unwrap_or_else(|| self.inference.ctx.fresh_var_id());
            generics.push(GenericBound {
                param: gp.name,
                bounds,
                type_var: type_var_id,
            });
        }

        // Step 3: Process where clauses
        // - Non-projection clauses (where T: Eq) merge into generics
        // - Projection clauses (where T.Item: Eq) go into where_constraints
        let mut where_constraints = Vec::new();
        for wc in &func.where_clauses {
            let bounds: Vec<Vec<Name>> = wc.bounds.iter().map(ori_ir::TraitBound::path).collect();
            let type_var_id = generic_type_var_ids
                .get(&wc.param)
                .copied()
                .unwrap_or_else(|| self.inference.ctx.fresh_var_id());

            if wc.projection.is_some() {
                // Projection constraint: where T.Item: Eq
                // Store separately for projection-aware checking
                where_constraints.push(WhereConstraint {
                    param: wc.param,
                    projection: wc.projection,
                    bounds,
                    type_var: type_var_id,
                });
            } else {
                // Non-projection constraint: where T: Eq
                // Merge into generics as before
                if let Some(gb) = generics.iter_mut().find(|g| g.param == wc.param) {
                    for bound in &wc.bounds {
                        gb.bounds.push(bound.path());
                    }
                } else {
                    generics.push(GenericBound {
                        param: wc.param,
                        bounds,
                        type_var: type_var_id,
                    });
                }
            }
        }

        // Step 4: Convert parameter types, using generic type vars when applicable
        // First resolve all types, then convert to TypeId to avoid borrow issues
        let params_as_types: Vec<Type> = self
            .context
            .arena
            .get_params(func.params)
            .iter()
            .map(|p| match &p.ty {
                Some(parsed_ty) => {
                    self.resolve_parsed_type_with_generics(parsed_ty, &generic_type_vars)
                }
                None => self.inference.ctx.fresh_var(),
            })
            .collect();

        // Step 5: Handle return type, also checking for generic return types
        let return_type_as_type = match &func.return_ty {
            Some(parsed_ty) => {
                self.resolve_parsed_type_with_generics(parsed_ty, &generic_type_vars)
            }
            None => self.inference.ctx.fresh_var(),
        };

        // Convert to TypeId after all mutable operations are done
        let interner = self.inference.env.interner();
        let params: Vec<TypeId> = params_as_types
            .iter()
            .map(|ty| ty.to_type_id(interner))
            .collect();
        let return_type = return_type_as_type.to_type_id(interner);

        let capabilities: Vec<Name> = func
            .capabilities
            .iter()
            .map(|cap_ref| cap_ref.name)
            .collect();

        FunctionType {
            name: func.name,
            generics,
            where_constraints,
            params,
            return_type,
            capabilities,
        }
    }
}

use crate::ast::{Declaration, Expression, Statement};
use crate::codegen::compiler::Compiler;
use crate::error::CompileError;
use inkwell::values::FunctionValue;

impl<'ctx> Compiler<'ctx> {
    /// Walk a statement list and substitute generic placeholders by concrete
    /// types in every expression position. Recursive over `if`/`while`/`match`
    /// arms and `Block` expressions. Phase 3 of the `compiler.rs` split (#113):
    /// moved verbatim from `src/codegen/compiler.rs` — behaviour-preserving.
    pub(in crate::codegen) fn substitute_types_in_body(
        &self,
        b: &mut [Statement],
        ph: &[String],
        conc: &[String],
    ) {
        for s in b.iter_mut() {
            match s {
                Statement::Let { value, .. } => self.substitute_types_in_expr(value, ph, conc),
                Statement::Assignment { target, value, .. } => {
                    self.substitute_types_in_expr(target, ph, conc);
                    self.substitute_types_in_expr(value, ph, conc);
                }
                Statement::Return { value, .. } => self.substitute_types_in_expr(value, ph, conc),
                Statement::ExpressionStmt(e, _) => self.substitute_types_in_expr(e, ph, conc),
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.substitute_types_in_expr(condition, ph, conc);
                    self.substitute_types_in_body(then_branch, ph, conc);
                    if let Some(eb) = else_branch {
                        self.substitute_types_in_body(eb, ph, conc);
                    }
                }
                Statement::While {
                    condition, body, ..
                } => {
                    self.substitute_types_in_expr(condition, ph, conc);
                    self.substitute_types_in_body(body, ph, conc);
                }
                Statement::For { range, body, .. } => {
                    self.substitute_types_in_expr(range, ph, conc);
                    self.substitute_types_in_body(body, ph, conc);
                }
                Statement::UnsafeBlock(stmts, _) | Statement::Spawn(stmts, _) => {
                    self.substitute_types_in_body(stmts, ph, conc)
                }
                Statement::Match {
                    condition, arms, ..
                } => {
                    self.substitute_types_in_expr(condition, ph, conc);
                    for arm in arms {
                        self.substitute_types_in_body(&mut arm.body, ph, conc);
                    }
                }
                _ => {}
            }
        }
    }

    /// Substitute generic placeholders in a single expression (and recurse into
    /// its sub-expressions). Type-string positions (call function name, cast
    /// target, generic args, identifier) use `Self::substitute_type_string`
    /// (the lossless token-aware substitution, #67 — moved to `intrinsics.rs`
    /// in phase 1). Phase 3 of #113: moved verbatim.
    pub(in crate::codegen) fn substitute_types_in_expr(
        &self,
        e: &mut Expression,
        ph: &[String],
        conc: &[String],
    ) {
        match e {
            Expression::Infix { left, right, .. } => {
                self.substitute_types_in_expr(left, ph, conc);
                self.substitute_types_in_expr(right, ph, conc);
            }
            Expression::Call {
                function,
                generic_args,
                arguments,
                ..
            } => {
                *function = Self::substitute_type_string(function, ph, conc);
                for arg in generic_args.iter_mut() {
                    *arg = Self::substitute_type_string(arg, ph, conc);
                }
                for arg in arguments {
                    self.substitute_types_in_expr(arg, ph, conc);
                }
            }
            Expression::EnumInst {
                name,
                generic_args,
                arguments,
                ..
            } => {
                *name = Self::substitute_type_string(name, ph, conc);
                for arg in generic_args.iter_mut() {
                    *arg = Self::substitute_type_string(arg, ph, conc);
                }
                for arg in arguments {
                    self.substitute_types_in_expr(arg, ph, conc);
                }
            }
            Expression::StructInst {
                name,
                generic_args,
                fields,
                ..
            } => {
                *name = Self::substitute_type_string(name, ph, conc);
                for arg in generic_args.iter_mut() {
                    *arg = Self::substitute_type_string(arg, ph, conc);
                }
                for (_, val) in fields {
                    self.substitute_types_in_expr(val, ph, conc);
                }
            }
            Expression::Cast { expr, target, .. } => {
                self.substitute_types_in_expr(expr, ph, conc);
                *target = Self::substitute_type_string(target, ph, conc);
            }
            Expression::Deref { expr, .. } => self.substitute_types_in_expr(expr, ph, conc),
            Expression::Intrinsic { arguments, .. } => {
                for arg in arguments {
                    self.substitute_types_in_expr(arg, ph, conc);
                }
            }
            Expression::Block { statements, .. } => {
                self.substitute_types_in_body(statements, ph, conc)
            }
            Expression::Identifier(n, _) => {
                *n = Self::substitute_type_string(n, ph, conc);
            }
            Expression::MemberAccess { receiver, .. } => {
                self.substitute_types_in_expr(receiver, ph, conc)
            }
            Expression::MethodCall {
                receiver,
                generic_args,
                arguments,
                ..
            } => {
                self.substitute_types_in_expr(receiver, ph, conc);
                for arg in generic_args {
                    *arg = Self::substitute_type_string(arg, ph, conc);
                }
                for arg in arguments {
                    self.substitute_types_in_expr(arg, ph, conc);
                }
            }
            Expression::Match {
                condition, arms, ..
            } => {
                self.substitute_types_in_expr(condition, ph, conc);
                for arm in arms {
                    self.substitute_types_in_body(&mut arm.body, ph, conc);
                }
            }
            _ => {}
        }
    }

    /// Instantiate a generic function `bn<T1, T2, ...>` with concrete types
    /// `ga`. Clones the declaration, substitutes placeholders in params/
    /// return-type/body via `substitute_type_string`, registers the clone
    /// under `bn_T1_T2_...` in `self.decls`, and lowers it through
    /// `self.compile_function`. Forward instantiations are cached in
    /// `self.compiled_instances` / `self.module`. Phase 3 of #113: moved
    /// verbatim. `compile_function` stays in `compiler.rs` (later phase).
    pub(in crate::codegen) fn instantiate_function(
        &mut self,
        bn: &str,
        ga: &[String],
    ) -> Result<FunctionValue<'ctx>, CompileError> {
        let d = self.decls.get(bn).cloned().ok_or_else(|| {
            CompileError::internal(format!("Generic function '{}' not found", bn))
        })?;
        if let Declaration::Function(mut f) = d {
            let ph = f.generic_params.clone();
            let nn = format!("{}_{}", bn, ga.join("_"));
            if let Some(e) = self.module.get_function(&nn) {
                return Ok(e);
            }

            f.name = nn.clone();
            f.generic_params = vec![];
            let n = ph.len().min(ga.len());
            for (_, pt, _) in f.params.iter_mut() {
                *pt = Self::substitute_type_string(pt, &ph[..n], &ga[..n]);
            }
            f.return_type = Self::substitute_type_string(&f.return_type, &ph[..n], &ga[..n]);
            if let Some(body) = &mut f.body {
                self.substitute_types_in_body(body, &ph[..n], &ga[..n]);
            }

            self.decls
                .insert(nn.clone(), Declaration::Function(f.clone()));
            self.compiled_instances.insert(nn.clone());
            self.compile_function(&Declaration::Function(f))
        } else {
            Err(CompileError::internal(format!(
                "'{}' is not a function",
                bn
            )))
        }
    }
}

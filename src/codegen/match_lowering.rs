use crate::ast::{Declaration, MatchArm};
use crate::codegen::compiler::Compiler;
use crate::error::CompileError;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue};
use inkwell::{AddressSpace, IntPredicate};
use std::collections::HashMap;

type VarMap<'ctx> = HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>;

impl<'ctx> Compiler<'ctx> {
    /// Shared match lowering for all four former call paths: enum and
    /// primitive (i64/String/range) `match`, in statement and expression
    /// position. Deduplicated from `control_flow.rs` and `expressions.rs`
    /// (#183, audit 2026-09-15 finding 10). Returns the merged phi value,
    /// or `None` when no arm produces a value (statement position: no
    /// result; expression position: the caller substitutes a zero).
    ///
    /// The four copies had drifted apart; the unified version always:
    /// - binds ALL enum payload params positionally (#161 / #174),
    /// - registers primitive binding vars in the arm frame for every type
    ///   (String bindings used to store but never insert in the statement
    ///   copy, leaving the name unresolved),
    /// - skips the last-arm zero sentinel for binding-variable arms (their
    ///   dispatch block branches unconditionally to the arm body, so a
    ///   sentinel phi edge would corrupt the IR — the #152 class).
    pub(in crate::codegen) fn lower_match(
        &mut self,
        cond_value: BasicValueEnum<'ctx>,
        cond_type_name: &str,
        arms: &[MatchArm],
        variables: &VarMap<'ctx>,
        function: FunctionValue<'ctx>,
        is_statement: bool,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CompileError> {
        let i64_t = self.context.i64_type();
        let pt = self.context.ptr_type(AddressSpace::default());
        let cv = cond_value;
        let ctn = cond_type_name;

        // Per-position block/phi names keep the generated IR identical to
        // the pre-refactor output of both call sites.
        let (exit_name, next_name, arm_prefix, phi_name) = if is_statement {
            ("matchexit", "match_next", "arm_", "matchres")
        } else {
            (
                "match_expr_exit",
                "match_arm_next",
                "match_arm_",
                "match_res",
            )
        };

        let exit_bb = self.context.append_basic_block(function, exit_name);
        let mut phis = Vec::new();

        let cbn = if ctn.contains('<') {
            ctn.split('<')
                .next()
                .ok_or_else(|| CompileError::internal("Invalid type name".to_string()))?
                .to_string()
        } else {
            ctn.to_string()
        };
        let fen = self
            .resolve_fuzzy_name(&self.enum_types, &cbn)
            .unwrap_or(cbn.clone());

        if let Some(et_ref) = self.enum_types.get(&fen) {
            let et = *et_ref;
            let ep = cv.into_pointer_value();
            let tag = self
                .builder
                .build_load(
                    i64_t,
                    self.builder.build_struct_gep(et, ep, 0, "tagptr")?,
                    "tag",
                )?
                .into_int_value();
            let na = arms.len();
            for (i, arm) in arms.iter().enumerate() {
                let ab = self
                    .context
                    .append_basic_block(function, &format!("{}{}_{}", arm_prefix, i, arm.pattern));
                let is_last = i == na - 1;
                let nb = if is_last {
                    exit_bb
                } else {
                    self.context.append_basic_block(function, next_name)
                };

                // Get all patterns to check
                let all_patterns: Vec<String> = if arm.patterns.is_empty() {
                    vec![arm.pattern.clone()]
                } else {
                    arm.patterns.clone()
                };

                let is_default = all_patterns.iter().any(|p| p == "_");
                let mut arm_match_cond: Option<inkwell::values::IntValue<'ctx>> = None;

                if !is_default && let Some(Declaration::Enum(e_decl)) = self.decls.get(&fen) {
                    for pat in &all_patterns {
                        let mut at = i as u64;
                        for (vi, v) in e_decl.variants.iter().enumerate() {
                            if pat == &v.name
                                || pat.ends_with(&format!(".{}", v.name))
                                || pat.ends_with(&format!("::{}", v.name))
                            {
                                at = vi as u64;
                                break;
                            }
                        }
                        // Fallback for common variants
                        if at == i as u64
                            && (pat == "Some"
                                || pat == "Ok"
                                || pat.ends_with(".Some")
                                || pat.ends_with("::Some"))
                        {
                            at = 0;
                        }
                        if at == i as u64
                            && (pat == "None"
                                || pat == "Err"
                                || pat.ends_with(".None")
                                || pat.ends_with("::None"))
                        {
                            at = 1;
                        }

                        let cond = self.builder.build_int_compare(
                            IntPredicate::EQ,
                            tag,
                            i64_t.const_int(at, false),
                            "is_arm",
                        )?;
                        arm_match_cond = Some(match arm_match_cond {
                            Some(prev) => self.builder.build_or(prev, cond, "arm_or")?,
                            None => cond,
                        });
                    }
                }

                if is_default && arm_match_cond.is_none() {
                    self.builder.build_unconditional_branch(ab)?;
                } else if let Some(cond) = arm_match_cond {
                    self.builder.build_conditional_branch(cond, ab, nb)?;
                } else {
                    self.builder.build_unconditional_branch(nb)?;
                }
                if is_last && !is_default {
                    let test_bb = self.builder.get_insert_block().ok_or_else(|| {
                        CompileError::internal("No active insert block".to_string())
                    })?;
                    phis.push((i64_t.const_zero().into(), test_bb));
                }
                self.builder.position_at_end(ab);
                let mut av = variables.clone();
                if !arm.params.is_empty() {
                    let dp = self.builder.build_struct_gep(et, ep, 1, "arm_dataptr")?;
                    // Resolve the matched variant's element types once,
                    // then bind each param positionally: element i lives
                    // at byte offset i*8 in the enum payload buffer (all
                    // Aion values are 8 bytes — ptr or i64). #161 / #174.
                    let mut data_types: Vec<String> = Vec::new();
                    if let Some(Declaration::Enum(e_decl)) = self.decls.get(&fen) {
                        for v in &e_decl.variants {
                            if arm.pattern == v.name
                                || arm.pattern.ends_with(&format!(".{}", v.name))
                                || arm.pattern.ends_with(&format!("::{}", v.name))
                            {
                                data_types = v.data_types.clone();
                                break;
                            }
                        }
                    }
                    let base_ptr = self
                        .builder
                        .build_bit_cast(dp, pt, "arm_datacast")?
                        .into_pointer_value();
                    for (i, param) in arm.params.iter().enumerate() {
                        let ptn = data_types
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| "i64".to_string());
                        let lt = self.aion_type_to_llvm(&ptn);
                        let elem_ptr = if i == 0 {
                            base_ptr
                        } else {
                            let byte_off = (i * 8) as u64;
                            unsafe {
                                self.builder.build_in_bounds_gep(
                                    self.context.i8_type(),
                                    base_ptr,
                                    &[i64_t.const_int(byte_off, false)],
                                    &format!("arm_off_{}", i),
                                )?
                            }
                        };
                        let lv_val = self.builder.build_load(lt, elem_ptr, param)?;
                        let pa = self.builder.build_alloca(lt, param)?;
                        self.builder.build_store(pa, lv_val)?;
                        av.insert(param.clone(), (pa, lt, ptn));
                    }
                }

                // Evaluate guard condition if present
                if let Some(guard_expr) = &arm.guard {
                    let guard_val = self
                        .compile_expr(guard_expr, &av, function)?
                        .into_int_value();
                    let guard_pass_bb = self.context.append_basic_block(function, "guard_pass");
                    let guard_fail_bb = nb;
                    let guard_cond = self.builder.build_int_compare(
                        IntPredicate::NE,
                        guard_val,
                        i64_t.const_zero(),
                        "guard_cond",
                    )?;
                    self.builder.build_conditional_branch(
                        guard_cond,
                        guard_pass_bb,
                        guard_fail_bb,
                    )?;
                    self.builder.position_at_end(guard_pass_bb);
                }

                let ar = self.compile_block(&arm.body, &mut av, function)?;
                let abf = self
                    .builder
                    .get_insert_block()
                    .ok_or_else(|| CompileError::internal("No active insert block".to_string()))?;
                if abf.get_terminator().is_none() {
                    let v = ar.unwrap_or(i64_t.const_zero().into());
                    phis.push((v, abf));
                }
                if !is_last {
                    self.builder.position_at_end(nb);
                }
            }
        } else {
            // Match on primitives (i64, String)
            let na = arms.len();
            for (i, arm) in arms.iter().enumerate() {
                let pattern_clean = arm
                    .pattern
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>();
                let ab = self.context.append_basic_block(
                    function,
                    &format!("{}{}_{}", arm_prefix, i, pattern_clean),
                );
                let is_last = i == na - 1;
                let nb = if is_last {
                    exit_bb
                } else {
                    self.context.append_basic_block(function, next_name)
                };

                // Get all patterns
                let all_patterns: Vec<String> = if arm.patterns.is_empty() {
                    vec![arm.pattern.clone()]
                } else {
                    arm.patterns.clone()
                };

                let is_default = all_patterns.iter().any(|p| p == "_");
                // If pattern is a binding variable (not a number) and we have params or guard, treat as wildcard
                let is_binding_var = !arm.params.is_empty() || arm.guard.is_some();
                let mut prim_match_cond: Option<inkwell::values::IntValue<'ctx>> = None;

                if !is_default && !is_binding_var {
                    if ctn == "i64" || ctn == "Integer" {
                        for pat in &all_patterns {
                            if let Some((start_str, end_str)) = pat.split_once("..") {
                                if let (Ok(start), Ok(end)) =
                                    (start_str.parse::<i64>(), end_str.parse::<i64>())
                                {
                                    let cv_val = cv.into_int_value();
                                    let cond_start = self.builder.build_int_compare(
                                        IntPredicate::SGE,
                                        cv_val,
                                        i64_t.const_int(start as u64, false),
                                        "range_start",
                                    )?;
                                    let cond_end = self.builder.build_int_compare(
                                        IntPredicate::SLE,
                                        cv_val,
                                        i64_t.const_int(end as u64, false),
                                        "range_end",
                                    )?;
                                    let range_cond = self.builder.build_and(
                                        cond_start,
                                        cond_end,
                                        "range_cond",
                                    )?;
                                    prim_match_cond = Some(match prim_match_cond {
                                        Some(prev) => {
                                            self.builder.build_or(prev, range_cond, "range_or")?
                                        }
                                        None => range_cond,
                                    });
                                }
                            } else if let Ok(val) = pat.parse::<i64>() {
                                let cond = self.builder.build_int_compare(
                                    IntPredicate::EQ,
                                    cv.into_int_value(),
                                    i64_t.const_int(val as u64, false),
                                    "match_cond",
                                )?;
                                prim_match_cond = Some(match prim_match_cond {
                                    Some(prev) => self.builder.build_or(prev, cond, "match_or")?,
                                    None => cond,
                                });
                            }
                        }
                    } else if ctn == "String" {
                        for pat in &all_patterns {
                            let pattern_str = if pat.starts_with('"') && pat.ends_with('"') {
                                pat[1..pat.len() - 1].to_string()
                            } else {
                                pat.clone()
                            };

                            let ps = self
                                .builder
                                .build_global_string_ptr(&pattern_str, "match_pattern")?;
                            let fnc = self.module.get_function("aion_str_eq").ok_or_else(|| {
                                CompileError::internal("aion_str_eq not found".to_string())
                            })?;
                            let cmp = self
                                .builder
                                .build_call(
                                    fnc,
                                    &[cv.into(), ps.as_basic_value_enum().into()],
                                    "streq",
                                )?
                                .try_as_basic_value()
                                .unwrap_basic()
                                .into_int_value();
                            let cond = self.builder.build_int_compare(
                                IntPredicate::NE,
                                cmp,
                                i64_t.const_zero(),
                                "match_cond",
                            )?;
                            prim_match_cond = Some(match prim_match_cond {
                                Some(prev) => self.builder.build_or(prev, cond, "match_or")?,
                                None => cond,
                            });
                        }
                    }
                }

                if (is_default || is_binding_var) && prim_match_cond.is_none() {
                    self.builder.build_unconditional_branch(ab)?;
                } else if let Some(cond) = prim_match_cond {
                    self.builder.build_conditional_branch(cond, ab, nb)?;
                } else {
                    self.builder.build_unconditional_branch(nb)?;
                }

                if is_last && !is_default && !is_binding_var {
                    let test_bb = self.builder.get_insert_block().ok_or_else(|| {
                        CompileError::internal("No active insert block".to_string())
                    })?;
                    phis.push((i64_t.const_zero().into(), test_bb));
                }

                self.builder.position_at_end(ab);

                // For primitives, bind pattern variable if present and evaluate guard
                let mut av = variables.clone();
                if !arm.params.is_empty() {
                    let cv_type = cv.get_type();
                    let pa = self.builder.build_alloca(cv_type, &arm.params[0])?;
                    if ctn == "String" {
                        let cv_ptr = cv.into_pointer_value();
                        self.builder.build_store(pa, cv_ptr)?;
                        // #183 — the statement copy stored without
                        // inserting, leaving the binding unresolved.
                        av.insert(arm.params[0].clone(), (pa, cv_type, ctn.to_string()));
                    } else if ctn != "i64" && ctn != "Integer" {
                        // For structs, store the pointer directly
                        // The pointer already points to the struct data
                        let cv_ptr = cv.into_pointer_value();
                        self.builder.build_store(pa, cv_ptr)?;
                        // Update type to pointer so member access works
                        let ptr_type = pt;
                        av.insert(
                            arm.params[0].clone(),
                            (pa, ptr_type.into(), format!("*{}", ctn)),
                        );
                    } else {
                        self.builder.build_store(pa, cv)?;
                        av.insert(arm.params[0].clone(), (pa, cv_type, ctn.to_string()));
                    }
                }

                // Evaluate guard condition if present
                if let Some(guard_expr) = &arm.guard {
                    let guard_val = self
                        .compile_expr(guard_expr, &av, function)?
                        .into_int_value();
                    let guard_pass_bb = self.context.append_basic_block(function, "guard_pass");
                    let guard_fail_bb = nb;
                    let guard_cond = self.builder.build_int_compare(
                        IntPredicate::NE,
                        guard_val,
                        i64_t.const_zero(),
                        "guard_cond",
                    )?;
                    self.builder.build_conditional_branch(
                        guard_cond,
                        guard_pass_bb,
                        guard_fail_bb,
                    )?;
                    self.builder.position_at_end(guard_pass_bb);
                }

                let ar = self.compile_block(&arm.body, &mut av, function)?;
                let abf = self
                    .builder
                    .get_insert_block()
                    .ok_or_else(|| CompileError::internal("No active insert block".to_string()))?;
                if abf.get_terminator().is_none() {
                    let v = ar.unwrap_or(i64_t.const_zero().into());
                    phis.push((v, abf));
                }
                if !is_last {
                    self.builder.position_at_end(nb);
                }
            }
        }

        self.builder.position_at_end(exit_bb);
        if exit_bb.get_terminator().is_none() {
            if phis.is_empty() {
                self.builder.build_unreachable()?;
                Ok(None)
            } else {
                let target_type = phis[0].0.get_type();
                let mut final_phis = Vec::new();
                for (mut v, b) in phis {
                    self.builder.position_at_end(b);
                    if v.get_type() != target_type {
                        if target_type.is_pointer_type() && v.is_int_value() {
                            v = self
                                .builder
                                .build_int_to_ptr(v.into_int_value(), pt, "phi_ptr")?
                                .into();
                        } else if target_type.is_int_type() && v.is_pointer_value() {
                            v = self
                                .builder
                                .build_ptr_to_int(v.into_pointer_value(), i64_t, "phi_int")?
                                .into();
                        }
                    }
                    // The block may already terminate (last arm's dispatch
                    // block ends in a conditional branch to exit_bb) — the
                    // phi edge then already exists and a second terminator
                    // would corrupt the IR (the #152 segfault). #152.
                    if b.get_terminator().is_none() {
                        self.builder.build_unconditional_branch(exit_bb)?;
                    }
                    final_phis.push((v, b));
                }
                self.builder.position_at_end(exit_bb);
                let phi = self.builder.build_phi(target_type, phi_name)?;
                for (v, b) in final_phis {
                    phi.add_incoming(&[(&v, b)]);
                }
                Ok(Some(phi.as_basic_value()))
            }
        } else {
            Ok(None)
        }
    }
}

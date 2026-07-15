use crate::ast::{Declaration, Statement};
use crate::codegen::compiler::Compiler;
use crate::error::CompileError;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue};
use inkwell::{AddressSpace, IntPredicate};
use std::collections::HashMap;

impl<'ctx> Compiler<'ctx> {
    /// Statement-level codegen: lower a statement list (`let`/`return`/`if`/
    /// `while`/`match`/`unsafe`/`spawn`/assignment) into LLVM IR. Recurses into
    /// nested blocks and arms. Hosts the `unreachable` merge-block pitfall
    /// documented in `docs/architecture.md`. Phase 4 of the `compiler.rs`
    /// split (#113): moved verbatim from `src/codegen/compiler.rs` —
    /// behaviour-preserving code motion.
    pub(in crate::codegen) fn compile_block(
        &mut self,
        body: &[Statement],
        variables: &mut HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>,
        function: FunctionValue<'ctx>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CompileError> {
        let mut lv = None;
        let i64_t = self.context.i64_type();
        let pt = self.context.ptr_type(AddressSpace::default());

        for s in body {
            match s {
                Statement::Let {
                    name,
                    value,
                    explicit_type,
                    ..
                } => {
                    let v = self.compile_expr(value, variables, function)?;
                    let inferred_vt = v.get_type();
                    let inferred_vtn = self.get_expr_type_name(value, variables).replace(" ", "");
                    // An explicit type annotation (`let x: T = expr`) wins over inference
                    // for the variable's stored type-name (used by `get_expr_type_name`
                    // downstream) and for the LLVM alloca type when the inferred type
                    // is ambiguous (e.g. intrinsics that lower to i64 but carry a
                    // struct/pointer meaning, or pointer-vs-int casts). #78.
                    if let Some(et) = explicit_type {
                        let et_clean = et.replace(" ", "");
                        let llvm_t = self.aion_type_to_llvm(&et_clean);
                        let final_v = if llvm_t != inferred_vt {
                            if llvm_t.is_pointer_type() && v.is_int_value() {
                                self.builder
                                    .build_int_to_ptr(
                                        v.into_int_value(),
                                        llvm_t.into_pointer_type(),
                                        "let_coerce",
                                    )?
                                    .into()
                            } else if llvm_t.is_int_type() && v.is_pointer_value() {
                                self.builder
                                    .build_ptr_to_int(
                                        v.into_pointer_value(),
                                        llvm_t.into_int_type(),
                                        "let_coerce",
                                    )?
                                    .into()
                            } else if llvm_t.is_int_type() && v.is_int_value() {
                                // Integer width coercion: widen (zext/sext) or
                                // narrow (trunc) the literal/value to the
                                // annotated integer type. Lets `let x: i32 = 42`
                                // store an i64 literal into an i32 slot. #52.
                                self.coerce_int_width(
                                    v.into_int_value(),
                                    llvm_t.into_int_type(),
                                    &et_clean,
                                )?
                                .into()
                            } else {
                                v
                            }
                        } else {
                            v
                        };
                        let a = self.builder.build_alloca(llvm_t, name)?;
                        self.builder.build_store(a, final_v)?;
                        variables.insert(name.clone(), (a, llvm_t, et_clean));
                    } else {
                        let a = self.builder.build_alloca(inferred_vt, name)?;
                        self.builder.build_store(a, v)?;
                        variables.insert(name.clone(), (a, inferred_vt, inferred_vtn));
                    }
                    lv = None;
                }
                Statement::LetTuple { names, value, .. } => {
                    // Compile the tuple value (a pointer to an anonymous
                    // struct registered in struct_types), then extract each
                    // field into its own alloca. #53.
                    let ptr_val = self.compile_expr(value, variables, function)?;
                    let ptr = ptr_val.into_pointer_value();
                    let tn = self.get_expr_type_name(value, variables);
                    let st = self.ensure_tuple_type(&tn)?;
                    for (i, n) in names.iter().enumerate() {
                        let gep = self.builder.build_struct_gep(
                            st,
                            ptr,
                            i as u32,
                            &format!("letup_{}", i),
                        )?;
                        let elem_ty = st.get_field_type_at_index(i as u32).ok_or_else(|| {
                            CompileError::internal("tuple field type missing".to_string())
                        })?;
                        let loaded = self.builder.build_load(elem_ty, gep, "letup_ld")?;
                        let a = self.builder.build_alloca(elem_ty, n)?;
                        self.builder.build_store(a, loaded)?;
                        let elem_tn = self.get_field_type(&tn, &i.to_string());
                        variables.insert(n.clone(), (a, elem_ty, elem_tn));
                    }
                    lv = None;
                }
                Statement::Assignment { target, value, .. } => {
                    let (ptr, tt) = self.compile_lvalue(target, variables, function)?;
                    let mut v = self.compile_expr(value, variables, function)?;
                    if tt.is_struct_type() && v.get_type().is_pointer_type() {
                        v = self
                            .builder
                            .build_load(tt, v.into_pointer_value(), "ld_assign")?;
                    }
                    self.builder.build_store(ptr, v)?;
                    lv = None;
                }
                Statement::Return { value, .. } => {
                    let mut v = self.compile_expr(value, variables, function)?;
                    if self
                        .builder
                        .get_insert_block()
                        .ok_or_else(|| {
                            CompileError::internal("No active insert block".to_string())
                        })?
                        .get_terminator()
                        .is_none()
                    {
                        let rt = function.get_type().get_return_type();
                        if let Some(tt) = rt
                            && v.get_type() != tt
                        {
                            if tt.is_pointer_type() && v.is_int_value() {
                                v = self
                                    .builder
                                    .build_int_to_ptr(v.into_int_value(), pt, "ret_ptr")?
                                    .into();
                            } else if tt.is_int_type() && v.is_pointer_value() {
                                v = self
                                    .builder
                                    .build_ptr_to_int(v.into_pointer_value(), i64_t, "ret_int")?
                                    .into();
                            }
                        }
                        self.builder.build_return(Some(&v))?;
                    }
                    lv = Some(v);
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    let cv = self
                        .compile_expr(condition, variables, function)?
                        .into_int_value();
                    let comp = self.builder.build_int_compare(
                        IntPredicate::NE,
                        cv,
                        i64_t.const_int(0, false),
                        "ifcond",
                    )?;
                    let tb = self.context.append_basic_block(function, "then");
                    let eb = self.context.append_basic_block(function, "else");
                    let mb = self.context.append_basic_block(function, "ifcont");
                    self.builder.build_conditional_branch(comp, tb, eb)?;
                    let mut phis = Vec::new();

                    self.builder.position_at_end(tb);
                    let mut tv = variables.clone();
                    let tr = self.compile_block(then_branch, &mut tv, function)?;
                    let tf = self.builder.get_insert_block().ok_or_else(|| {
                        CompileError::internal("No active insert block".to_string())
                    })?;
                    if tf.get_terminator().is_none() {
                        let v = tr.unwrap_or(i64_t.const_zero().into());
                        phis.push((v, tf));
                    }

                    self.builder.position_at_end(eb);
                    let mut ev = variables.clone();
                    let er = if let Some(e) = else_branch {
                        self.compile_block(e, &mut ev, function)?
                    } else {
                        None
                    };
                    let ef = self.builder.get_insert_block().ok_or_else(|| {
                        CompileError::internal("No active insert block".to_string())
                    })?;
                    if ef.get_terminator().is_none() {
                        let v = er.unwrap_or(i64_t.const_zero().into());
                        phis.push((v, ef));
                    }

                    self.builder.position_at_end(mb);
                    if !phis.is_empty() {
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
                            self.builder.build_unconditional_branch(mb)?;
                            final_phis.push((v, b));
                        }
                        self.builder.position_at_end(mb);
                        let phi = self.builder.build_phi(target_type, "ifres")?;
                        for (v, b) in final_phis {
                            phi.add_incoming(&[(&v, b)]);
                        }
                        lv = Some(phi.as_basic_value());
                    } else {
                        if self
                            .builder
                            .get_insert_block()
                            .ok_or_else(|| {
                                CompileError::internal("No active insert block".to_string())
                            })?
                            .get_terminator()
                            .is_none()
                        {
                            self.builder.build_unreachable()?;
                        }
                        lv = None;
                    }
                }
                Statement::While {
                    condition, body, ..
                } => {
                    let cb = self.context.append_basic_block(function, "while_cond");
                    let bb = self.context.append_basic_block(function, "while_body");
                    let eb = self.context.append_basic_block(function, "while_exit");
                    self.builder.build_unconditional_branch(cb)?;
                    self.builder.position_at_end(cb);
                    let cv = self
                        .compile_expr(condition, variables, function)?
                        .into_int_value();
                    self.builder.build_conditional_branch(
                        self.builder.build_int_compare(
                            IntPredicate::NE,
                            cv,
                            i64_t.const_int(0, false),
                            "loopcond",
                        )?,
                        bb,
                        eb,
                    )?;
                    self.builder.position_at_end(bb);
                    self.loop_exit_blocks.push(eb);
                    self.loop_cond_blocks.push(cb);
                    let mut bvars = variables.clone();
                    self.compile_block(body, &mut bvars, function)?;
                    self.loop_exit_blocks.pop();
                    self.loop_cond_blocks.pop();
                    if self
                        .builder
                        .get_insert_block()
                        .ok_or_else(|| {
                            CompileError::internal("No active insert block".to_string())
                        })?
                        .get_terminator()
                        .is_none()
                    {
                        self.builder.build_unconditional_branch(cb)?;
                    }
                    self.builder.position_at_end(eb);
                    lv = None;
                }
                Statement::Break(_) => {
                    let eb = self
                        .loop_exit_blocks
                        .last()
                        .ok_or_else(|| CompileError::internal("break outside loop".to_string()))?;
                    self.builder.build_unconditional_branch(*eb)?;
                    lv = None;
                }
                Statement::Continue(_) => {
                    let cb = self.loop_cond_blocks.last().ok_or_else(|| {
                        CompileError::internal("continue outside loop".to_string())
                    })?;
                    self.builder.build_unconditional_branch(*cb)?;
                    lv = None;
                }
                Statement::Match {
                    condition, arms, ..
                } => {
                    let cv = self.compile_expr(condition, variables, function)?;
                    let exit_bb = self.context.append_basic_block(function, "matchexit");
                    let mut phis = Vec::new();
                    let ctn = self.get_expr_type_name(condition, variables);
                    let cbn = if ctn.contains('<') {
                        ctn.split('<')
                            .next()
                            .ok_or_else(|| CompileError::internal("Invalid type name".to_string()))?
                            .to_string()
                    } else {
                        ctn.clone()
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
                            let ab = self.context.append_basic_block(
                                function,
                                &format!("arm_{}_{}", i, arm.pattern),
                            );
                            let is_last = i == na - 1;
                            let nb = if is_last {
                                exit_bb
                            } else {
                                self.context.append_basic_block(function, "match_next")
                            };

                            // Get all patterns to check
                            let all_patterns: Vec<String> = if arm.patterns.is_empty() {
                                vec![arm.pattern.clone()]
                            } else {
                                arm.patterns.clone()
                            };

                            let is_default = all_patterns.iter().any(|p| p == "_");
                            let mut arm_match_cond: Option<inkwell::values::IntValue<'ctx>> = None;

                            if !is_default
                                && let Some(Declaration::Enum(e_decl)) = self.decls.get(&fen)
                            {
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
                                        Some(prev) => {
                                            self.builder.build_or(prev, cond, "arm_or")?
                                        }
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
                                let mut ptn = "i64".to_string();
                                if let Some(Declaration::Enum(e_decl)) = self.decls.get(&fen) {
                                    for v in &e_decl.variants {
                                        if arm.pattern == v.name
                                            || arm.pattern.ends_with(&format!(".{}", v.name))
                                            || arm.pattern.ends_with(&format!("::{}", v.name))
                                        {
                                            if !v.data_types.is_empty() {
                                                ptn = v.data_types[0].clone();
                                            }
                                            break;
                                        }
                                    }
                                }
                                let lt = self.aion_type_to_llvm(&ptn);
                                let cp = self.builder.build_bit_cast(
                                    dp,
                                    self.context.ptr_type(AddressSpace::default()),
                                    "arm_datacast",
                                )?;
                                let lv_val = self.builder.build_load(
                                    lt,
                                    cp.into_pointer_value(),
                                    &arm.params[0],
                                )?;
                                let pa = self.builder.build_alloca(lt, &arm.params[0])?;
                                self.builder.build_store(pa, lv_val)?;
                                av.insert(arm.params[0].clone(), (pa, lt, ptn));
                            }

                            // Evaluate guard condition if present
                            if let Some(guard_expr) = &arm.guard {
                                let guard_val = self
                                    .compile_expr(guard_expr, &av, function)?
                                    .into_int_value();
                                let guard_pass_bb =
                                    self.context.append_basic_block(function, "guard_pass");
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
                            let abf = self.builder.get_insert_block().ok_or_else(|| {
                                CompileError::internal("No active insert block".to_string())
                            })?;
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
                                &format!("arm_{}_{}", i, pattern_clean),
                            );
                            let is_last = i == na - 1;
                            let nb = if is_last {
                                exit_bb
                            } else {
                                self.context.append_basic_block(function, "match_next")
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
                                                    Some(prev) => self
                                                        .builder
                                                        .build_or(prev, range_cond, "range_or")?,
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
                                                Some(prev) => {
                                                    self.builder.build_or(prev, cond, "match_or")?
                                                }
                                                None => cond,
                                            });
                                        }
                                    }
                                } else if ctn == "String" {
                                    for pat in &all_patterns {
                                        let pattern_str =
                                            if pat.starts_with('"') && pat.ends_with('"') {
                                                pat[1..pat.len() - 1].to_string()
                                            } else {
                                                pat.clone()
                                            };

                                        let ps = self.builder.build_global_string_ptr(
                                            &pattern_str,
                                            "match_pattern",
                                        )?;
                                        let fnc = self
                                            .module
                                            .get_function("aion_str_eq")
                                            .ok_or_else(|| {
                                                CompileError::internal(
                                                    "aion_str_eq not found".to_string(),
                                                )
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
                                            Some(prev) => {
                                                self.builder.build_or(prev, cond, "match_or")?
                                            }
                                            None => cond,
                                        });
                                    }
                                }
                            }

                            if is_default && prim_match_cond.is_none() || is_binding_var {
                                self.builder.build_unconditional_branch(ab)?;
                            } else if let Some(cond) = prim_match_cond {
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

                            // For primitives, bind pattern variable if present and evaluate guard
                            let mut av = variables.clone();
                            if !arm.params.is_empty() {
                                let cv_type = cv.get_type();
                                let pa = self.builder.build_alloca(cv_type, &arm.params[0])?;
                                if ctn == "String" {
                                    let cv_ptr = cv.into_pointer_value();
                                    self.builder.build_store(pa, cv_ptr)?;
                                } else if ctn != "i64" && ctn != "Integer" {
                                    // For structs, store the pointer directly
                                    // The pointer already points to the struct data
                                    let cv_ptr = cv.into_pointer_value();
                                    self.builder.build_store(pa, cv_ptr)?;
                                    // Update type to pointer so member access works
                                    let ptr_type = self.context.ptr_type(AddressSpace::default());
                                    av.insert(
                                        arm.params[0].clone(),
                                        (pa, ptr_type.into(), format!("*{}", ctn)),
                                    );
                                } else {
                                    self.builder.build_store(pa, cv)?;
                                    av.insert(
                                        arm.params[0].clone(),
                                        (pa, cv_type, ctn.to_string()),
                                    );
                                }
                            }

                            // Evaluate guard condition if present
                            if let Some(guard_expr) = &arm.guard {
                                let guard_val = self
                                    .compile_expr(guard_expr, &av, function)?
                                    .into_int_value();
                                let guard_pass_bb =
                                    self.context.append_basic_block(function, "guard_pass");
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
                            let abf = self.builder.get_insert_block().ok_or_else(|| {
                                CompileError::internal("No active insert block".to_string())
                            })?;
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
                            lv = None;
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
                                            .build_ptr_to_int(
                                                v.into_pointer_value(),
                                                i64_t,
                                                "phi_int",
                                            )?
                                            .into();
                                    }
                                }
                                if b.get_terminator().is_none() {
                                    self.builder.build_unconditional_branch(exit_bb)?;
                                }
                                final_phis.push((v, b));
                            }
                            self.builder.position_at_end(exit_bb);
                            let phi = self.builder.build_phi(target_type, "matchres")?;
                            for (v, b) in final_phis {
                                phi.add_incoming(&[(&v, b)]);
                            }
                            lv = Some(phi.as_basic_value());
                        }
                    } else {
                        lv = None;
                    }
                }
                Statement::ExpressionStmt(e, _) => {
                    lv = Some(self.compile_expr(e, variables, function)?);
                }
                Statement::UnsafeBlock(stmts, _) | Statement::Spawn(stmts, _) => {
                    lv = self.compile_block(stmts, variables, function)?;
                }
                _ => {
                    lv = None;
                }
            }
        }
        Ok(lv)
    }
}

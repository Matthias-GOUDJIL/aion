use crate::ast::Statement;
use crate::codegen::compiler::Compiler;
use crate::error::CompileError;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
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
                    let ctn = self.get_expr_type_name(condition, variables);
                    // Shared lowering with `Expression::Match` — see
                    // `src/codegen/match_lowering.rs`. #183.
                    lv = self.lower_match(cv, &ctn, arms, variables, function, true)?;
                }
                Statement::ExpressionStmt(e, _) => {
                    lv = Some(self.compile_expr(e, variables, function)?);
                }
                Statement::UnsafeBlock(stmts, _) => {
                    lv = self.compile_block(stmts, variables, function)?;
                }
                Statement::Spawn(stmts, _) => {
                    // Lower the spawn block into a fresh void() function and
                    // hand its pointer to the runtime `aion_spawn`. The
                    // checker rejects captured locals (#176), so the body
                    // compiles against an empty variable frame (globals like
                    // argc/argv and function calls still resolve).
                    let caller_bb = self.builder.get_insert_block();
                    let spawn_name = format!("__aion_spawn_{}", self.spawn_counter);
                    self.spawn_counter += 1;
                    let spawn_fn = self.module.add_function(
                        &spawn_name,
                        self.context.void_type().fn_type(&[], false),
                        None,
                    );
                    let bb = self.context.append_basic_block(spawn_fn, "entry");
                    self.builder.position_at_end(bb);
                    let mut svars = HashMap::new();
                    self.compile_block(stmts, &mut svars, spawn_fn)?;
                    if self
                        .builder
                        .get_insert_block()
                        .ok_or_else(|| {
                            CompileError::internal("No active insert block".to_string())
                        })?
                        .get_terminator()
                        .is_none()
                    {
                        self.builder.build_return(None)?;
                    }
                    // Restore the caller's insert position before emitting
                    // the spawn call — the builder still points into the
                    // freshly generated thunk otherwise.
                    if let Some(cb) = caller_bb {
                        self.builder.position_at_end(cb);
                    }
                    let fptr = spawn_fn.as_global_value().as_pointer_value();
                    let spawn_call = self.module.get_function("aion_spawn").ok_or_else(|| {
                        CompileError::internal("aion_spawn not found".to_string())
                    })?;
                    self.builder
                        .build_call(spawn_call, &[fptr.into()], "spawn_call")?;
                    lv = None;
                }
                _ => {
                    lv = None;
                }
            }
        }
        Ok(lv)
    }
}

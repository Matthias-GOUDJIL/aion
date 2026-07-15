use crate::ast::Expression;
use crate::codegen::compiler::Compiler;
use crate::error::CompileError;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{FunctionValue, PointerValue};
use inkwell::{AddressSpace, IntPredicate};
use std::collections::HashMap;

impl<'ctx> Compiler<'ctx> {
    /// Lower an assignment target (lvalue) to an LLVM pointer + element
    /// type. Handles bare identifiers, qualified field access (`a.foo`,
    /// `rec.member`), dereference (`*p`), and array indexing with the same
    /// runtime bounds check as the read path. Phase 5 of the
    /// `compiler.rs` split (#113): moved verbatim from
    /// `src/codegen/compiler.rs` — behaviour-preserving code motion. The
    /// single `unsafe` block (in-bounds GEP) moves with the function; its
    /// scope is unchanged.
    pub(in crate::codegen) fn compile_lvalue(
        &mut self,
        e: &Expression,
        variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>,
        function: FunctionValue<'ctx>,
    ) -> Result<(PointerValue<'ctx>, BasicTypeEnum<'ctx>), CompileError> {
        let pt = self.context.ptr_type(AddressSpace::default());
        match e {
            Expression::Identifier(name, _) => {
                if let Some((vn, fnm)) = name.split_once('.')
                    && let Some((vptr, vt, vtn)) = variables.get(vn)
                {
                    let btn = if vtn.contains('<') {
                        vtn.split('<').next().ok_or_else(|| {
                            CompileError::internal("Invalid type name".to_string())
                        })?
                    } else {
                        vtn
                    };
                    let ftn = self
                        .resolve_fuzzy_name(&self.struct_types, btn)
                        .unwrap_or(btn.to_string());
                    if let Some(flds) = self.struct_fields.get(ftn.as_str())
                        && let Some(&idx) = flds.get(fnm)
                    {
                        let st = *self.struct_types.get(&ftn).ok_or_else(|| {
                            CompileError::internal(format!("LLVM struct type '{}' not found", ftn))
                        })?;
                        let st_ptr = self
                            .builder
                            .build_load(*vt, *vptr, "st_load")?
                            .into_pointer_value();
                        return Ok((
                            self.builder.build_struct_gep(st, st_ptr, idx, "fldptr")?,
                            self.aion_type_to_llvm(&self.get_field_type(vtn, fnm)),
                        ));
                    }
                }
                if let Some((ptr, vt, _)) = variables.get(name) {
                    Ok((*ptr, *vt))
                } else {
                    Err(self.err(format!("variable '{}' not found", name), e))
                }
            }
            Expression::MemberAccess {
                receiver, member, ..
            } => {
                let (rp, rt_llvm) = self.compile_lvalue(receiver, variables, function)?;
                let rtn = self.get_expr_type_name(receiver, variables);
                let ftn = self.get_field_type(&rtn, member);
                let mut bc = if rtn.contains('<') {
                    rtn.split('<')
                        .next()
                        .ok_or_else(|| CompileError::internal("Invalid type name".to_string()))?
                } else {
                    &rtn
                };
                while bc.starts_with('*') {
                    bc = &bc[1..];
                }
                let ft = self.resolve_fuzzy_name(&self.decls, bc).ok_or_else(|| {
                    CompileError::internal(format!(
                        "Struct '{}' not found (rec_type={}, receiver={:?})",
                        bc, rtn, receiver
                    ))
                })?;
                let st = *self.struct_types.get(&ft).ok_or_else(|| {
                    CompileError::internal(format!("LLVM type not found for '{}'", ft))
                })?;
                let idx = *self
                    .struct_fields
                    .get(&ft)
                    .ok_or_else(|| {
                        CompileError::internal(format!("Fields for struct '{}' not found", ft))
                    })?
                    .get(member)
                    .ok_or_else(|| {
                        CompileError::internal(format!("Field '{}' not found", member))
                    })?;

                let st_ptr = self
                    .builder
                    .build_load(rt_llvm, rp, "st_load")?
                    .into_pointer_value();
                Ok((
                    self.builder.build_struct_gep(st, st_ptr, idx, member)?,
                    self.aion_type_to_llvm(&ftn),
                ))
            }
            Expression::Deref { expr, .. } => {
                let v = self.compile_expr(expr, variables, function)?;
                let tn = self.get_expr_type_name(expr, variables);
                let et = self.aion_type_to_llvm(tn.strip_prefix('*').unwrap_or("i64"));
                let p = if v.is_int_value() {
                    self.builder
                        .build_int_to_ptr(v.into_int_value(), pt, "i2p")?
                } else {
                    v.into_pointer_value()
                };
                Ok((p, et))
            }
            Expression::Index { target, index, .. } => {
                // `arr[i] = v` lvalue with the same runtime bounds check as
                // the read path. Returns the element slot pointer. #54.
                let i64_t = self.context.i64_type();
                let arr_val = self.compile_expr(target, variables, function)?;
                let idx_val = self.compile_expr(index, variables, function)?;
                let idx = idx_val.into_int_value();
                let tn = self.get_expr_type_name(target, variables);
                let (elem_llvm, n) = self.parse_array_type_name(&tn).ok_or_else(|| {
                    CompileError::internal(format!("array type '{}' not parseable", tn))
                })?;
                let arr_ty = elem_llvm.array_type(n as u32);
                let arr_ptr = arr_val.into_pointer_value();
                let in_bounds = self.builder.build_and(
                    self.builder.build_int_compare(
                        IntPredicate::SGE,
                        idx,
                        i64_t.const_zero(),
                        "arr_lo",
                    )?,
                    self.builder.build_int_compare(
                        IntPredicate::SLT,
                        idx,
                        i64_t.const_int(n, false),
                        "arr_hi",
                    )?,
                    "arr_inb",
                )?;
                let oob_bb = self.context.append_basic_block(function, "arr_oob");
                let ok_bb = self.context.append_basic_block(function, "arr_ok");
                self.builder
                    .build_conditional_branch(in_bounds, ok_bb, oob_bb)?;
                self.builder.position_at_end(oob_bb);
                let oob_fn = self.module.get_function("aion_array_oob").ok_or_else(|| {
                    CompileError::internal("aion_array_oob not found".to_string())
                })?;
                self.builder.build_call(
                    oob_fn,
                    &[idx.into(), i64_t.const_int(n, false).into()],
                    "oob_call",
                )?;
                self.builder.build_unreachable()?;
                self.builder.position_at_end(ok_bb);
                let gep = unsafe {
                    self.builder.build_in_bounds_gep(
                        arr_ty,
                        arr_ptr,
                        &[i64_t.const_zero(), idx],
                        "arr_setp",
                    )?
                };
                Ok((gep, elem_llvm))
            }
            _ => Err(CompileError::internal(format!("Not an lvalue: {:?}", e))),
        }
    }
}

use std::path::Path;
use std::collections::HashMap;
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, PointerValue, FunctionValue, BasicValue, ValueKind};
use inkwell::types::{StructType, BasicTypeEnum, BasicType};
use inkwell::{AddressSpace, IntPredicate};
use crate::ast::*;
use crate::token::Token;
use crate::checker::TypeChecker;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub struct_types: HashMap<String, StructType<'ctx>>,
    pub enum_types: HashMap<String, StructType<'ctx>>,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
            struct_types: HashMap::new(),
            enum_types: HashMap::new(),
        }
    }

    fn aion_type_to_llvm(&self, type_name: &str) -> BasicTypeEnum<'ctx> {
        match type_name {
            "i64" => self.context.i64_type().into(),
            "f64" => self.context.f64_type().into(),
            "bool" => self.context.i64_type().into(),
            "String" => self.context.ptr_type(AddressSpace::default()).into(),
            "Date" => self.context.i64_type().into(),
            "Duration" => self.context.i64_type().into(),
            "void" => self.context.i64_type().into(),
            _ => self.context.i64_type().into(),
        }
    }

    pub fn compile(&mut self, program: &Program) -> Result<(), String> {
        // 1. Run Type Checker (Safety Pass)
        let mut checker = TypeChecker::new();
        if let Err(e) = checker.check_program(program) {
            return Err(format!("Type/Safety Error: {}", e));
        }

        let f64_type = self.context.f64_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());

        // Register built-in functions
        let printf_type = self.context.i32_type().fn_type(&[ptr_type.into()], true);
        self.module.add_function("printf", printf_type, None);

        let strlen_type = self.context.i64_type().fn_type(&[ptr_type.into()], false);
        self.module.add_function("strlen", strlen_type, None);

        let strcat_type = ptr_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);
        self.module.add_function("strcat", strcat_type, None);

        let spawn_type = self.context.void_type().fn_type(&[ptr_type.into()], false);
        self.module.add_function("aion_spawn", spawn_type, None);

        let pow_type = f64_type.fn_type(&[f64_type.into(), f64_type.into()], false);
        self.module.add_function("pow", pow_type, None);

        // std.fs support
        let read_file_type = ptr_type.fn_type(&[ptr_type.into()], false);
        self.module.add_function("aion_read_file", read_file_type, None);

        let write_file_type = self.context.i32_type().fn_type(&[ptr_type.into(), ptr_type.into()], false);
        self.module.add_function("aion_write_file", write_file_type, None);

        let get_argv_type = ptr_type.fn_type(&[ptr_type.into(), self.context.i32_type().into()], false);
        self.module.add_function("aion_get_argv_index", get_argv_type, None);

        // Register Structs
        for decl in &program.declarations {
            match decl {
                Declaration::Struct(s) => {
                    let struct_type = self.context.opaque_struct_type(&s.name);
                    self.struct_types.insert(s.name.clone(), struct_type);
                },
                Declaration::Enum(e) => {
                    // Enum represented as { i64 tag, [N x i8] data }
                    // For prototype, we use a fixed size data buffer (e.g., 64 bytes)
                    let tag_type = self.context.i64_type();
                    let data_type = self.context.i8_type().array_type(64);
                    let enum_type = self.context.struct_type(&[tag_type.into(), data_type.into()], false);
                    self.enum_types.insert(e.name.clone(), enum_type);
                },
                _ => {}
            }
        }

        // Process Functions
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                let mut param_types = Vec::new();
                for (_, p_type) in &f.params {
                    param_types.push(self.aion_type_to_llvm(p_type).into());
                }
                
                let ret_type = self.aion_type_to_llvm(&f.return_type);
                
                // If it's a known Enum, use its registered struct type
                let llvm_ret_type = if let Some(e_type) = self.enum_types.get(&f.return_type) {
                    e_type.as_basic_type_enum()
                } else {
                    ret_type
                };

                // Special case for main: always takes (i64, ptr) if defined as such
                let (fn_type, is_main) = if f.name == "main" {
                    let main_params = vec![
                        self.context.i64_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into()
                    ];
                    (self.context.i64_type().fn_type(&main_params, false), true)
                } else {
                    (llvm_ret_type.fn_type(&param_types, false), false)
                };

                let function = self.module.add_function(&f.name, fn_type, None);

                if is_main {
                    function.get_nth_param(0).unwrap().set_name("argc");
                    function.get_nth_param(1).unwrap().set_name("argv");
                } else {
                    for (i, arg) in function.get_param_iter().enumerate() {
                        arg.set_name(&f.params[i].0);
                    }
                }

                if let Some(body) = &f.body {
                    let basic_block = self.context.append_basic_block(function, "entry");
                    self.builder.position_at_end(basic_block);
                    
                    let mut local_vars = HashMap::new(); 
                    
                    if is_main {
                        let argc = function.get_nth_param(0).unwrap();
                        let argv = function.get_nth_param(1).unwrap();
                        
                        let argc_alloca = self.builder.build_alloca(self.context.i64_type(), "argc").unwrap();
                        self.builder.build_store(argc_alloca, argc).unwrap();
                        local_vars.insert("argc".to_string(), (argc_alloca, self.context.i64_type().into()));
                        
                        let argv_alloca = self.builder.build_alloca(self.context.ptr_type(AddressSpace::default()), "argv").unwrap();
                        self.builder.build_store(argv_alloca, argv).unwrap();
                        local_vars.insert("argv".to_string(), (argv_alloca, self.context.ptr_type(AddressSpace::default()).into()));
                    } else {
                        for (i, arg) in function.get_param_iter().enumerate() {
                            let arg_name = &f.params[i].0;
                            let arg_type = arg.get_type();
                            let alloca = self.builder.build_alloca(arg_type, arg_name).unwrap();
                            self.builder.build_store(alloca, arg).unwrap();
                            local_vars.insert(arg_name.clone(), (alloca, arg_type));
                        }
                    }

                    self.compile_block(body, &mut local_vars, function)?;
                    
                    if basic_block.get_terminator().is_none() {
                        self.builder.build_return(Some(&llvm_ret_type.const_zero())).unwrap();
                    }
                }
            }
        }

        Ok(())
    }

    fn compile_block(
        &self,
        body: &[Statement],
        variables: &mut HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
        function: FunctionValue<'ctx>
    ) -> Result<Option<BasicValueEnum<'ctx>>, String> {
        let mut last_val = None;
        for stmt in body {
            match stmt {
                Statement::Let { name, value, .. } => {
                    let val = self.compile_expr(value, variables, function)?;
                    let val_type = val.get_type();
                    let alloca = self.builder.build_alloca(val_type, name).unwrap();
                    self.builder.build_store(alloca, val).unwrap();
                    variables.insert(name.clone(), (alloca, val_type));
                    last_val = None;
                },
                Statement::Return { value, .. } => {
                    let val = self.compile_expr(value, variables, function)?;
                    self.builder.build_return(Some(&val)).unwrap();
                    last_val = Some(val);
                },
                Statement::If { condition, then_branch, else_branch } => {
                    let cond_val = self.compile_expr(condition, variables, function)?.into_int_value();
                    let comparison = self.builder.build_int_compare(IntPredicate::NE, cond_val, self.context.i64_type().const_int(0, false), "ifcond").unwrap();
                    let then_bb = self.context.append_basic_block(function, "then");
                    let else_bb = self.context.append_basic_block(function, "else");
                    let merge_bb = self.context.append_basic_block(function, "ifcont");
                    self.builder.build_conditional_branch(comparison, then_bb, else_bb).unwrap();
                    
                    let mut reachable = false;
                    self.builder.position_at_end(then_bb);
                    let then_val = self.compile_block(then_branch, variables, function)?;
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb).unwrap();
                        reachable = true;
                    }
                    
                    self.builder.position_at_end(else_bb);
                    let else_val = if let Some(eb) = else_branch { 
                        self.compile_block(eb, variables, function)?
                    } else { None };
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb).unwrap();
                        reachable = true;
                    }
                    
                    if reachable {
                        self.builder.position_at_end(merge_bb);
                    } else {
                        unsafe { merge_bb.delete().unwrap(); }
                    }
                    last_val = then_val.or(else_val);
                },
                Statement::Match { condition, arms } => {
                    let cond_val = self.compile_expr(condition, variables, function)?;
                    let exit_bb = self.context.append_basic_block(function, "matchexit");
                    
                    if cond_val.is_struct_value() {
                        // It's an Enum (Tagged Union)
                        let enum_val = cond_val.into_struct_value();
                        let alloca = self.builder.build_alloca(enum_val.get_type(), "matched_enum").unwrap();
                        self.builder.build_store(alloca, enum_val).unwrap();
                        
                        let tag_ptr = self.builder.build_struct_gep(enum_val.get_type(), alloca, 0, "tagptr").unwrap();
                        let tag = self.builder.build_load(self.context.i64_type(), tag_ptr, "tag").unwrap().into_int_value();
                        
                        for (i, arm) in arms.iter().enumerate() {
                            let arm_bb = self.context.append_basic_block(function, &format!("arm_{}", arm.pattern));
                            let next_bb = self.context.append_basic_block(function, "match_next");
                            
                            // Map "Ok" to 0, "Err" to 1 for prototype
                            let arm_tag = if arm.pattern == "Ok" { 0 } else if arm.pattern == "Err" { 1 } else { i as u64 };
                            
                            let is_arm = self.builder.build_int_compare(IntPredicate::EQ, tag, self.context.i64_type().const_int(arm_tag, false), "is_arm").unwrap();
                            self.builder.build_conditional_branch(is_arm, arm_bb, next_bb).unwrap();
                            
                            self.builder.position_at_end(arm_bb);
                            
                            // Extract data from enum into local variables if pattern has args
                            let mut arm_vars = variables.clone();
                            if !arm.params.is_empty() {
                                let data_ptr = self.builder.build_struct_gep(enum_val.get_type(), alloca, 1, "arm_dataptr").unwrap();
                                // For prototype, extract first arg as i64
                                let param_name = &arm.params[0];
                                let casted_data_ptr = self.builder.build_bit_cast(data_ptr, self.context.ptr_type(AddressSpace::default()), "arm_datacast").unwrap();
                                let loaded_val = self.builder.build_load(self.context.i64_type(), casted_data_ptr.into_pointer_value(), param_name).unwrap();
                                
                                let param_alloca = self.builder.build_alloca(self.context.i64_type(), param_name).unwrap();
                                self.builder.build_store(param_alloca, loaded_val).unwrap();
                                arm_vars.insert(param_name.clone(), (param_alloca, self.context.i64_type().into()));
                            }

                            self.compile_block(&arm.body, &mut arm_vars, function)?;
                            
                            // Re-check current block after compiling arm body
                            let current_bb = self.builder.get_insert_block().unwrap();
                            if current_bb.get_terminator().is_none() {
                                self.builder.build_unconditional_branch(exit_bb).unwrap();
                            }
                            
                            self.builder.position_at_end(next_bb);
                        }
                        self.builder.build_unconditional_branch(exit_bb).unwrap();
                    } else {
                        // Fallback for primitive match
                        let _val = cond_val.into_int_value();
                        for arm in arms {
                            let arm_bb = self.context.append_basic_block(function, &format!("arm_{}", arm.pattern));
                            self.builder.position_at_end(arm_bb);
                            self.compile_block(&arm.body, variables, function)?;
                            if arm_bb.get_terminator().is_none() { self.builder.build_unconditional_branch(exit_bb).unwrap(); }
                        }
                    }
                    
                    self.builder.position_at_end(exit_bb);
                    last_val = None;
                },
                Statement::ExpressionStmt(expr) => {
                    last_val = Some(self.compile_expr(expr, variables, function)?);
                },
                Statement::UnsafeBlock(body) => {
                    last_val = self.compile_block(body, variables, function)?;
                },
                _ => { last_val = None; }
            }
        }
        Ok(last_val)
    }

    fn compile_expr(
        &self,
        expr: &Expression, 
        variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
        function: FunctionValue<'ctx>
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let i64_type = self.context.i64_type();
        let f64_type = self.context.f64_type();
        match expr {
            Expression::Integer(n) => Ok(i64_type.const_int(*n as u64, false).into()),
            Expression::Float(f) => Ok(f64_type.const_float(*f).into()),
            Expression::Boolean(b) => Ok(i64_type.const_int(if *b { 1 } else { 0 }, false).into()),
            Expression::Duration(secs, nanos) => {
                let total_ms = *secs * 1000 + (*nanos as u64) / 1_000_000;
                Ok(i64_type.const_int(total_ms, false).into())
            },
            Expression::Date(ts) => {
                let total_ms = *ts * 1000;
                Ok(i64_type.const_int(total_ms as u64, false).into())
            },
            Expression::String(s) => {
                let s_with_null = format!("{}\0", s);
                let global_str = self.builder.build_global_string_ptr(&s_with_null, "aion_str").unwrap();
                Ok(global_str.as_basic_value_enum())
            },
            Expression::Identifier(name) => {
                let (ptr, var_type) = variables.get(name).ok_or_else(|| format!("Var '{}' not found", name))?;
                Ok(self.builder.build_load(*var_type, *ptr, name).unwrap())
            },
            Expression::Call { function: func_name, arguments } => {
                if func_name == "io.println" {
                    return self.compile_expr(&Expression::Intrinsic { name: "io_println".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "string.len" {
                    return self.compile_expr(&Expression::Intrinsic { name: "str_len".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "string.concat" {
                    return self.compile_expr(&Expression::Intrinsic { name: "str_concat".to_string(), arguments: arguments.clone() }, variables, function);
                }
                let fn_val = self.module.get_function(func_name).ok_or(format!("Function '{}' not found", func_name))?;
                let mut compiled_args = Vec::new();
                for arg in arguments {
                    compiled_args.push(self.compile_expr(arg, variables, function)?.into());
                }
                let call = self.builder.build_call(fn_val, &compiled_args, "calltmp").unwrap();
                match call.try_as_basic_value() {
                    ValueKind::Basic(val) => Ok(val),
                    ValueKind::Instruction(_) => {
                        // For void or aggregate returns that might not be 'Basic'
                        let ret_type = fn_val.get_type().get_return_type();
                        if let Some(t) = ret_type {
                            Ok(t.const_zero())
                        } else {
                            Ok(i64_type.const_int(0, false).into())
                        }
                    }
                }
            },
            Expression::Intrinsic { name, arguments } => {
                if name == "io_println" {
                    let printf = self.module.get_function("printf").ok_or("printf not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    let printf_arg = if arg.get_type().is_pointer_type() { arg.into() } else { arg.into() };
                    
                    let fmt_str = self.builder.build_global_string_ptr("%s\n\0", "println_fmt").unwrap();
                    self.builder.build_call(printf, &[fmt_str.as_basic_value_enum().into(), printf_arg], "printftmp").unwrap();
                    Ok(i64_type.const_int(0, false).into())
                } else if name == "str_len" {
                    let strlen = self.module.get_function("strlen").ok_or("strlen not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    let call = self.builder.build_call(strlen, &[arg.into()], "strlentmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => Ok(val),
                        ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                    }
                } else if name == "str_concat" {
                    let strcat = self.module.get_function("strcat").ok_or("strcat not found")?;
                    let arg1 = self.compile_expr(&arguments[0], variables, function)?;
                    let arg2 = self.compile_expr(&arguments[1], variables, function)?;
                    let call = self.builder.build_call(strcat, &[arg1.into(), arg2.into()], "strcattmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => Ok(val),
                        ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                    }
                } else {
                    let fn_val = self.module.get_function(name).ok_or(format!("Intrinsic '{}' not found", name))?;
                    let mut compiled_args = Vec::new();
                    for arg in arguments {
                        compiled_args.push(self.compile_expr(arg, variables, function)?.into());
                    }
                    let call = self.builder.build_call(fn_val, &compiled_args, "intrinsictmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => Ok(val),
                        ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                    }
                }
            },
            Expression::Infix { left, operator, right } => {
                let lhs = self.compile_expr(left, variables, function)?;
                let rhs = self.compile_expr(right, variables, function)?;
                
                if lhs.is_int_value() && rhs.is_int_value() {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    match operator {
                        Token::Plus => Ok(self.builder.build_int_add(l, r, "addtmp").unwrap().into()),
                        Token::Minus => Ok(self.builder.build_int_sub(l, r, "subtmp").unwrap().into()),
                        Token::Star => Ok(self.builder.build_int_mul(l, r, "multmp").unwrap().into()),
                        Token::Slash => Ok(self.builder.build_int_signed_div(l, r, "divtmp").unwrap().into()),
                        Token::Percent => Ok(self.builder.build_int_signed_rem(l, r, "remtmp").unwrap().into()),
                        
                        Token::And => Ok(self.builder.build_and(l, r, "andtmp").unwrap().into()),
                        Token::Or => Ok(self.builder.build_or(l, r, "ortmp").unwrap().into()),
                        Token::Bang => Ok(self.builder.build_xor(r, i64_type.const_int(1, false), "nottmp").unwrap().into()),
                        
                        Token::EqEq => {
                            let res = self.builder.build_int_compare(IntPredicate::EQ, l, r, "eqtmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        Token::NotEq => {
                            let res = self.builder.build_int_compare(IntPredicate::NE, l, r, "netmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        Token::Gt => {
                            let res = self.builder.build_int_compare(IntPredicate::SGT, l, r, "gttmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        Token::Lt => {
                            let res = self.builder.build_int_compare(IntPredicate::SLT, l, r, "lttmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        Token::GtEq => {
                            let res = self.builder.build_int_compare(IntPredicate::SGE, l, r, "getmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        Token::LtEq => {
                            let res = self.builder.build_int_compare(IntPredicate::SLE, l, r, "letmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        _ => Err(format!("Integer operator {:?} not supported", operator)),
                    }
                } else if lhs.is_float_value() && rhs.is_float_value() {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    match operator {
                        Token::Plus => Ok(self.builder.build_float_add(l, r, "faddtmp").unwrap().into()),
                        Token::Minus => Ok(self.builder.build_float_sub(l, r, "fsubtmp").unwrap().into()),
                        Token::Star => Ok(self.builder.build_float_mul(l, r, "fmultmp").unwrap().into()),
                        Token::Slash => Ok(self.builder.build_float_div(l, r, "fdivtmp").unwrap().into()),
                        Token::Caret => {
                            let pow = self.module.get_function("pow").unwrap();
                            let res = self.builder.build_call(pow, &[l.into(), r.into()], "powtmp").unwrap();
                            match res.try_as_basic_value() {
                                ValueKind::Basic(val) => Ok(val),
                                ValueKind::Instruction(_) => Err("pow must return a value".to_string()),
                            }
                        },
                        _ => Err(format!("Float operator {:?} not supported", operator)),
                    }
                } else {
                    Err(format!("Mismatched types for operator {:?}", operator))
                }
            },
            Expression::Block { statements, .. } => {
                let mut local_vars = variables.clone();
                let val = self.compile_block(statements, &mut local_vars, function)?;
                Ok(val.unwrap_or(i64_type.const_int(0, false).into()))
            },
            Expression::EnumInst { name, variant, arguments } => {
                let enum_type = self.enum_types.get(name).ok_or(format!("Enum '{}' not found", name))?;
                let alloca = self.builder.build_alloca(*enum_type, &format!("{}_inst", name)).unwrap();
                
                // Find variant index
                // For prototype, we search the program decls again (should be cached)
                // Let's assume tags are 0, 1, 2...
                // We'll hardcode some for now to test Result
                let tag = if variant == "Ok" { 0 } else if variant == "Err" { 1 } else { 0 };
                
                let tag_ptr = self.builder.build_struct_gep(*enum_type, alloca, 0, "tagptr").unwrap();
                self.builder.build_store(tag_ptr, self.context.i64_type().const_int(tag, false)).unwrap();
                
                if !arguments.is_empty() {
                    let data_val = self.compile_expr(&arguments[0], variables, function)?;
                    let data_ptr = self.builder.build_struct_gep(*enum_type, alloca, 1, "dataptr").unwrap();
                    // Cast data_ptr to the type of the argument for storing
                    let casted_ptr = self.builder.build_bit_cast(data_ptr, self.context.ptr_type(AddressSpace::default()), "datacast").unwrap();
                    self.builder.build_store(casted_ptr.into_pointer_value(), data_val).unwrap();
                }
                
                Ok(self.builder.build_load(*enum_type, alloca, "enumtmp").unwrap())
            },
            _ => Ok(i64_type.const_int(0, false).into()),
        }
    }

    pub fn print_to_file(&self, path: &Path) -> Result<(), String> {
        self.module.print_to_file(path).map_err(|e| e.to_string())
    }
}

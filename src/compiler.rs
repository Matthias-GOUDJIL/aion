use std::path::Path;
use std::collections::{HashMap, HashSet};
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
    pub decls: HashMap<String, Declaration>,
    pub compiled_instances: HashSet<String>,
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
            decls: HashMap::new(),
            compiled_instances: HashSet::new(),
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
            _ => {
                let base_name = if type_name.contains('<') {
                    type_name.split('<').next().unwrap()
                } else {
                    type_name
                };

                if let Some(e_type) = self.enum_types.get(base_name) {
                    return e_type.as_basic_type_enum();
                }
                if let Some(s_type) = self.struct_types.get(base_name) {
                    return s_type.as_basic_type_enum();
                }
                self.context.i64_type().into()
            },
        }
    }

    fn get_monomorphized_name(&self, base_name: &str, generic_args: &[String]) -> String {
        if generic_args.is_empty() {
            base_name.to_string()
        } else {
            format!("{}_{}", base_name, generic_args.join("_"))
        }
    }

    fn substitute_types_in_body(&self, body: &mut [Statement], placeholders: &[String], concrete: &[String]) {
        for stmt in body.iter_mut() {
            match stmt {
                Statement::Let { value, .. } => self.substitute_types_in_expr(value, placeholders, concrete),
                Statement::Return { value, .. } => self.substitute_types_in_expr(value, placeholders, concrete),
                Statement::ExpressionStmt(expr) => self.substitute_types_in_expr(expr, placeholders, concrete),
                Statement::If { condition, then_branch, else_branch } => {
                    self.substitute_types_in_expr(condition, placeholders, concrete);
                    self.substitute_types_in_body(then_branch, placeholders, concrete);
                    if let Some(eb) = else_branch { self.substitute_types_in_body(eb, placeholders, concrete); }
                },
                Statement::For { range, body, .. } => {
                    self.substitute_types_in_expr(range, placeholders, concrete);
                    self.substitute_types_in_body(body, placeholders, concrete);
                },
                Statement::UnsafeBlock(stmts) => self.substitute_types_in_body(stmts, placeholders, concrete),
                Statement::Spawn(stmts) => self.substitute_types_in_body(stmts, placeholders, concrete),
                Statement::Match { condition, arms } => {
                    self.substitute_types_in_expr(condition, placeholders, concrete);
                    for arm in arms { self.substitute_types_in_body(&mut arm.body, placeholders, concrete); }
                },
            }
        }
    }

    fn substitute_types_in_expr(&self, expr: &mut Expression, placeholders: &[String], concrete: &[String]) {
        match expr {
            Expression::Infix { left, right, .. } => {
                self.substitute_types_in_expr(left, placeholders, concrete);
                self.substitute_types_in_expr(right, placeholders, concrete);
            },
            Expression::Call { generic_args, arguments, .. } => {
                for arg in generic_args.iter_mut() {
                    for i in 0..placeholders.len() {
                        if arg == &placeholders[i] { *arg = concrete[i].clone(); }
                    }
                }
                for arg in arguments { self.substitute_types_in_expr(arg, placeholders, concrete); }
            },
            Expression::EnumInst { generic_args, arguments, .. } => {
                for arg in generic_args.iter_mut() {
                    for i in 0..placeholders.len() {
                        if arg == &placeholders[i] { *arg = concrete[i].clone(); }
                    }
                }
                for arg in arguments { self.substitute_types_in_expr(arg, placeholders, concrete); }
            },
            Expression::StructInst { generic_args, fields, .. } => {
                for arg in generic_args.iter_mut() {
                    for i in 0..placeholders.len() {
                        if arg == &placeholders[i] { *arg = concrete[i].clone(); }
                    }
                }
                for (_, val) in fields { self.substitute_types_in_expr(val, placeholders, concrete); }
            },
            Expression::Intrinsic { arguments, .. } => {
                for arg in arguments { self.substitute_types_in_expr(arg, placeholders, concrete); }
            },
            Expression::Range { start, end } => {
                self.substitute_types_in_expr(start, placeholders, concrete);
                self.substitute_types_in_expr(end, placeholders, concrete);
            },
            Expression::Block { statements, .. } => self.substitute_types_in_body(statements, placeholders, concrete),
            _ => {}
        }
    }

    fn compile_function(&mut self, decl: &Declaration) -> Result<FunctionValue<'ctx>, String> {
        if let Declaration::Function(f) = decl {
            let function = if let Some(existing) = self.module.get_function(&f.name) {
                if existing.get_first_basic_block().is_some() { return Ok(existing); }
                existing
            } else {
                let mut param_types = Vec::new();
                if f.name == "main" {
                    param_types.push(self.context.i32_type().into());
                    param_types.push(self.context.ptr_type(AddressSpace::default()).into());
                } else {
                    for (_, p_type) in &f.params {
                        param_types.push(self.aion_type_to_llvm(p_type).into());
                    }
                }
                let llvm_ret_type = self.aion_type_to_llvm(&f.return_type);
                let fn_type = llvm_ret_type.fn_type(&param_types, false);
                self.module.add_function(&f.name, fn_type, None)
            };

            if let Some(body) = &f.body {
                let prev_block = self.builder.get_insert_block();
                let basic_block = self.context.append_basic_block(function, "entry");
                self.builder.position_at_end(basic_block);
                
                let mut local_vars = HashMap::new(); 
                if f.name == "main" {
                    if let Some(argc) = function.get_nth_param(0) {
                        argc.set_name("argc");
                        let argc_val = self.builder.build_int_z_extend(argc.into_int_value(), self.context.i64_type(), "argc_ext").unwrap();
                        let alloca = self.builder.build_alloca(self.context.i64_type(), "argc").unwrap();
                        self.builder.build_store(alloca, argc_val).unwrap();
                        local_vars.insert("argc".to_string(), (alloca, self.context.i64_type().into()));
                        
                        // Store to global
                        if let Some(global) = self.module.get_global("aion_argc") {
                            self.builder.build_store(global.as_pointer_value(), argc_val).unwrap();
                        }
                    }
                    if let Some(argv) = function.get_nth_param(1) {
                        argv.set_name("argv");
                        let alloca = self.builder.build_alloca(self.context.ptr_type(AddressSpace::default()), "argv").unwrap();
                        self.builder.build_store(alloca, argv).unwrap();
                        local_vars.insert("argv".to_string(), (alloca, self.context.ptr_type(AddressSpace::default()).into()));
                        
                        // Store to global
                        if let Some(global) = self.module.get_global("aion_argv") {
                            self.builder.build_store(global.as_pointer_value(), argv).unwrap();
                        }
                    }
                } else {
                    for (i, arg) in function.get_param_iter().enumerate() {
                        if i < f.params.len() {
                            let arg_name = &f.params[i].0;
                            arg.set_name(arg_name);
                            let alloca = self.builder.build_alloca(arg.get_type(), arg_name).unwrap();
                            self.builder.build_store(alloca, arg).unwrap();
                            local_vars.insert(arg_name.clone(), (alloca, arg.get_type()));
                        }
                    }
                }

                self.compile_block(body, &mut local_vars, function)?;
                if let Some(current_block) = self.builder.get_insert_block() {
                    if current_block.get_terminator().is_none() {
                        let ret_type = function.get_type().get_return_type().map(|t| t.as_basic_type_enum()).unwrap_or(self.context.i64_type().into());
                        self.builder.build_return(Some(&ret_type.const_zero())).unwrap();
                    }
                }
                if let Some(prev) = prev_block { self.builder.position_at_end(prev); }
            }
            Ok(function)
        } else {
            Err("Not a function".to_string())
        }
    }

    pub fn compile(&mut self, program: &Program) -> Result<(), String> {
        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => { self.decls.insert(f.name.clone(), decl.clone()); },
                Declaration::Struct(s) => { self.decls.insert(s.name.clone(), decl.clone()); },
                Declaration::Enum(e) => { self.decls.insert(e.name.clone(), decl.clone()); },
                _ => {}
            }
        }

        let mut checker = TypeChecker::new();
        if let Err(e) = checker.check_program(program) {
            return Err(format!("Type/Safety Error: {}", e));
        }

        let ptr_type = self.context.ptr_type(AddressSpace::default());
        self.module.add_function("printf", self.context.i32_type().fn_type(&[ptr_type.into()], true), None);
        self.module.add_function("strlen", self.context.i64_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("strcat", ptr_type.fn_type(&[ptr_type.into(), ptr_type.into()], false), None);
        self.module.add_function("aion_spawn", self.context.void_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("pow", self.context.f64_type().fn_type(&[self.context.f64_type().into(), self.context.f64_type().into()], false), None);
        self.module.add_function("aion_read_file", ptr_type.fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("aion_write_file", self.context.i32_type().fn_type(&[ptr_type.into(), ptr_type.into()], false), None);
        self.module.add_function("aion_fs_exists", self.context.i64_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("aion_getenv", ptr_type.fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("aion_get_argv_index", ptr_type.fn_type(&[ptr_type.into(), self.context.i32_type().into()], false), None);
        
        // Add global argc/argv
        let argc_global = self.module.add_global(self.context.i64_type(), Some(AddressSpace::default()), "aion_argc");
        argc_global.set_initializer(&self.context.i64_type().const_zero());
        
        let argv_global = self.module.add_global(self.context.ptr_type(AddressSpace::default()), Some(AddressSpace::default()), "aion_argv");
        argv_global.set_initializer(&self.context.ptr_type(AddressSpace::default()).const_null());

        // Added for NULL check
        // Note: mem_is_null is handled as pure LLVM IR generation, no C runtime needed for pointer diff.
        // But if we want runtime function:
        // self.module.add_function("aion_mem_is_null", ...);

        for decl in &program.declarations {
            match decl {
                Declaration::Struct(s) => {
                    let struct_type = self.context.opaque_struct_type(&s.name);
                    self.struct_types.insert(s.name.clone(), struct_type);
                },
                Declaration::Enum(e) => {
                    let enum_type = self.context.struct_type(&[self.context.i64_type().into(), self.context.i8_type().array_type(64).into()], false);
                    self.enum_types.insert(e.name.clone(), enum_type);
                },
                _ => {}
            }
        }

        // Manually register Option enum if missing (for stdlib mocking)
        if !self.enum_types.contains_key("Option") {
             let enum_type = self.context.struct_type(&[self.context.i64_type().into(), self.context.i8_type().array_type(64).into()], false);
             self.enum_types.insert("Option".to_string(), enum_type);
        }

        // Compile all non-generic functions
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                if f.generic_params.is_empty() { self.compile_function(decl)?; }
            }
        }

        Ok(())
    }

    fn compile_block(
        &mut self,
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
                    
                    if reachable { self.builder.position_at_end(merge_bb); }
                    else { unsafe { merge_bb.delete().unwrap(); } }
                    last_val = then_val.or(else_val);
                },
                Statement::Match { condition, arms } => {
                    let cond_val = self.compile_expr(condition, variables, function)?;
                    // eprintln!("DEBUG: Match condition type: {:?}", cond_val.get_type());
                    
                    let exit_bb = self.context.append_basic_block(function, "matchexit");
                    
                    if cond_val.is_struct_value() {
                        let enum_val = cond_val.into_struct_value();
                        let alloca = self.builder.build_alloca(enum_val.get_type(), "matched_enum").unwrap();
                        self.builder.build_store(alloca, enum_val).unwrap();
                        let tag_ptr = self.builder.build_struct_gep(enum_val.get_type(), alloca, 0, "tagptr").unwrap();
                        let tag = self.builder.build_load(self.context.i64_type(), tag_ptr, "tag").unwrap().into_int_value();
                        
                        for (i, arm) in arms.iter().enumerate() {
                            // Fix: Use arm.pattern for uniqueness? 
                            // If pattern is same (e.g. literals), might conflict block names?
                            let arm_bb_name = format!("arm_{}_{}", arm.pattern, i);
                            let arm_bb = self.context.append_basic_block(function, &arm_bb_name);
                            let next_bb = self.context.append_basic_block(function, "match_next");
                            
                            // Determine tag value for pattern
                            // Default to pattern index if not Ok/Err
                            // This is fragile. Ideally we resolve Enum Variant Tag from TypeChecker info.
                            let arm_tag = if arm.pattern == "Some" || arm.pattern == "Ok" { 0 } 
                                     else if arm.pattern == "None" || arm.pattern == "Err" { 1 } 
                                     else { i as u64 };
                                     
                            let is_arm = self.builder.build_int_compare(IntPredicate::EQ, tag, self.context.i64_type().const_int(arm_tag, false), "is_arm").unwrap();
                            self.builder.build_conditional_branch(is_arm, arm_bb, next_bb).unwrap();
                            
                            self.builder.position_at_end(arm_bb);
                            let mut arm_vars = variables.clone();
                            if !arm.params.is_empty() {
                                let data_ptr = self.builder.build_struct_gep(enum_val.get_type(), alloca, 1, "arm_dataptr").unwrap();
                                let param_name = &arm.params[0];
                                
                                // Heuristic for prototype: Some/Ok payloads are often Strings (pointers)
                                let (load_type, cast_type) = if arm.pattern == "Some" || arm.pattern == "Ok" {
                                    let ptr_t = self.context.ptr_type(AddressSpace::default());
                                    (ptr_t.into(), ptr_t.ptr_type(AddressSpace::default()))
                                } else {
                                    let i64_t = self.context.i64_type();
                                    (i64_t.into(), i64_t.ptr_type(AddressSpace::default()))
                                };

                                let casted_ptr = self.builder.build_bit_cast(data_ptr, cast_type, "arm_datacast").unwrap();
                                let loaded_val = self.builder.build_load(load_type, casted_ptr.into_pointer_value(), param_name).unwrap();
                                let param_alloca = self.builder.build_alloca(load_type, param_name).unwrap();
                                self.builder.build_store(param_alloca, loaded_val).unwrap();
                                arm_vars.insert(param_name.clone(), (param_alloca, load_type));
                                eprintln!("DEBUG: Bound param '{}' with type {:?}", param_name, load_type);
                            }
                            self.compile_block(&arm.body, &mut arm_vars, function)?;
                            if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                                self.builder.build_unconditional_branch(exit_bb).unwrap();
                            }
                            self.builder.position_at_end(next_bb);
                        }
                        self.builder.build_unconditional_branch(exit_bb).unwrap();
                    } else {
                        for arm in arms {
                            let arm_bb = self.context.append_basic_block(function, &format!("arm_{}", arm.pattern));
                            self.builder.position_at_end(arm_bb);
                            self.compile_block(&arm.body, variables, function)?;
                            if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                                self.builder.build_unconditional_branch(exit_bb).unwrap();
                            }
                        }
                    }
                    self.builder.position_at_end(exit_bb);
                    last_val = None;
                },
                Statement::ExpressionStmt(expr) => { last_val = Some(self.compile_expr(expr, variables, function)?); },
                Statement::UnsafeBlock(body) => { last_val = self.compile_block(body, variables, function)?; },
                _ => { last_val = None; }
            }
        }
        Ok(last_val)
    }

    fn compile_expr(
        &mut self,
        expr: &Expression, 
        variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
        function: FunctionValue<'ctx>
    ) -> Result<BasicValueEnum<'ctx>, String> {
        // eprintln!("DEBUG: Compiling {:?}", expr);
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
                let var = variables.get(name);
                if let Some((ptr, var_type)) = var {
                    Ok(self.builder.build_load(*var_type, *ptr, name).unwrap())
                } else {
                    // Check globals
                    if name == "argc" {
                        if let Some(global) = self.module.get_global("aion_argc") {
                            return Ok(self.builder.build_load(self.context.i64_type(), global.as_pointer_value(), "argc_load").unwrap());
                        }
                    } else if name == "argv" {
                        if let Some(global) = self.module.get_global("aion_argv") {
                            return Ok(self.builder.build_load(self.context.ptr_type(AddressSpace::default()), global.as_pointer_value(), "argv_load").unwrap());
                        }
                    }
                    eprintln!("DEBUG: Var '{}' not found. Available: {:?}", name, variables.keys());
                    Err(format!("Var '{}' not found", name))
                }
            },
            Expression::Call { function: func_name, generic_args, arguments } => {
                if func_name == "io.println" {
                    return self.compile_expr(&Expression::Intrinsic { name: "io_println".to_string(), arguments: arguments.clone() }, variables, function);
                }
                // Intercept env.var to return Option<String>
                if func_name == "env.var" {
                    let getenv_fn = self.module.get_function("aion_getenv").ok_or("aion_getenv not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    let call = self.builder.build_call(getenv_fn, &[arg.into()], "getenvtmp").unwrap();
                    
                    let ptr_val = match call.try_as_basic_value() {
                        ValueKind::Basic(val) => val,
                        ValueKind::Instruction(_) => return Err("getenv returned instruction value".to_string()),
                    };
                    
                    let option_type = *self.enum_types.get("Option").ok_or("Option type not found")?;
                    let alloca = self.builder.build_alloca(option_type, "env_var_res").unwrap();
                    
                    let ptr_int = self.builder.build_ptr_to_int(ptr_val.into_pointer_value(), self.context.i64_type(), "ptrtoint").unwrap();
                    let is_null = self.builder.build_int_compare(IntPredicate::EQ, ptr_int, self.context.i64_type().const_zero(), "isnull").unwrap();
                    
                    let then_bb = self.context.append_basic_block(function, "is_null");
                    let else_bb = self.context.append_basic_block(function, "not_null");
                    let merge_bb = self.context.append_basic_block(function, "merge");
                    self.builder.build_conditional_branch(is_null, then_bb, else_bb).unwrap();
                    
                    self.builder.position_at_end(then_bb);
                    let tag_ptr = self.builder.build_struct_gep(option_type, alloca, 0, "tagptr_none").unwrap();
                    self.builder.build_store(tag_ptr, self.context.i64_type().const_int(1, false)).unwrap();
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                    
                    self.builder.position_at_end(else_bb);
                    let tag_ptr = self.builder.build_struct_gep(option_type, alloca, 0, "tagptr_some").unwrap();
                    self.builder.build_store(tag_ptr, self.context.i64_type().const_int(0, false)).unwrap();
                    let data_ptr = self.builder.build_struct_gep(option_type, alloca, 1, "dataptr_some").unwrap();
                    let casted_ptr_ptr = self.builder.build_bit_cast(data_ptr, self.context.ptr_type(AddressSpace::default()), "cast_to_ptrptr").unwrap();
                    self.builder.build_store(casted_ptr_ptr.into_pointer_value(), ptr_val).unwrap();
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                    
                    self.builder.position_at_end(merge_bb);
                    return Ok(self.builder.build_load(option_type, alloca, "option_res").unwrap());
                }
                if func_name == "fs.read_to_string" {
                    return self.compile_expr(&Expression::Intrinsic { name: "fs_read_to_string".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "fs.write" {
                    return self.compile_expr(&Expression::Intrinsic { name: "fs_write".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "fs.exists" {
                    return self.compile_expr(&Expression::Intrinsic { name: "fs_exists".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "string.len" {
                    return self.compile_expr(&Expression::Intrinsic { name: "str_len".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "string.concat" {
                    return self.compile_expr(&Expression::Intrinsic { name: "str_concat".to_string(), arguments: arguments.clone() }, variables, function);
                }

                let actual_func_name = if !generic_args.is_empty() {
                    let name = self.get_monomorphized_name(func_name, generic_args);
                    if !self.compiled_instances.contains(&name) { 
                        // Trigger monomorphization
                        let decl = self.decls.get(func_name).cloned().ok_or(format!("Generic function '{}' not found", func_name))?;
                        if let Declaration::Function(mut f) = decl {
                            self.compiled_instances.insert(name.clone());
                            let placeholders = f.generic_params.clone();
                            for i in 0..placeholders.len() {
                                let p = &placeholders[i];
                                let c = &generic_args[i];
                                for (_, pt) in f.params.iter_mut() { if pt == p { *pt = c.clone(); } }
                                if &f.return_type == p { f.return_type = c.clone(); }
                            }
                            if let Some(body) = &mut f.body { self.substitute_types_in_body(body, &placeholders, generic_args); }
                            f.name = name.clone();
                            self.compile_function(&Declaration::Function(f))?;
                        }
                    }
                    name
                } else { func_name.clone() };

                let fn_val = self.module.get_function(&actual_func_name).ok_or(format!("Function '{}' not found", actual_func_name))?;
                let mut compiled_args = Vec::new();
                for arg in arguments { compiled_args.push(self.compile_expr(arg, variables, function)?.into()); }
                let call = self.builder.build_call(fn_val, &compiled_args, "calltmp").unwrap();
                match call.try_as_basic_value() {
                    ValueKind::Basic(val) => Ok(val),
                    ValueKind::Instruction(_) => {
                        let ret_type = fn_val.get_type().get_return_type();
                        if let Some(t) = ret_type { Ok(t.const_zero()) } else { Ok(i64_type.const_int(0, false).into()) }
                    }
                }
            },
            Expression::Intrinsic { name, arguments } => {
                if name == "io_println" {
                    let printf = self.module.get_function("printf").ok_or("printf not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    
                    let fmt = if arg.get_type().is_pointer_type() {
                        "%s\n\0"
                    } else {
                        "%lld\n\0"
                    };
                    
                    let fmt_str = self.builder.build_global_string_ptr(fmt, "println_fmt").unwrap();
                    self.builder.build_call(printf, &[fmt_str.as_basic_value_enum().into(), arg.into()], "printftmp").unwrap();
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
                } else if name == "fs_read_to_string" {
                    let read_fn = self.module.get_function("aion_read_file").ok_or("aion_read_file not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    let call = self.builder.build_call(read_fn, &[arg.into()], "readtmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => Ok(val),
                        ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                    }
                } else if name == "fs_write" {
                    let write_fn = self.module.get_function("aion_write_file").ok_or("aion_write_file not found")?;
                    let arg1 = self.compile_expr(&arguments[0], variables, function)?;
                    let arg2 = self.compile_expr(&arguments[1], variables, function)?;
                    let call = self.builder.build_call(write_fn, &[arg1.into(), arg2.into()], "writetmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => {
                            // Cast i32 result to i64
                            if val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 32 {
                                Ok(self.builder.build_int_s_extend(val.into_int_value(), self.context.i64_type(), "i32toi64").unwrap().into())
                            } else {
                                Ok(val)
                            }
                        },
                        ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                    }
                } else if name == "fs_exists" {
                    let exists_fn = self.module.get_function("aion_fs_exists").ok_or("aion_fs_exists not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    let call = self.builder.build_call(exists_fn, &[arg.into()], "existstmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => {
                            // Cast i32 result to i64 (bool)
                            if val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 32 {
                                Ok(self.builder.build_int_z_extend(val.into_int_value(), self.context.i64_type(), "boolcast").unwrap().into())
                            } else {
                                Ok(val)
                            }
                        },
                        ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                    }
                } else if name == "env_var" {
                    let getenv_fn = self.module.get_function("aion_getenv").ok_or("aion_getenv not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    let call = self.builder.build_call(getenv_fn, &[arg.into()], "getenvtmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => Ok(val),
                        ValueKind::Instruction(_) => Ok(self.context.ptr_type(AddressSpace::default()).const_null().into()),
                    }
                } else if name == "mem_is_null" {
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    if arg.is_pointer_value() {
                        let ptr = arg.into_pointer_value();
                        let ptr_int = self.builder.build_ptr_to_int(ptr, self.context.i64_type(), "ptrtoint").unwrap();
                        let eq_zero = self.builder.build_int_compare(IntPredicate::EQ, ptr_int, self.context.i64_type().const_zero(), "eqzero").unwrap();
                        Ok(self.builder.build_int_z_extend(eq_zero, self.context.i64_type(), "boolcast").unwrap().into())
                    } else {
                        Ok(i64_type.const_int(0, false).into())
                    }
                } else {
                    let fn_val = self.module.get_function(name).ok_or(format!("Intrinsic '{}' not found", name))?;
                    let mut compiled_args = Vec::new();
                    for arg in arguments { compiled_args.push(self.compile_expr(arg, variables, function)?.into()); }
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
                } else { Err(format!("Mismatched types for operator {:?}", operator)) }
            },
            Expression::Block { statements, .. } => {
                let mut local_vars = variables.clone();
                let val = self.compile_block(statements, &mut local_vars, function)?;
                Ok(val.unwrap_or(i64_type.const_int(0, false).into()))
            },
            Expression::EnumInst { name, variant, generic_args: _, arguments } => {
                let enum_type = *self.enum_types.get(name).ok_or(format!("Enum '{}' not found", name))?;
                let alloca = self.builder.build_alloca(enum_type, &format!("{}_inst", name)).unwrap();
                let tag = if variant == "Ok" { 0 } else if variant == "Err" { 1 } else { 0 };
                let tag_ptr = self.builder.build_struct_gep(enum_type, alloca, 0, "tagptr").unwrap();
                self.builder.build_store(tag_ptr, self.context.i64_type().const_int(tag, false)).unwrap();
                if !arguments.is_empty() {
                    let data_val = self.compile_expr(&arguments[0], variables, function)?;
                    let data_ptr = self.builder.build_struct_gep(enum_type, alloca, 1, "dataptr").unwrap();
                    let casted_ptr = self.builder.build_bit_cast(data_ptr, self.context.ptr_type(AddressSpace::default()), "datacast").unwrap();
                    self.builder.build_store(casted_ptr.into_pointer_value(), data_val).unwrap();
                }
                Ok(self.builder.build_load(enum_type, alloca, "enumtmp").unwrap())
            },
            _ => Ok(i64_type.const_int(0, false).into()),
        }
    }

    pub fn print_to_file(&self, path: &Path) -> Result<(), String> {
        self.module.print_to_file(path).map_err(|e| e.to_string())
    }
}

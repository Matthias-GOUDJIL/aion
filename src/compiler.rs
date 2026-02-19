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
    pub struct_fields: HashMap<String, HashMap<String, u32>>,
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
            struct_fields: HashMap::new(),
            enum_types: HashMap::new(),
            decls: HashMap::new(),
            compiled_instances: HashSet::new(),
        }
    }

    fn resolve_fuzzy_name<T>(&self, map: &HashMap<String, T>, name: &str) -> Option<String> {
        if map.contains_key(name) { return Some(name.to_string()); }
        for key in map.keys() {
            if key.ends_with(name) && (key.len() == name.len() || key.as_bytes()[key.len() - name.len() - 1] == b'.') {
                return Some(key.clone());
            }
        }
        None
    }

    fn aion_type_to_llvm(&self, type_name: &str) -> BasicTypeEnum<'ctx> {
        if type_name.starts_with('*') {
            return self.context.ptr_type(AddressSpace::default()).into();
        }
        match type_name {
            "i64" => self.context.i64_type().into(),
            "f64" => self.context.f64_type().into(),
            "bool" => self.context.i64_type().into(),
            "String" => self.context.ptr_type(AddressSpace::default()).into(),
            "Date" => self.context.i64_type().into(),
            "Duration" => self.context.i64_type().into(),
            "void" | "Unit" => self.context.i64_type().into(),
            _ => {
                let base_name = if type_name.contains('<') {
                    type_name.split('<').next().unwrap()
                } else {
                    type_name
                };

                if let Some(full_name) = self.resolve_fuzzy_name(&self.enum_types, base_name) {
                    return self.enum_types.get(&full_name).unwrap().as_basic_type_enum();
                }
                if let Some(full_name) = self.resolve_fuzzy_name(&self.struct_types, base_name) {
                    return self.struct_types.get(&full_name).unwrap().as_basic_type_enum();
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
                Statement::Assignment { target, value } => {
                    self.substitute_types_in_expr(target, placeholders, concrete);
                    self.substitute_types_in_expr(value, placeholders, concrete);
                },
                Statement::Return { value, .. } => self.substitute_types_in_expr(value, placeholders, concrete),
                Statement::ExpressionStmt(expr) => self.substitute_types_in_expr(expr, placeholders, concrete),
                Statement::If { condition, then_branch, else_branch } => {
                    self.substitute_types_in_expr(condition, placeholders, concrete);
                    self.substitute_types_in_body(then_branch, placeholders, concrete);
                    if let Some(eb) = else_branch { self.substitute_types_in_body(eb, placeholders, concrete); }
                },
                Statement::While { condition, body } => {
                    self.substitute_types_in_expr(condition, placeholders, concrete);
                    self.substitute_types_in_body(body, placeholders, concrete);
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
                Statement::NoOp => {},
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
            Expression::If { condition, then_branch, else_branch } => {
                self.substitute_types_in_expr(condition, placeholders, concrete);
                self.substitute_types_in_body(then_branch, placeholders, concrete);
                if let Some(eb) = else_branch { self.substitute_types_in_body(eb, placeholders, concrete); }
            },
            Expression::Cast { expr, target } => {
                self.substitute_types_in_expr(expr, placeholders, concrete);
                for i in 0..placeholders.len() {
                    if target == &placeholders[i] { *target = concrete[i].clone(); }
                }
            },
            Expression::Deref { expr } => self.substitute_types_in_expr(expr, placeholders, concrete),
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
                // This branch should ideally not be hit after Pass 1
                let mut param_types = Vec::new();
                if f.name == "main" {
                    param_types.push(self.context.i32_type().into());
                    param_types.push(self.context.ptr_type(AddressSpace::default()).into());
                } else {
                    for (p_name, p_type) in &f.params {
                        let mut llvm_p_type = self.aion_type_to_llvm(p_type);
                        if p_name == "self" {
                            let base_type_name = if p_type.contains('<') { p_type.split('<').next().unwrap() } else { p_type };
                            if self.struct_types.contains_key(base_type_name) {
                                llvm_p_type = self.context.ptr_type(AddressSpace::default()).into();
                            }
                        }
                        param_types.push(llvm_p_type.into());
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
                        local_vars.insert("argc".to_string(), (alloca, self.context.i64_type().into(), "i64".to_string()));
                        
                        if let Some(global) = self.module.get_global("aion_argc") {
                            self.builder.build_store(global.as_pointer_value(), argc_val).unwrap();
                        }
                    }
                    if let Some(argv) = function.get_nth_param(1) {
                        argv.set_name("argv");
                        let alloca = self.builder.build_alloca(self.context.ptr_type(AddressSpace::default()), "argv").unwrap();
                        self.builder.build_store(alloca, argv).unwrap();
                        local_vars.insert("argv".to_string(), (alloca, self.context.ptr_type(AddressSpace::default()).into(), "ptr".to_string()));
                        
                        if let Some(global) = self.module.get_global("aion_argv") {
                            self.builder.build_store(global.as_pointer_value(), argv).unwrap();
                        }
                    }
                } else {
                    for (i, arg) in function.get_param_iter().enumerate() {
                        if i < f.params.len() {
                            let arg_name = &f.params[i].0;
                            let arg_type_name = &f.params[i].1;
                            arg.set_name(arg_name);

                            let base_type_name = if arg_type_name.contains('<') { arg_type_name.split('<').next().unwrap() } else { arg_type_name };
                            if arg_name == "self" && self.struct_types.contains_key(base_type_name) {
                                let struct_type = *self.struct_types.get(base_type_name).unwrap();
                                local_vars.insert(arg_name.clone(), (arg.into_pointer_value(), struct_type.as_basic_type_enum(), arg_type_name.clone()));
                            } else {
                                let alloca = self.builder.build_alloca(arg.get_type(), arg_name).unwrap();
                                self.builder.build_store(alloca, arg).unwrap();
                                local_vars.insert(arg_name.clone(), (alloca, arg.get_type(), arg_type_name.clone()));
                            }
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
                Declaration::Impl(i) => {
                    for f in &i.functions {
                        let mut new_f = f.clone();
                        new_f.name = format!("{}.{}", i.target_name, f.name);
                        
                        for (_, p_type) in new_f.params.iter_mut() { 
                            if p_type == "Self" { *p_type = i.target_name.clone(); } 
                        }
                        if new_f.return_type == "Self" { new_f.return_type = i.target_name.clone(); }

                        let mut combined = i.generic_params.clone();
                        combined.extend(f.generic_params.clone());
                        new_f.generic_params = combined;
                        self.decls.insert(new_f.name.clone(), Declaration::Function(new_f));
                    }
                },
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
        
        let argc_global = self.module.add_global(self.context.i64_type(), Some(AddressSpace::default()), "aion_argc");
        argc_global.set_initializer(&self.context.i64_type().const_zero());
        
        let argv_global = self.module.add_global(self.context.ptr_type(AddressSpace::default()), Some(AddressSpace::default()), "aion_argv");
        argv_global.set_initializer(&self.context.ptr_type(AddressSpace::default()).const_null());

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

        for decl in &program.declarations {
            if let Declaration::Struct(s) = decl {
                if s.generic_params.is_empty() {
                    let struct_type = *self.struct_types.get(&s.name).unwrap();
                    let mut field_types = Vec::new();
                    let mut field_map = HashMap::new();
                    for (i, (name, type_name)) in s.fields.iter().enumerate() {
                        field_types.push(self.aion_type_to_llvm(type_name));
                        field_map.insert(name.clone(), i as u32);
                    }
                    struct_type.set_body(&field_types, false);
                    self.struct_fields.insert(s.name.clone(), field_map);
                }
            }
        }

        if !self.enum_types.contains_key("Option") {
             let enum_type = self.context.struct_type(&[self.context.i64_type().into(), self.context.i8_type().array_type(64).into()], false);
             self.enum_types.insert("Option".to_string(), enum_type);
        }

        let all_decls: Vec<Declaration> = self.decls.values().cloned().collect();
        
        for decl in &all_decls {
            if let Declaration::Function(f) = decl {
                if f.generic_params.is_empty() {
                    let mut param_types = Vec::new();
                    if f.name == "main" {
                        param_types.push(self.context.i32_type().into());
                        param_types.push(self.context.ptr_type(AddressSpace::default()).into());
                    } else {
                        for (p_name, p_type) in &f.params {
                            let mut llvm_p_type = self.aion_type_to_llvm(p_type);
                            if p_name == "self" {
                                let base_type_name = if p_type.contains('<') { p_type.split('<').next().unwrap() } else { p_type };
                                if self.struct_types.contains_key(base_type_name) {
                                    llvm_p_type = self.context.ptr_type(AddressSpace::default()).into();
                                }
                            }
                            param_types.push(llvm_p_type.into());
                        }
                    }
                    let mut llvm_name = f.name.clone();
                    for (attr_name, attr_val) in &f.attributes {
                        if attr_name == "intrinsic" {
                            llvm_name = attr_val.replace("libc.", "");
                            break;
                        }
                    }

                    let llvm_ret_type = self.aion_type_to_llvm(&f.return_type);
                    let fn_type = llvm_ret_type.fn_type(&param_types, false);
                    self.module.add_function(&llvm_name, fn_type, None);
                }
            }
        }

        for decl in &all_decls {
            if let Declaration::Function(f) = decl {
                if f.generic_params.is_empty() { 
                    self.compile_function(decl)?; 
                }
            }
        }

        Ok(())
    }

    fn compile_block(
        &mut self,
        body: &[Statement],
        variables: &mut HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>,
        function: FunctionValue<'ctx>
    ) -> Result<Option<BasicValueEnum<'ctx>>, String> {
        let mut last_val = None;
        for stmt in body {
            match stmt {
                Statement::Let { name, value, .. } => {
                    let val = self.compile_expr(value, variables, function)?;
                    let val_type = val.get_type();
                    let val_type_name = self.get_expr_type_name(value, variables);
                    let alloca = self.builder.build_alloca(val_type, name).unwrap();
                    self.builder.build_store(alloca, val).unwrap();
                    variables.insert(name.clone(), (alloca, val_type, val_type_name));
                    last_val = None;
                },
                Statement::Assignment { target, value } => {
                    let ptr = match target {
                        Expression::Identifier(name) => {
                            if let Some((var_name, field_name)) = name.split_once('.') {
                                if let Some((var_ptr, var_type, type_name)) = variables.get(var_name) {
                                    let full_type_name = self.resolve_fuzzy_name(&self.struct_fields, type_name).unwrap_or(type_name.clone());
                                    if let Some(fields) = self.struct_fields.get(&full_type_name) {
                                        if let Some(&idx) = fields.get(field_name) {
                                            self.builder.build_struct_gep(*var_type, *var_ptr, idx, "fieldptr").unwrap()
                                        } else { return Err(format!("Field '{}' not found on type '{}'", field_name, full_type_name)); }
                                    } else { return Err(format!("Type '{}' is not a known struct with fields", full_type_name)); }
                                } else { return Err(format!("Variable '{}' not defined", var_name)); }
                            } else {
                                if let Some((alloca, _, _)) = variables.get(name) {
                                    *alloca
                                } else {
                                    return Err(format!("Assignment to undefined variable '{}'", name));
                                }
                            }
                        },
                        Expression::Deref { expr } => {
                            let val = self.compile_expr(expr, variables, function)?;
                            if val.is_pointer_value() {
                                val.into_pointer_value()
                            } else {
                                return Err("Assignment to non-pointer dereference".to_string());
                            }
                        },
                        _ => return Err("Invalid assignment target".to_string()),
                    };
                    
                    let val = self.compile_expr(value, variables, function)?;
                    self.builder.build_store(ptr, val).unwrap();
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
                Statement::While { condition, body } => {
                    let cond_bb = self.context.append_basic_block(function, "while_cond");
                    let body_bb = self.context.append_basic_block(function, "while_body");
                    let exit_bb = self.context.append_basic_block(function, "while_exit");
                    
                    self.builder.build_unconditional_branch(cond_bb).unwrap();
                    
                    self.builder.position_at_end(cond_bb);
                    let cond_val = self.compile_expr(condition, variables, function)?.into_int_value();
                    let comparison = self.builder.build_int_compare(IntPredicate::NE, cond_val, self.context.i64_type().const_int(0, false), "loopcond").unwrap();
                    self.builder.build_conditional_branch(comparison, body_bb, exit_bb).unwrap();
                    
                    self.builder.position_at_end(body_bb);
                    self.compile_block(body, variables, function)?;
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(cond_bb).unwrap();
                    }
                    
                    self.builder.position_at_end(exit_bb);
                    last_val = None;
                },
                Statement::Match { condition, arms } => {
                    let cond_val = self.compile_expr(condition, variables, function)?;
                    
                    let exit_bb = self.context.append_basic_block(function, "matchexit");
                    
                    if cond_val.is_struct_value() {
                        let enum_val = cond_val.into_struct_value();
                        let alloca = self.builder.build_alloca(enum_val.get_type(), "matched_enum").unwrap();
                        self.builder.build_store(alloca, enum_val).unwrap();
                        let tag_ptr = self.builder.build_struct_gep(enum_val.get_type(), alloca, 0, "tagptr").unwrap();
                        let tag = self.builder.build_load(self.context.i64_type(), tag_ptr, "tag").unwrap().into_int_value();
                        
                        let cond_type_name = self.get_expr_type_name(condition, variables);

                        for (i, arm) in arms.iter().enumerate() {
                            let arm_bb_name = format!("arm_{}_{}", arm.pattern, i);
                            let arm_bb = self.context.append_basic_block(function, &arm_bb_name);
                            let next_bb = self.context.append_basic_block(function, "match_next");
                            
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
                                
                                let (load_type, cast_type) = if arm.pattern == "Some" || arm.pattern == "Ok" {
                                    let ptr_t = self.context.ptr_type(AddressSpace::default());
                                    (ptr_t.into(), ptr_t)
                                } else {
                                    let i64_t = self.context.i64_type();
                                    (i64_t.into(), self.context.ptr_type(AddressSpace::default()))
                                };

                                let casted_ptr = self.builder.build_bit_cast(data_ptr, cast_type, "arm_datacast").unwrap();
                                let loaded_val = self.builder.build_load(load_type, casted_ptr.into_pointer_value(), param_name).unwrap();
                                let param_alloca = self.builder.build_alloca(load_type, param_name).unwrap();
                                self.builder.build_store(param_alloca, loaded_val).unwrap();
                                
                                let payload_type_name = if cond_type_name.starts_with("Option<") {
                                    cond_type_name[7..cond_type_name.len()-1].to_string()
                                } else { "unknown".to_string() };

                                arm_vars.insert(param_name.clone(), (param_alloca, load_type, payload_type_name));
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
                Statement::NoOp => { last_val = None; },
                _ => { last_val = None; }
            }
        }
        Ok(last_val)
    }

    fn get_expr_type_name(&self, expr: &Expression, variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>) -> String {
        match expr {
            Expression::Integer(_) => "i64".to_string(),
            Expression::Float(_) => "f64".to_string(),
            Expression::Boolean(_) => "bool".to_string(),
            Expression::String(_) => "String".to_string(),
            Expression::Duration(_, _) => "Duration".to_string(),
            Expression::Date(_) => "Date".to_string(),
            Expression::Identifier(name) => {
                if let Some((var_name, field_name)) = name.split_once('.') {
                    if let Some((_, _, var_type_name)) = variables.get(var_name) {
                        if let Some(decl) = self.decls.get(var_type_name) {
                            if let Declaration::Struct(s) = decl {
                                for (f_name, f_type) in &s.fields {
                                    if f_name == field_name { return f_type.clone(); }
                                }
                            }
                        }
                    }
                    "unknown".to_string()
                } else {
                    if let Some((_, _, t_name)) = variables.get(name) {
                        t_name.clone()
                    } else { "unknown".to_string() }
                }
            },
            Expression::Call { function: name, .. } => {
                if name == "string.len" || name == "fs.write" || name == "fs.exists" { return "i64".to_string(); }
                if name == "string.concat" || name == "fs.read_to_string" { return "String".to_string(); }
                if name == "env.var" { return "Option<String>".to_string(); }
                
                if let Some(decl) = self.decls.get(name) {
                    if let Declaration::Function(f) = decl {
                        f.return_type.clone()
                    } else { "unknown".to_string() }
                } else { "unknown".to_string() }
            },
            Expression::StructInst { name, .. } => name.clone(),
            Expression::EnumInst { name, .. } => name.clone(),
            Expression::If { .. } => "unknown".to_string(),
            Expression::Cast { target, .. } => target.clone(),
            Expression::Deref { .. } => "unknown".to_string(),
            Expression::Intrinsic { name, .. } => {
                 if name == "str_len" { "i64".to_string() }
                 else if name == "str_concat" { "String".to_string() }
                 else if name == "fs_read_to_string" { "String".to_string() }
                 else { "i64".to_string() } 
            },
            _ => "unknown".to_string(),
        }
    }

    fn compile_expr(
        &mut self,
        expr: &Expression, 
        variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>,
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
                if let Some((var_name, field_name)) = name.split_once('.') {
                    if let Some((var_ptr, var_type, type_name)) = variables.get(var_name) {
                        let full_type_name = self.resolve_fuzzy_name(&self.struct_fields, type_name).unwrap_or(type_name.clone());
                        if let Some(fields) = self.struct_fields.get(&full_type_name) {
                            if let Some(&idx) = fields.get(field_name) {
                                let field_ptr = self.builder.build_struct_gep(*var_type, *var_ptr, idx, "fieldptr").unwrap();
                                let field_type = var_type.into_struct_type().get_field_type_at_index(idx).unwrap();
                                Ok(self.builder.build_load(field_type, field_ptr, field_name).unwrap())
                            } else { Err(format!("Field '{}' not found on type '{}'", field_name, full_type_name)) }
                        } else { Err(format!("Type '{}' not struct", full_type_name)) }
                    } else { 
                        Err(format!("Variable '{}' not defined", var_name)) 
                    }
                } else {
                    let var = variables.get(name);
                    if let Some((ptr, var_type, _)) = var {
                        Ok(self.builder.build_load(*var_type, *ptr, name).unwrap())
                    } else {
                        if name == "argc" {
                            if let Some(global) = self.module.get_global("aion_argc") {
                                return Ok(self.builder.build_load(self.context.i64_type(), global.as_pointer_value(), "argc_load").unwrap());
                            }
                        } else if name == "argv" {
                            if let Some(global) = self.module.get_global("aion_argv") {
                                return Ok(self.builder.build_load(self.context.ptr_type(AddressSpace::default()), global.as_pointer_value(), "argv_load").unwrap());
                            }
                        }
                        Err(format!("Var '{}' not found", name))
                    }
                }
            },
            Expression::Call { function: func_name, generic_args, arguments } => {
                if func_name == "io.println" {
                    return self.compile_expr(&Expression::Intrinsic { name: "io_println".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "io.print" {
                    return self.compile_expr(&Expression::Intrinsic { name: "io_print".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "env.var" {
                    let getenv_fn = self.module.get_function("aion_getenv").ok_or("aion_getenv not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    
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
                
                let (resolved_func_name, resolved_args) = if let Some((var_name, method_name)) = func_name.split_once('.') {
                     if let Some((var_ptr, var_type, type_name)) = variables.get(var_name) {
                         if type_name.starts_with('*') && method_name == "offset" && arguments.len() == 1 {
                             let idx = self.compile_expr(&arguments[0], variables, function)?.into_int_value();
                             let ptr = self.builder.build_load(*var_type, *var_ptr, "ptr").unwrap().into_pointer_value();
                             let elem_type_name = &type_name[1..];
                             let elem_type = self.aion_type_to_llvm(elem_type_name);
                             let gep = unsafe { self.builder.build_gep(elem_type, ptr, &[idx], "offset_ptr").unwrap() };
                             return Ok(gep.into());
                         }

                         let type_prefix = self.resolve_fuzzy_name(&self.struct_fields, type_name).unwrap_or(type_name.clone());
                         let candidate = format!("{}.{}", type_prefix, method_name);
                         if self.decls.contains_key(&candidate) {
                             let mut new_args = arguments.clone();
                             new_args.insert(0, Expression::Identifier(var_name.to_string()));
                             (candidate, new_args)
                         } else {
                             (func_name.clone(), arguments.clone())
                         }
                     } else { 
                         (func_name.clone(), arguments.clone()) 
                     }
                } else { (func_name.clone(), arguments.clone()) };

                let func_name = &resolved_func_name;
                let arguments = &resolved_args;

                let actual_func_name = if !generic_args.is_empty() {
                    let name = self.get_monomorphized_name(func_name, generic_args);
                    if !self.compiled_instances.contains(&name) { 
                        let decl_name = self.resolve_fuzzy_name(&self.decls, func_name).ok_or(format!("Generic function '{}' not found", func_name))?;
                        let decl = self.decls.get(&decl_name).cloned().unwrap();
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
                } else { 
                    self.resolve_fuzzy_name(&self.decls, func_name).unwrap_or(func_name.clone())
                };

                let fn_val = {
                    let full_name = self.resolve_fuzzy_name(&self.decls, &actual_func_name).unwrap_or(actual_func_name.clone());
                    let mut llvm_name = full_name.clone();
                    if let Some(decl) = self.decls.get(&full_name) {
                        if let Declaration::Function(f) = decl {
                            for (attr_name, attr_val) in &f.attributes {
                                if attr_name == "intrinsic" {
                                    llvm_name = attr_val.replace("libc.", "");
                                    break;
                                }
                            }
                        }
                    }
                    self.module.get_function(&llvm_name).ok_or(format!("Function '{}' not found (LLVM name: {})", actual_func_name, llvm_name))?
                };

                let mut compiled_args = Vec::new();
                let full_name = self.resolve_fuzzy_name(&self.decls, &actual_func_name).unwrap_or(actual_func_name.clone());
                let decl = self.decls.get(&full_name).cloned();
                
                for (i, arg_expr) in arguments.iter().enumerate() {
                    let mut passed_by_pointer = false;
                    if let Some(Declaration::Function(f)) = &decl {
                        if i < f.params.len() {
                            let (p_name, p_type) = &f.params[i];
                            let base_type_name = if p_type.contains('<') { p_type.split('<').next().unwrap() } else { p_type };
                            if p_name == "self" && self.struct_types.contains_key(base_type_name) {
                                if let Expression::Identifier(var_name) = arg_expr {
                                    if let Some((ptr, _, _)) = variables.get(var_name) {
                                        compiled_args.push((*ptr).into());
                                        passed_by_pointer = true;
                                    }
                                }
                            }
                        }
                    }
                    
                    if !passed_by_pointer {
                        compiled_args.push(self.compile_expr(arg_expr, variables, function)?.into());
                    }
                }
                
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
                if name == "io_println" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "io_println")) {
                    let printf = self.module.get_function("printf").ok_or("printf not found")?;
                    let arg_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let arg = self.compile_expr(arg_expr, variables, function)?;
                    
                    let fmt = if arg.get_type().is_pointer_type() {
                        "%s\n\0"
                    } else {
                        "%lld\n\0"
                    };
                    
                    let fmt_str = self.builder.build_global_string_ptr(fmt, "println_fmt").unwrap();
                    self.builder.build_call(printf, &[fmt_str.as_basic_value_enum().into(), arg.into()], "printftmp").unwrap();
                    Ok(i64_type.const_int(0, false).into())
                } else if name == "io_print" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "io_print")) {
                    let printf = self.module.get_function("printf").ok_or("printf not found")?;
                    let arg_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let arg = self.compile_expr(arg_expr, variables, function)?;
                    
                    let fmt = if arg.get_type().is_pointer_type() {
                        "%s\0"
                    } else {
                        "%lld\0"
                    };
                    
                    let fmt_str = self.builder.build_global_string_ptr(fmt, "print_fmt").unwrap();
                    self.builder.build_call(printf, &[fmt_str.as_basic_value_enum().into(), arg.into()], "printtmp").unwrap();
                    Ok(i64_type.const_int(0, false).into())
                } else if name == "io_read_line" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "io_read_line")) {
                    Ok(self.context.ptr_type(AddressSpace::default()).const_null().into())
                } else if name == "sizeof" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "sizeof")) {
                    let type_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    if let Expression::Identifier(type_name) = type_expr {
                        let llvm_type = self.aion_type_to_llvm(type_name);
                        let size = llvm_type.size_of().ok_or(format!("Could not determine size of '{}'", type_name))?;
                        Ok(size.into())
                    } else { Err("sizeof requires type identifier".to_string()) }
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
                            if val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 32 {
                                Ok(self.builder.build_int_s_extend(val.into_int_value(), self.context.i64_type(), "i32toi64").unwrap().into())
                            } else { Ok(val) }
                        },
                        ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                    }
                } else if name == "fs_exists" {
                    let exists_fn = self.module.get_function("aion_fs_exists").ok_or("aion_fs_exists not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    let call = self.builder.build_call(exists_fn, &[arg.into()], "existstmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => {
                            if val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 32 {
                                Ok(self.builder.build_int_z_extend(val.into_int_value(), self.context.i64_type(), "boolcast").unwrap().into())
                            } else { Ok(val) }
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
                } else if name == "mem_is_null" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "mem_is_null")) {
                    let ptr_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let arg = self.compile_expr(ptr_expr, variables, function)?;
                    if arg.is_pointer_value() {
                        let ptr = arg.into_pointer_value();
                        let ptr_int = self.builder.build_ptr_to_int(ptr, self.context.i64_type(), "ptrtoint").unwrap();
                        let eq_zero = self.builder.build_int_compare(IntPredicate::EQ, ptr_int, self.context.i64_type().const_zero(), "isnull").unwrap();
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
            Expression::Cast { expr, target } => {
                let val = self.compile_expr(expr, variables, function)?;
                let dest_type = self.aion_type_to_llvm(target);
                
                if val.is_pointer_value() && dest_type.is_pointer_type() {
                    Ok(self.builder.build_bit_cast(val.into_pointer_value(), dest_type.into_pointer_type(), "cast").unwrap().into())
                } else if val.is_int_value() && dest_type.is_pointer_type() {
                    Ok(self.builder.build_int_to_ptr(val.into_int_value(), dest_type.into_pointer_type(), "cast").unwrap().into())
                } else if val.is_pointer_value() && dest_type.is_int_type() {
                    Ok(self.builder.build_ptr_to_int(val.into_pointer_value(), dest_type.into_int_type(), "cast").unwrap().into())
                } else {
                    Ok(self.builder.build_bit_cast(val, dest_type, "cast").unwrap())
                }
            },
            Expression::Deref { expr } => {
                let val = self.compile_expr(expr, variables, function)?;
                if val.is_pointer_value() {
                    let ptr = val.into_pointer_value();
                    Ok(self.builder.build_load(self.context.ptr_type(AddressSpace::default()), ptr, "deref").unwrap())
                } else {
                    Err("Dereference of non-pointer".to_string())
                }
            },
            _ => Ok(i64_type.const_int(0, false).into()),
        }
    }

    pub fn print_to_file(&self, path: &Path) -> Result<(), String> {
        self.module.print_to_file(path).map_err(|e| e.to_string())
    }
}

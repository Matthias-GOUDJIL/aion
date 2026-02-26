use std::path::Path;
use std::collections::{HashMap, HashSet};
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, PointerValue, FunctionValue, BasicValue, ValueKind};
use inkwell::types::{StructType, BasicTypeEnum, BasicType};
use inkwell::{AddressSpace, IntPredicate};
use crate::ast::*;
use crate::token::{Token, TokenKind};

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
            "i64" | "u64" => self.context.i64_type().into(),
            "i32" | "u32" => self.context.i32_type().into(),
            "i8" | "u8" => self.context.i8_type().into(),
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
            Expression::Call { function, generic_args, arguments, .. } => {
                for i in 0..placeholders.len() {
                    *function = function.replace(&placeholders[i], &concrete[i]);
                    for arg in generic_args.iter_mut() {
                        *arg = arg.replace(&placeholders[i], &concrete[i]);
                    }
                }
                for arg in arguments { self.substitute_types_in_expr(arg, placeholders, concrete); }
            },
            Expression::EnumInst { name, generic_args, arguments, .. } => {
                for i in 0..placeholders.len() {
                    *name = name.replace(&placeholders[i], &concrete[i]);
                    for arg in generic_args.iter_mut() {
                        *arg = arg.replace(&placeholders[i], &concrete[i]);
                    }
                }
                for arg in arguments { self.substitute_types_in_expr(arg, placeholders, concrete); }
            },
            Expression::StructInst { name, generic_args, fields, .. } => {
                for i in 0..placeholders.len() {
                    *name = name.replace(&placeholders[i], &concrete[i]);
                    for arg in generic_args.iter_mut() {
                        *arg = arg.replace(&placeholders[i], &concrete[i]);
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
                    *target = target.replace(&placeholders[i], &concrete[i]);
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
            Expression::Identifier(name) => {
                for i in 0..placeholders.len() {
                    *name = name.replace(&placeholders[i], &concrete[i]);
                }
            },
            Expression::MemberAccess { receiver, .. } => self.substitute_types_in_expr(receiver, placeholders, concrete),
            Expression::MethodCall { receiver, generic_args, arguments, .. } => {
                self.substitute_types_in_expr(receiver, placeholders, concrete);
                for arg in generic_args {
                    for i in 0..placeholders.len() {
                        *arg = arg.replace(&placeholders[i], &concrete[i]);
                    }
                }
                for arg in arguments {
                    self.substitute_types_in_expr(arg, placeholders, concrete);
                }
            },
            Expression::TypeRef { name, generic_args } => {
                for i in 0..placeholders.len() {
                    *name = name.replace(&placeholders[i], &concrete[i]);
                }
                for arg in generic_args {
                    for i in 0..placeholders.len() {
                        *arg = arg.replace(&placeholders[i], &concrete[i]);
                    }
                }
            },
            _ => {}
        }
    }

    fn instantiate_function(&mut self, base_name: &str, generic_args: &[String]) -> Result<FunctionValue<'ctx>, String> {
        let decl = self.decls.get(base_name).cloned().ok_or(format!("Generic function '{}' not found", base_name))?;
        if let Declaration::Function(mut f) = decl {
            let placeholders = f.generic_params.clone();
            let new_name = format!("{}_{}", base_name, generic_args.join("_"));
            
            if let Some(existing) = self.module.get_function(&new_name) {
                return Ok(existing);
            }

            f.name = new_name.clone();
            f.generic_params = vec![];
            
            for i in 0..placeholders.len() {
                let p = &placeholders[i];
                let c = &generic_args[i];
                for (_, pt) in f.params.iter_mut() {
                    *pt = pt.replace(p, c);
                }
                f.return_type = f.return_type.replace(p, c);
            }
            
            if let Some(body) = &mut f.body {
                self.substitute_types_in_body(body, &placeholders, generic_args);
            }
            
            self.decls.insert(new_name.clone(), Declaration::Function(f.clone()));
            self.compiled_instances.insert(new_name.clone());
            
            self.compile_function(&Declaration::Function(f))
        } else {
            Err(format!("'{}' is not a function", base_name))
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
                    for (p_name, p_type) in &f.params {
                        let mut llvm_p_type = self.aion_type_to_llvm(p_type);
                        if p_name == "self" {
                            let base_type_name = if p_type.contains('<') { p_type.split('<').next().unwrap() } else { p_type };
                            if self.struct_types.contains_key(base_type_name) || self.enum_types.contains_key(base_type_name) {
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
                    let gc_init = self.module.get_function("GC_init").unwrap();
                    self.builder.build_call(gc_init, &[], "").unwrap();
                    
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
                            let resolved_base_name = self.resolve_fuzzy_name(&self.struct_types, base_type_name)
                                .or_else(|| self.resolve_fuzzy_name(&self.enum_types, base_type_name));
                            
                            if arg_name == "self" && resolved_base_name.is_some() {
                                let full_type_name = resolved_base_name.unwrap();
                                let struct_type = self.struct_types.get(&full_type_name)
                                    .or_else(|| self.enum_types.get(&full_type_name))
                                    .unwrap();
                                local_vars.insert(arg_name.clone(), (arg.into_pointer_value(), struct_type.as_basic_type_enum(), arg_type_name.clone()));
                            } else {
                                let alloca = self.builder.build_alloca(arg.get_type(), arg_name).unwrap();
                                self.builder.build_store(alloca, arg).unwrap();
                                local_vars.insert(arg_name.clone(), (alloca, arg.get_type(), arg_type_name.clone()));
                            }
                        }
                    }
                }

                let last_block_val = self.compile_block(body, &mut local_vars, function)?;
                if let Some(current_block) = self.builder.get_insert_block() {
                    if current_block.get_terminator().is_none() {
                        if let Some(val) = last_block_val {
                            self.builder.build_return(Some(&val)).unwrap();
                        } else {
                            let ret_type = function.get_type().get_return_type().map(|t| t.as_basic_type_enum()).unwrap_or(self.context.i64_type().into());
                            self.builder.build_return(Some(&ret_type.const_zero())).unwrap();
                        }
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
                    let mut full_target_name = i.target_name.clone();
                    if !i.generic_params.is_empty() {
                        full_target_name = format!("{}<{}>", i.target_name, i.generic_params.join(", "));
                    }
                    let base_target = if i.target_name.contains('<') {
                        i.target_name.split('<').next().unwrap()
                    } else {
                        &i.target_name
                    };
                    for f in &i.functions {
                        let mut new_f = f.clone();
                        new_f.name = format!("{}.{}", base_target, f.name);
                        
                        for (_, p_type) in new_f.params.iter_mut() { 
                            if p_type == "Self" { *p_type = full_target_name.clone(); } 
                        }
                        if new_f.return_type == "Self" { new_f.return_type = full_target_name.clone(); }

                        let mut combined = i.generic_params.clone();
                        combined.extend(f.generic_params.clone());
                        new_f.generic_params = combined;
                        self.decls.insert(new_f.name.clone(), Declaration::Function(new_f));
                    }
                },
                _ => {}
            }
        }

        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let i64_type = self.context.i64_type();
        let f64_type = self.context.f64_type();
        self.module.add_function("printf", self.context.i32_type().fn_type(&[ptr_type.into()], true), None);
        self.module.add_function("strlen", self.context.i64_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("exit", self.context.void_type().fn_type(&[self.context.i32_type().into()], false), None);
        self.module.add_function("malloc", ptr_type.fn_type(&[self.context.i64_type().into()], false), None);
        self.module.add_function("realloc", ptr_type.fn_type(&[ptr_type.into(), self.context.i64_type().into()], false), None);
        self.module.add_function("free", self.context.void_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("aion_io_print", self.context.void_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("aion_io_println", self.context.void_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("GC_init", self.context.void_type().fn_type(&[], false), None);
        self.module.add_function("aion_str_concat", ptr_type.fn_type(&[ptr_type.into(), ptr_type.into()], false), None);
        self.module.add_function("aion_int_to_str", ptr_type.fn_type(&[i64_type.into()], false), None);
        self.module.add_function("aion_float_to_str", ptr_type.fn_type(&[f64_type.into()], false), None);
        self.module.add_function("aion_str_eq", i64_type.fn_type(&[ptr_type.into(), ptr_type.into()], false), None);
        self.module.add_function("aion_spawn", self.context.void_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("pow", self.context.f64_type().fn_type(&[self.context.f64_type().into(), self.context.f64_type().into()], false), None);
        self.module.add_function("aion_read_file", ptr_type.fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("aion_write_file", self.context.i32_type().fn_type(&[ptr_type.into(), ptr_type.into()], false), None);
        self.module.add_function("aion_fs_exists", self.context.i64_type().fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("aion_getenv", ptr_type.fn_type(&[ptr_type.into()], false), None);
        self.module.add_function("aion_get_argv_index", ptr_type.fn_type(&[ptr_type.into(), i64_type.into()], false), None);
        
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
                let mut field_map = HashMap::new();
                for (i, (name, _)) in s.fields.iter().enumerate() {
                    field_map.insert(name.clone(), i as u32);
                }
                self.struct_fields.insert(s.name.clone(), field_map);

                let struct_type = *self.struct_types.get(&s.name).unwrap();
                let mut field_types = Vec::new();
                for (_, type_name) in &s.fields {
                    field_types.push(self.aion_type_to_llvm(type_name));
                }
                struct_type.set_body(&field_types, false);
            }
        }

        if self.resolve_fuzzy_name(&self.enum_types, "Option").is_none() {
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
                                if self.struct_types.contains_key(base_type_name) || self.enum_types.contains_key(base_type_name) {
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
                    let ptr = self.compile_lvalue(target, variables, function)?;
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
                    
                    let mut phi_entries: Vec<(Option<BasicValueEnum<'ctx>>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();
                    
                    self.builder.position_at_end(then_bb);
                    let mut then_vars = variables.clone();
                    let then_val = self.compile_block(then_branch, &mut then_vars, function)?;
                    let then_block_final = self.builder.get_insert_block().unwrap();
                    if then_block_final.get_terminator().is_none() {
                        phi_entries.push((then_val, then_block_final));
                        self.builder.build_unconditional_branch(merge_bb).unwrap();
                    }
                    
                    self.builder.position_at_end(else_bb);
                    let mut else_vars = variables.clone();
                    let else_val = if let Some(eb) = else_branch { 
                        self.compile_block(eb, &mut else_vars, function)?
                    } else { None };
                    let else_block_final = self.builder.get_insert_block().unwrap();
                    if else_block_final.get_terminator().is_none() {
                        phi_entries.push((else_val, else_block_final));
                        self.builder.build_unconditional_branch(merge_bb).unwrap();
                    }
                    
                    if merge_bb.get_first_use().is_some() || true {
                        self.builder.position_at_end(merge_bb);
                        
                        let active_entries: Vec<(BasicValueEnum<'ctx>, _)> = phi_entries.iter()
                            .filter_map(|(v, bb)| v.map(|val| (val, *bb)))
                            .collect();

                        if !active_entries.is_empty() {
                            let first_val = active_entries[0].0;
                            let all_same = phi_entries.iter().all(|(v, _)| v.map_or(true, |val| val.get_type() == first_val.get_type()));
                            if all_same {
                                let phi = self.builder.build_phi(first_val.get_type(), "ifres").unwrap();
                                for (v, bb) in phi_entries {
                                    let val = v.unwrap_or_else(|| {
                                        if first_val.is_pointer_value() {
                                            self.context.ptr_type(AddressSpace::default()).const_null().into()
                                        } else if first_val.is_float_value() {
                                            self.context.f64_type().const_zero().into()
                                        } else {
                                            self.context.i64_type().const_zero().into()
                                        }
                                    });
                                    phi.add_incoming(&[(&val, bb)]);
                                }
                                last_val = Some(phi.as_basic_value());
                            } else { last_val = None; }
                        } else { last_val = None; }
                    } else {
                        unsafe { merge_bb.delete().unwrap(); }
                        last_val = None;
                    }
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
                    let mut body_vars = variables.clone();
                    self.compile_block(body, &mut body_vars, function)?;
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        self.builder.build_unconditional_branch(cond_bb).unwrap();
                    }
                    
                    self.builder.position_at_end(exit_bb);
                    last_val = None;
                },
                Statement::Match { condition, arms } => {
                    let cond_val = self.compile_expr(condition, variables, function)?;
                    let exit_bb = self.context.append_basic_block(function, "matchexit");
                    let mut phi_entries: Vec<(Option<BasicValueEnum<'ctx>>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();
                    
                    if cond_val.is_struct_value() {
                        let enum_val = cond_val.into_struct_value();
                        let alloca = self.builder.build_alloca(enum_val.get_type(), "matched_enum").unwrap();
                        self.builder.build_store(alloca, enum_val).unwrap();
                        let tag_ptr = self.builder.build_struct_gep(enum_val.get_type(), alloca, 0, "tagptr").unwrap();
                        let tag = self.builder.build_load(self.context.i64_type(), tag_ptr, "tag").unwrap().into_int_value();
                        let cond_type_name = self.get_expr_type_name(condition, variables);

                        for (i, arm) in arms.iter().enumerate() {
                            let arm_bb = self.context.append_basic_block(function, &format!("arm_{}_{}", arm.pattern, i));
                            let next_bb = self.context.append_basic_block(function, "match_next");
                            let arm_tag = if arm.pattern == "Some" || arm.pattern == "Ok" || arm.pattern.ends_with("::Some") || arm.pattern.ends_with(".Some") { 0 } 
                                     else if arm.pattern == "None" || arm.pattern == "Err" || arm.pattern.ends_with("::None") || arm.pattern.ends_with(".None") { 1 } 
                                     else { i as u64 };
                            let is_arm = self.builder.build_int_compare(IntPredicate::EQ, tag, self.context.i64_type().const_int(arm_tag, false), "is_arm").unwrap();
                            self.builder.build_conditional_branch(is_arm, arm_bb, next_bb).unwrap();
                            
                            self.builder.position_at_end(arm_bb);
                            let mut arm_vars = variables.clone();
                            if !arm.params.is_empty() {
                                let data_ptr = self.builder.build_struct_gep(enum_val.get_type(), alloca, 1, "arm_dataptr").unwrap();
                                let param_name = &arm.params[0];
                                let payload_type_name = if cond_type_name.contains('<') {
                                    let parts: Vec<&str> = cond_type_name.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                                    if arm.pattern == "Some" || arm.pattern == "Ok" { parts[1].trim().to_string() }
                                    else { parts.get(2).unwrap_or(&parts[1]).trim().to_string() }
                                } else { "i64".to_string() };
                                let load_type = self.aion_type_to_llvm(&payload_type_name);
                                let casted_ptr = self.builder.build_bit_cast(data_ptr, self.context.ptr_type(AddressSpace::default()), "arm_datacast").unwrap();
                                let loaded_val = self.builder.build_load(load_type, casted_ptr.into_pointer_value(), param_name).unwrap();
                                let param_alloca = self.builder.build_alloca(load_type, param_name).unwrap();
                                self.builder.build_store(param_alloca, loaded_val).unwrap();
                                arm_vars.insert(param_name.clone(), (param_alloca, load_type, payload_type_name));
                            }
                            let arm_val = self.compile_block(&arm.body, &mut arm_vars, function)?;
                            let arm_block_final = self.builder.get_insert_block().unwrap();
                            if arm_block_final.get_terminator().is_none() {
                                phi_entries.push((arm_val, arm_block_final));
                                self.builder.build_unconditional_branch(exit_bb).unwrap();
                            }
                            self.builder.position_at_end(next_bb);
                        }
                        let final_fallback_bb = self.builder.get_insert_block().unwrap();
                        if final_fallback_bb.get_terminator().is_none() {
                            phi_entries.push((None, final_fallback_bb));
                            self.builder.build_unconditional_branch(exit_bb).unwrap();
                        }
                    } else {
                        for arm in arms {
                            let arm_bb = self.context.append_basic_block(function, &format!("arm_{}", arm.pattern));
                            self.builder.position_at_end(arm_bb);
                            let mut arm_vars = variables.clone();
                            let arm_val = self.compile_block(&arm.body, &mut arm_vars, function)?;
                            let arm_block_final = self.builder.get_insert_block().unwrap();
                            if arm_block_final.get_terminator().is_none() {
                                phi_entries.push((arm_val, arm_block_final));
                                self.builder.build_unconditional_branch(exit_bb).unwrap();
                            }
                        }
                        let final_fallback_bb = self.builder.get_insert_block().unwrap();
                        if final_fallback_bb.get_terminator().is_none() {
                            phi_entries.push((None, final_fallback_bb));
                            self.builder.build_unconditional_branch(exit_bb).unwrap();
                        }
                    }
                    
                    self.builder.position_at_end(exit_bb);
                    // Filter phi_entries to get only those with values
                    let active_entries: Vec<(BasicValueEnum<'ctx>, _)> = phi_entries.iter()
                        .filter_map(|(v, bb)| v.map(|val| (val, *bb)))
                        .collect();

                    if !active_entries.is_empty() {
                        let first_val = active_entries[0].0;
                        let phi = self.builder.build_phi(first_val.get_type(), "matchres").unwrap();
                        for (v, bb) in phi_entries {
                            let val = v.unwrap_or_else(|| {
                                // Provide a zero value of the correct type
                                if first_val.is_pointer_value() {
                                    self.context.ptr_type(AddressSpace::default()).const_null().into()
                                } else if first_val.is_float_value() {
                                    self.context.f64_type().const_zero().into()
                                } else {
                                    self.context.i64_type().const_zero().into()
                                }
                            });
                            phi.add_incoming(&[(&val, bb)]);
                        }
                        last_val = Some(phi.as_basic_value());
                    } else {
                        last_val = None;
                    }
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
                        let base_var_type_name = if var_type_name.contains('<') { var_type_name.split('<').next().unwrap() } else { var_type_name };
                        let full_type_name = self.resolve_fuzzy_name(&self.struct_types, base_var_type_name).unwrap_or(base_var_type_name.to_string());
                        if let Some(decl) = self.decls.get(&full_type_name) {
                            if let Declaration::Struct(s) = decl {
                                for (f_name, f_type) in &s.fields {
                                    if f_name == field_name { 
                                        let mut ret = f_type.clone();
                                        if var_type_name.contains('<') {
                                            let parts: Vec<&str> = var_type_name.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                                            let concrete = &parts[1..];
                                            let placeholders = &s.generic_params;
                                            for i in 0..placeholders.len() {
                                                if i < concrete.len() {
                                                    ret = ret.replace(placeholders[i].trim(), concrete[i].trim());
                                                }
                                            }
                                        }
                                        return ret; 
                                    }
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
            Expression::Call { function: name, generic_args, .. } => {
                if let Some((receiver_name, method_name)) = name.rsplit_once('.') {
                    if method_name == "offset" {
                        let receiver_expr = Expression::Identifier(receiver_name.to_string());
                        return self.get_expr_type_name(&receiver_expr, variables);
                    }
                    
                    let receiver_expr = Expression::Identifier(receiver_name.to_string());
                    let type_name = self.get_expr_type_name(&receiver_expr, variables);
                    if type_name != "unknown" {
                         let (base_type_name, type_generic_args) = if type_name.contains('<') {
                             let parts: Vec<&str> = type_name.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                             (parts[0].to_string(), parts[1..].iter().map(|s| s.trim().to_string()).collect::<Vec<String>>())
                         } else { (type_name.clone(), vec![]) };
                         let type_prefix = self.resolve_fuzzy_name(&self.decls, &base_type_name).unwrap_or(base_type_name.clone());
                         let candidate = format!("{}.{}", type_prefix, method_name);
                         if let Some(Declaration::Function(f)) = self.decls.get(&candidate) {
                             let mut ret = f.return_type.clone();
                             if let Some(Declaration::Struct(s)) = self.decls.get(&type_prefix) {
                                 for (i, p) in s.generic_params.iter().enumerate() {
                                     if i < type_generic_args.len() { ret = ret.replace(p, &type_generic_args[i]); }
                                 }
                             }
                             for (i, p) in f.generic_params.iter().enumerate() {
                                 if i < generic_args.len() { ret = ret.replace(p, &generic_args[i]); }
                             }
                             return ret;
                         }
                    }
                }
                if name == "string.len" || name == "fs.write" || name == "fs.exists" { return "i64".to_string(); }
                if name == "string.concat" || name == "fs.read_to_string" || name == "int_to_str" || name == "float_to_str" { return "String".to_string(); }
                if name == "env.var" { return "Option<String>".to_string(); }
                
                let decl_name = self.resolve_fuzzy_name(&self.decls, name).unwrap_or(name.clone());
                if let Some(decl) = self.decls.get(&decl_name) {
                    if let Declaration::Function(f) = decl {
                        let mut ret = f.return_type.clone();
                        for (i, p) in f.generic_params.iter().enumerate() {
                            if i < generic_args.len() {
                                ret = ret.replace(p, &generic_args[i]);
                            }
                        }
                        return ret;
                    }
                }
                "unknown".to_string()
            },
            Expression::StructInst { name, generic_args, .. } => {
                if generic_args.is_empty() { name.clone() }
                else { format!("{}<{}>", name, generic_args.join(", ")) }
            },
            Expression::EnumInst { name, generic_args, .. } => {
                if generic_args.is_empty() { name.clone() }
                else { format!("{}<{}>", name, generic_args.join(", ")) }
            },
            Expression::Cast { target, .. } => target.clone(),
            Expression::TypeRef { name, generic_args } => {
                if generic_args.is_empty() { name.clone() }
                else { format!("{}<{}>", name, generic_args.join(", ")) }
            },
            Expression::Deref { expr } => {
                let t = self.get_expr_type_name(expr, variables);
                if t.starts_with('*') { t[1..].to_string() }
                else { "unknown".to_string() }
            },
            Expression::Intrinsic { name, .. } => {
                 if name == "str_len" || name == "fs_exists" { "i64".to_string() }
                 else if name == "str_concat" || name == "fs_read_to_string" || name == "int_to_str" || name == "float_to_str" { "String".to_string() }
                 else if name == "str_ptr" { "*u8".to_string() }
                 else { "i64".to_string() } 
            },
            Expression::Block { statements, .. } => {
                if let Some(Statement::ExpressionStmt(expr)) = statements.last() {
                    self.get_expr_type_name(expr, variables)
                } else { "void".to_string() }
            },
            Expression::MemberAccess { receiver, member } => {
                let rec_type = self.get_expr_type_name(receiver, variables);
                let (base_type, type_gen_args) = if rec_type.contains('<') {
                    let parts: Vec<&str> = rec_type.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                    (parts[0].to_string(), parts[1..].iter().map(|s| s.trim().to_string()).collect::<Vec<String>>())
                } else { (rec_type.clone(), vec![]) };
                let full_type = self.resolve_fuzzy_name(&self.decls, &base_type).unwrap_or(base_type);
                if let Some(Declaration::Struct(s)) = self.decls.get(&full_type) {
                    for (f_name, f_type) in &s.fields {
                        if f_name == member { 
                            let mut ret = f_type.clone();
                            for (i, p) in s.generic_params.iter().enumerate() {
                                if i < type_gen_args.len() { ret = ret.replace(p, &type_gen_args[i]); }
                            }
                            return ret; 
                        }
                    }
                }
                "unknown".to_string()
            },
            Expression::MethodCall { receiver, method, generic_args, arguments: _ } => {
                if method == "offset" { return self.get_expr_type_name(receiver, variables); }
                let rec_type = self.get_expr_type_name(receiver, variables);
                let (base_type, type_gen_args) = if rec_type.contains('<') {
                    let parts: Vec<&str> = rec_type.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                    (parts[0].to_string(), parts[1..].iter().map(|s| s.trim().to_string()).collect::<Vec<String>>())
                } else { (rec_type.clone(), vec![]) };
                let full_type = self.resolve_fuzzy_name(&self.decls, &base_type).unwrap_or(base_type);
                let candidate = format!("{}.{}", full_type, method);
                if let Some(Declaration::Function(f)) = self.decls.get(&candidate) {
                    let mut ret = f.return_type.clone();
                    if let Some(Declaration::Struct(s)) = self.decls.get(&full_type) {
                        for (i, p) in s.generic_params.iter().enumerate() {
                            if i < type_gen_args.len() { ret = ret.replace(p, &type_gen_args[i]); }
                        }
                    }
                    for (i, p) in f.generic_params.iter().enumerate() {
                        if i < generic_args.len() { ret = ret.replace(p, &generic_args[i]); }
                    }
                    return ret;
                }
                "unknown".to_string()
            },
            _ => "unknown".to_string(),
        }
    }

    fn compile_lvalue(
        &mut self,
        expr: &Expression,
        variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>,
        function: FunctionValue<'ctx>
    ) -> Result<PointerValue<'ctx>, String> {
        match expr {
            Expression::Identifier(name) => {
                if let Some((var_name, field_name)) = name.split_once('.') {
                    if let Some((var_ptr, var_type, type_name)) = variables.get(var_name) {
                        let base_type_name = if type_name.contains('<') { type_name.split('<').next().unwrap() } else { type_name };
                        let full_type_name = self.resolve_fuzzy_name(&self.struct_types, base_type_name).unwrap_or(base_type_name.to_string());
                        if let Some(fields) = self.struct_fields.get(&full_type_name) {
                            if let Some(&idx) = fields.get(field_name) {
                                return Ok(self.builder.build_struct_gep(*var_type, *var_ptr, idx, "fieldptr").unwrap());
                            }
                        }
                    }
                }
                let var = variables.get(name).ok_or(format!("Variable '{}' not defined", name))?;
                Ok(var.0)
            },
            Expression::Deref { expr } => {
                let ptr_val = self.compile_expr(expr, variables, function)?;
                if ptr_val.is_pointer_value() { Ok(ptr_val.into_pointer_value()) }
                else { Err("Cannot get l-value of non-pointer dereference".to_string()) }
            },
            Expression::MemberAccess { receiver, member } => {
                let rec_ptr = self.compile_lvalue(receiver, variables, function)?;
                let rec_type_name = self.get_expr_type_name(receiver, variables);
                let base_type = if rec_type_name.contains('<') { rec_type_name.split('<').next().unwrap() } else { &rec_type_name };
                let full_type = self.resolve_fuzzy_name(&self.struct_types, base_type).ok_or(format!("Struct '{}' not found", base_type))?;
                let struct_type = *self.struct_types.get(&full_type).unwrap();
                let fields = self.struct_fields.get(&full_type).unwrap();
                let idx = fields.get(member).ok_or(format!("Field '{}' not found on type '{}'", member, full_type))?;
                Ok(self.builder.build_struct_gep(struct_type, rec_ptr, *idx, "memberptr").unwrap())
            },
            _ => Err(format!("Expression {:?} is not an l-value", expr)),
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
                        let base_type_name = if type_name.contains('<') { type_name.split('<').next().unwrap() } else { type_name };
                        let full_type_name = self.resolve_fuzzy_name(&self.struct_types, base_type_name).unwrap_or(base_type_name.to_string());
                        if let Some(fields) = self.struct_fields.get(&full_type_name) {
                            if let Some(&idx) = fields.get(field_name) {
                                let field_ptr = self.builder.build_struct_gep(*var_type, *var_ptr, idx, field_name).unwrap();
                                let field_type_name = self.get_expr_type_name(expr, variables);
                                let llvm_field_type = self.aion_type_to_llvm(&field_type_name);
                                return Ok(self.builder.build_load(llvm_field_type, field_ptr, field_name).unwrap());
                            }
                        }
                    }
                }
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
            },
            Expression::Call { function: func_name, generic_args, arguments } => {
                if func_name == "io.println" {
                    return self.compile_expr(&Expression::Intrinsic { name: "io_println".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "io.print" {
                    return self.compile_expr(&Expression::Intrinsic { name: "io_print".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "string.len" {
                    return self.compile_expr(&Expression::Intrinsic { name: "str_len".to_string(), arguments: arguments.clone() }, variables, function);
                }
                if func_name == "string.concat" {
                    return self.compile_expr(&Expression::Intrinsic { name: "str_concat".to_string(), arguments: arguments.clone() }, variables, function);
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
                if func_name == "mem.is_null" {
                    return self.compile_expr(&Expression::Intrinsic { name: "mem_is_null".to_string(), arguments: arguments.clone() }, variables, function);
                }

                if func_name == "env.var" {
                    let getenv_fn = self.module.get_function("aion_getenv").ok_or("aion_getenv not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    let call = self.builder.build_call(getenv_fn, &[arg.into()], "getenv_call").unwrap();
                    let ptr_val = match call.try_as_basic_value() {
                        ValueKind::Basic(val) => val,
                        ValueKind::Instruction(_) => return Err("getenv returned no value".to_string()),
                    };
                    let full_option_name = self.resolve_fuzzy_name(&self.enum_types, "Option").ok_or("Option type not found")?;
                    let option_type = *self.enum_types.get(&full_option_name).ok_or("Option type not found in enum_types")?;
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
                
                let mut actual_func_name = func_name.clone();
                let mut actual_generic_args = generic_args.clone();
                let mut actual_args = arguments.clone();
                let mut is_method_call = false;

                if let Some((receiver_name, method_name)) = func_name.rsplit_once('.') {
                     let receiver_expr = Expression::Identifier(receiver_name.to_string());
                     let type_name = self.get_expr_type_name(&receiver_expr, variables);
                     
                     if type_name.starts_with('*') && method_name == "offset" && arguments.len() == 1 {
                         let idx = self.compile_expr(&arguments[0], variables, function)?.into_int_value();
                         let ptr = self.compile_expr(&receiver_expr, variables, function)?.into_pointer_value();
                         let elem_type_name = &type_name[1..];
                         let elem_type = self.aion_type_to_llvm(elem_type_name);
                         let gep = unsafe { self.builder.build_gep(elem_type, ptr, &[idx], "offset_ptr").unwrap() };
                         return Ok(gep.into());
                     }

                     if type_name != "unknown" {
                         let (base_type_name, type_generic_args) = if type_name.contains('<') {
                             let parts: Vec<&str> = type_name.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                             (parts[0].to_string(), parts[1..].iter().map(|s| s.trim().to_string()).collect::<Vec<String>>())
                         } else {
                             (type_name.clone(), vec![])
                         };
                         
                         let type_prefix = self.resolve_fuzzy_name(&self.struct_types, &base_type_name)
                             .or_else(|| self.resolve_fuzzy_name(&self.enum_types, &base_type_name))
                             .unwrap_or(base_type_name.clone());
                         let candidate = format!("{}.{}", type_prefix, method_name);
                         
                         let mut final_candidate = self.resolve_fuzzy_name(&self.decls, &candidate).unwrap_or(candidate.clone());
                         let mut found = self.decls.contains_key(&final_candidate);
                         
                         if !found {
                             if let Some(decl) = self.decls.get(&type_prefix) {
                                 if let Declaration::Struct(s) = decl {
                                     if !s.generic_params.is_empty() {
                                         let generic_candidate = format!("{}<{}>.{}", type_prefix, s.generic_params.join(", "), method_name);
                                         if let Some(full_gen_name) = self.resolve_fuzzy_name(&self.decls, &generic_candidate) {
                                             final_candidate = full_gen_name;
                                             found = true;
                                         }
                                     }
                                 }
                             }
                         }

                         if found {
                             actual_args.insert(0, receiver_expr);
                             if actual_generic_args.is_empty() { actual_generic_args = type_generic_args; }
                             actual_func_name = final_candidate;
                             is_method_call = true;
                         }
                     }
                }

                let mut compiled_args = Vec::new();
                for (i, arg) in actual_args.iter().enumerate() {
                    let val = if i == 0 && is_method_call {
                        self.compile_lvalue(arg, variables, function).map(|p| p.into())
                            .unwrap_or_else(|_| {
                                let v = self.compile_expr(arg, variables, function).unwrap();
                                let alloca = self.builder.build_alloca(v.get_type(), "temp_method_rec").unwrap();
                                self.builder.build_store(alloca, v).unwrap();
                                alloca.into()
                            })
                    } else {
                        self.compile_expr(arg, variables, function)?
                    };
                    compiled_args.push(val.into());
                }

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
                    
                    if !actual_generic_args.is_empty() {
                        let gen_name = format!("{}_{}", full_name, actual_generic_args.join("_"));
                        if let Some(existing) = self.module.get_function(&gen_name) { existing }
                        else { self.instantiate_function(&full_name, &actual_generic_args)? }
                    } else {
                        self.module.get_function(&llvm_name).ok_or(format!("Function '{}' not found (LLVM name: {}) [Generic args: {:?}]", actual_func_name, llvm_name, actual_generic_args))?
                    }
                };

                let call = if fn_val.get_type().get_return_type().is_none() {
                    self.builder.build_call(fn_val, &compiled_args, "").unwrap()
                } else {
                    self.builder.build_call(fn_val, &compiled_args, "calltmp").unwrap()
                };
                
                match call.try_as_basic_value() {
                    ValueKind::Basic(val) => Ok(val),
                    ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                }
            },
            Expression::Infix { left, operator, right } => {
                if operator.kind == TokenKind::And {
                    let lhs = self.compile_expr(left, variables, function)?;
                    let lhs_int = match lhs {
                        BasicValueEnum::IntValue(i) => i,
                        _ => return Err(format!("Expected boolean (integer) for && operator, found {:?}", lhs)),
                    };
                    
                    let rhs_bb = self.context.append_basic_block(function, "and_rhs");
                    let merge_bb = self.context.append_basic_block(function, "and_merge");
                    
                    let cond = self.builder.build_int_compare(IntPredicate::NE, lhs_int, i64_type.const_zero(), "and_cond").unwrap();
                    self.builder.build_conditional_branch(cond, rhs_bb, merge_bb).unwrap();
                    
                    let lhs_final_bb = self.builder.get_insert_block().unwrap();
                    
                    self.builder.position_at_end(rhs_bb);
                    let rhs = self.compile_expr(right, variables, function)?;
                    let rhs_int = match rhs {
                        BasicValueEnum::IntValue(i) => i,
                        _ => return Err(format!("Expected boolean (integer) for && operator, found {:?}", rhs)),
                    };
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                    let rhs_final_bb = self.builder.get_insert_block().unwrap();
                    
                    self.builder.position_at_end(merge_bb);
                    let phi = self.builder.build_phi(i64_type, "and_res").unwrap();
                    phi.add_incoming(&[
                        (&i64_type.const_zero(), lhs_final_bb),
                        (&rhs_int, rhs_final_bb),
                    ]);
                    return Ok(phi.as_basic_value().into());
                }
                
                if operator.kind == TokenKind::Or {
                    let lhs = self.compile_expr(left, variables, function)?;
                    let lhs_int = match lhs {
                        BasicValueEnum::IntValue(i) => i,
                        _ => return Err(format!("Expected boolean (integer) for || operator, found {:?}", lhs)),
                    };
                    
                    let rhs_bb = self.context.append_basic_block(function, "or_rhs");
                    let merge_bb = self.context.append_basic_block(function, "or_merge");
                    
                    let cond = self.builder.build_int_compare(IntPredicate::NE, lhs_int, i64_type.const_zero(), "or_cond").unwrap();
                    self.builder.build_conditional_branch(cond, merge_bb, rhs_bb).unwrap();
                    
                    let lhs_final_bb = self.builder.get_insert_block().unwrap();
                    
                    self.builder.position_at_end(rhs_bb);
                    let rhs = self.compile_expr(right, variables, function)?;
                    let rhs_int = match rhs {
                        BasicValueEnum::IntValue(i) => i,
                        _ => return Err(format!("Expected boolean (integer) for || operator, found {:?}", rhs)),
                    };
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                    let rhs_final_bb = self.builder.get_insert_block().unwrap();
                    
                    self.builder.position_at_end(merge_bb);
                    let phi = self.builder.build_phi(i64_type, "or_res").unwrap();
                    phi.add_incoming(&[
                        (&i64_type.const_int(1, false), lhs_final_bb),
                        (&rhs_int, rhs_final_bb),
                    ]);
                    return Ok(phi.as_basic_value().into());
                }

                let lhs = self.compile_expr(left, variables, function)?;
                let rhs = self.compile_expr(right, variables, function)?;
                if lhs.is_int_value() && rhs.is_int_value() {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    match &operator.kind {
                        TokenKind::Plus => Ok(self.builder.build_int_add(l, r, "addtmp").unwrap().into()),
                        TokenKind::Minus => Ok(self.builder.build_int_sub(l, r, "subtmp").unwrap().into()),
                        TokenKind::Star => Ok(self.builder.build_int_mul(l, r, "multmp").unwrap().into()),
                        TokenKind::Slash => Ok(self.builder.build_int_signed_div(l, r, "divtmp").unwrap().into()),
                        TokenKind::Percent => Ok(self.builder.build_int_signed_rem(l, r, "remtmp").unwrap().into()),
                        TokenKind::And => Ok(self.builder.build_and(l, r, "andtmp").unwrap().into()),
                        TokenKind::Or => Ok(self.builder.build_or(l, r, "ortmp").unwrap().into()),
                        TokenKind::EqEq => {
                            let res = self.builder.build_int_compare(IntPredicate::EQ, l, r, "eqtmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        TokenKind::NotEq => {
                            let res = self.builder.build_int_compare(IntPredicate::NE, l, r, "netmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        TokenKind::Gt => {
                            let res = self.builder.build_int_compare(IntPredicate::SGT, l, r, "gttmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        TokenKind::Lt => {
                            let res = self.builder.build_int_compare(IntPredicate::SLT, l, r, "lttmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        TokenKind::GtEq => {
                            let res = self.builder.build_int_compare(IntPredicate::SGE, l, r, "getmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        TokenKind::LtEq => {
                            let res = self.builder.build_int_compare(IntPredicate::SLE, l, r, "letmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        TokenKind::Caret => Ok(self.builder.build_xor(l, r, "xortmp").unwrap().into()),
                        _ => Err(format!("Integer operator {:?} not supported", operator)),
                    }
                } else if lhs.is_float_value() && rhs.is_float_value() {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    match &operator.kind {
                        TokenKind::Plus => Ok(self.builder.build_float_add(l, r, "faddtmp").unwrap().into()),
                        TokenKind::Minus => Ok(self.builder.build_float_sub(l, r, "fsubtmp").unwrap().into()),
                        TokenKind::Star => Ok(self.builder.build_float_mul(l, r, "fmultmp").unwrap().into()),
                        TokenKind::Slash => Ok(self.builder.build_float_div(l, r, "fdivtmp").unwrap().into()),
                        TokenKind::Caret => {
                            let pow = self.module.get_function("pow").unwrap();
                            let res = self.builder.build_call(pow, &[l.into(), r.into()], "powtmp").unwrap();
                            match res.try_as_basic_value() {
                                ValueKind::Basic(val) => Ok(val),
                                ValueKind::Instruction(_) => Err("pow must return a value".to_string()),
                            }
                        },
                        _ => Err(format!("Float operator {:?} not supported", operator)),
                    }
                } else if lhs.is_pointer_value() && rhs.is_pointer_value() {
                    let l = lhs.into_pointer_value();
                    let r = rhs.into_pointer_value();
                    match &operator.kind {
                        TokenKind::EqEq => {
                            let res = self.builder.build_int_compare(IntPredicate::EQ, l, r, "ptreqtmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        TokenKind::NotEq => {
                            let res = self.builder.build_int_compare(IntPredicate::NE, l, r, "ptrnetmp").unwrap();
                            Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                        },
                        TokenKind::Plus => {
                            let fn_val = self.module.get_function("aion_str_concat").ok_or("aion_str_concat not found")?;
                            let call = self.builder.build_call(fn_val, &[l.into(), r.into()], "strcattmp").unwrap();
                            match call.try_as_basic_value() { ValueKind::Basic(val) => Ok(val), _ => Ok(i64_type.const_zero().into()) }
                        },
                        _ => Err(format!("Pointer operator {:?} not supported", operator)),
                    }
                } else {
                    Err(format!("Mismatched types for operator {:?}", operator))
                }
            },
            Expression::If { condition, then_branch, else_branch } => {
                let cond_val = self.compile_expr(condition, variables, function)?.into_int_value();
                let comparison = self.builder.build_int_compare(IntPredicate::NE, cond_val, self.context.i64_type().const_int(0, false), "ifcond").unwrap();
                let then_bb = self.context.append_basic_block(function, "then");
                let else_bb = self.context.append_basic_block(function, "else");
                let merge_bb = self.context.append_basic_block(function, "ifcont");
                self.builder.build_conditional_branch(comparison, then_bb, else_bb).unwrap();
                
                let mut phi_entries = Vec::new();
                self.builder.position_at_end(then_bb);
                let mut then_vars = variables.clone();
                let then_val = self.compile_block(then_branch, &mut then_vars, function)?;
                if let Some(val) = then_val {
                    phi_entries.push((val, self.builder.get_insert_block().unwrap()));
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }
                
                self.builder.position_at_end(else_bb);
                let mut else_vars = variables.clone();
                let else_val = if let Some(eb) = else_branch { 
                    self.compile_block(eb, &mut else_vars, function)?
                } else { None };
                if let Some(val) = else_val {
                    phi_entries.push((val, self.builder.get_insert_block().unwrap()));
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }
                
                self.builder.position_at_end(merge_bb);
                if !phi_entries.is_empty() {
                    let first_val = phi_entries[0].0;
                    let phi = self.builder.build_phi(first_val.get_type(), "ifres").unwrap();
                    for (val, bb) in phi_entries {
                        phi.add_incoming(&[(&val, bb)]);
                    }
                    Ok(phi.as_basic_value())
                } else {
                    Ok(i64_type.const_int(0, false).into())
                }
            },
            Expression::Block { statements, .. } => {
                let mut local_vars = variables.clone();
                let val = self.compile_block(statements, &mut local_vars, function)?;
                Ok(val.unwrap_or(i64_type.const_int(0, false).into()))
            },
            Expression::StructInst { name, generic_args: _, fields } => {
                let full_name = self.resolve_fuzzy_name(&self.struct_types, name).ok_or(format!("Struct '{}' not found", name))?;
                let struct_type = *self.struct_types.get(&full_name).ok_or(format!("Struct '{}' not found in struct_types", full_name))?;
                let alloca = self.builder.build_alloca(struct_type, &format!("{}_inst", name)).unwrap();
                let field_map = self.struct_fields.get(&full_name).ok_or(format!("Field map for struct '{}' not found", full_name))?.clone();
                for (f_name, f_expr) in fields {
                    let f_val = self.compile_expr(f_expr, variables, function)?;
                    let f_idx = *field_map.get(f_name).ok_or(format!("Field '{}' not found in struct '{}'", f_name, full_name))?;
                    let f_ptr = self.builder.build_struct_gep(struct_type, alloca, f_idx, &format!("field_{}", f_name)).unwrap();
                    self.builder.build_store(f_ptr, f_val).unwrap();
                }
                Ok(self.builder.build_load(struct_type, alloca, "structtmp").unwrap())
            },
            Expression::EnumInst { name, variant, generic_args, arguments } => {
                let full_name = self.resolve_fuzzy_name(&self.enum_types, name).unwrap_or(name.clone());
                let mut real_variant_tag = None;
                if let Some(Declaration::Enum(e)) = self.decls.get(&full_name) {
                    for (idx, v) in e.variants.iter().enumerate() {
                        if v.name == *variant {
                            real_variant_tag = Some(idx as u64);
                            break;
                        }
                    }
                }
                
                if let Some(tag_val) = real_variant_tag {
                    let enum_type = *self.enum_types.get(&full_name).ok_or(format!("Enum type not found: {}", full_name))?;
                    let alloca = self.builder.build_alloca(enum_type, &format!("{}_inst", name)).unwrap();
                    let tag_ptr = self.builder.build_struct_gep(enum_type, alloca, 0, "tagptr").unwrap();
                    self.builder.build_store(tag_ptr, self.context.i64_type().const_int(tag_val, false)).unwrap();
                    if !arguments.is_empty() {
                        let data_val = self.compile_expr(&arguments[0], variables, function)?;
                        let data_ptr = self.builder.build_struct_gep(enum_type, alloca, 1, "dataptr").unwrap();
                        let casted_ptr = self.builder.build_bit_cast(data_ptr, self.context.ptr_type(AddressSpace::default()), "datacast").unwrap();
                        self.builder.build_store(casted_ptr.into_pointer_value(), data_val).unwrap();
                    }
                    Ok(self.builder.build_load(enum_type, alloca, "enumtmp").unwrap())
                } else {
                    // Fallback to static method call: Type::method()
                    let func_name = format!("{}.{}", full_name, variant);
                    self.compile_expr(&Expression::Call { function: func_name, generic_args: generic_args.clone(), arguments: arguments.clone() }, variables, function)
                }
            },
            Expression::MemberAccess { receiver, member } => {
                let rec_type_name = self.get_expr_type_name(receiver, variables);
                let base_type = if rec_type_name.contains('<') { rec_type_name.split('<').next().unwrap() } else { &rec_type_name };
                let full_type = self.resolve_fuzzy_name(&self.struct_types, base_type).unwrap_or(base_type.to_string());
                let rec_ptr = if let Ok(ptr) = self.compile_lvalue(receiver, variables, function) { ptr } else {
                    let rec_val = self.compile_expr(receiver, variables, function)?;
                    let alloca = self.builder.build_alloca(rec_val.get_type(), "temp_member_rec").unwrap();
                    self.builder.build_store(alloca, rec_val).unwrap();
                    alloca
                };
                if let Some(Declaration::Struct(s)) = self.decls.get(&full_type) {
                    let mut idx = 0;
                    for (f_name, f_type_name) in &s.fields {
                        if f_name == member {
                            let field_ptr = self.builder.build_struct_gep(*self.struct_types.get(&full_type).ok_or(format!("Struct type not found: {}", full_type))?, rec_ptr, idx as u32, member).unwrap();
                            let llvm_field_type = self.aion_type_to_llvm(f_type_name);
                            return Ok(self.builder.build_load(llvm_field_type, field_ptr, member).unwrap());
                        }
                        idx += 1;
                    }
                }
                Err(format!("Field '{}' not found on type '{}'", member, rec_type_name))
            },
            Expression::MethodCall { receiver, method, generic_args, arguments } => {
                // 1. Static call handler (e.g. Type<T>::method())
                if let Expression::TypeRef { name: ref n, generic_args: ref type_gen_args } = **receiver {
                    let mut combined_gen_args = type_gen_args.clone();
                    combined_gen_args.extend(generic_args.clone());
                    let func_name = format!("{}.{}", n, method);
                    return self.compile_expr(&Expression::Call { function: func_name, generic_args: combined_gen_args, arguments: arguments.clone() }, variables, function);
                }

                // 2. Get receiver type and handle special cases
                let rec_type_name = self.get_expr_type_name(receiver, variables);
                
                // Pointer offset handler: ptr.offset(idx)
                if rec_type_name.starts_with('*') && method == "offset" && arguments.len() == 1 {
                    let rec_val = self.compile_expr(receiver, variables, function)?;
                    let idx = self.compile_expr(&arguments[0], variables, function)?.into_int_value();
                    let ptr = rec_val.into_pointer_value();
                    let elem_type_name = &rec_type_name[1..];
                    let elem_type = self.aion_type_to_llvm(elem_type_name);
                    let gep = unsafe { self.builder.build_gep(elem_type, ptr, &[idx], "offset_ptr").unwrap() };
                    return Ok(gep.into());
                }

                // 3. Resolve method on receiver type
                let (base_type_name, type_generic_args) = if rec_type_name.contains('<') {
                    let parts: Vec<&str> = rec_type_name.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                    (parts[0].to_string(), parts[1..].iter().map(|s| s.trim().to_string()).collect::<Vec<String>>())
                } else { (rec_type_name.clone(), vec![]) };
                
                let type_prefix = self.resolve_fuzzy_name(&self.struct_types, &base_type_name)
                    .or_else(|| self.resolve_fuzzy_name(&self.enum_types, &base_type_name))
                    .unwrap_or(base_type_name.clone());
                
                let candidate = format!("{}.{}", type_prefix, method);
                let full_method_name = self.resolve_fuzzy_name(&self.decls, &candidate).unwrap_or(candidate);
                
                let mut combined_gen_args = type_generic_args;
                combined_gen_args.extend(generic_args.clone());

                // 4. Prepare and compile arguments
                let mut compiled_args = Vec::new();
                
                // Compile receiver as 'self' (passed by pointer for structs/enums)
                let rec_val = self.compile_expr(receiver, variables, function)?;
                let pass_by_ptr = self.struct_types.contains_key(&type_prefix) || self.enum_types.contains_key(&type_prefix);
                
                if pass_by_ptr {
                    if let Ok(ptr) = self.compile_lvalue(receiver, variables, function) {
                        compiled_args.push(ptr.into());
                    } else {
                        let alloca = self.builder.build_alloca(rec_val.get_type(), "temp_method_chain_rec").unwrap();
                        self.builder.build_store(alloca, rec_val).unwrap();
                        compiled_args.push(alloca.into());
                    }
                } else {
                    compiled_args.push(rec_val.into());
                }

                for arg in arguments {
                    compiled_args.push(self.compile_expr(arg, variables, function)?.into());
                }

                // 5. Instantiate and call
                let fn_val = if !combined_gen_args.is_empty() {
                    let gen_name = format!("{}_{}", full_method_name, combined_gen_args.join("_"));
                    if let Some(existing) = self.module.get_function(&gen_name) { existing }
                    else { self.instantiate_function(&full_method_name, &combined_gen_args)? }
                } else {
                    self.module.get_function(&full_method_name).ok_or(format!("Method '{}' not found on type '{}'", method, rec_type_name))?
                };

                let call = if fn_val.get_type().get_return_type().is_none() {
                    self.builder.build_call(fn_val, &compiled_args, "").unwrap()
                } else {
                    self.builder.build_call(fn_val, &compiled_args, "calltmp").unwrap()
                };
                
                Ok(match call.try_as_basic_value() {
                    ValueKind::Basic(val) => val,
                    _ => self.context.i64_type().const_zero().into(),
                })
            },
            Expression::Cast { expr, target } => {
                let val = self.compile_expr(expr, variables, function)?;
                let dest_type = self.aion_type_to_llvm(target);
                
                if val.is_int_value() && dest_type.is_int_type() {
                    let src_width = val.into_int_value().get_type().get_bit_width();
                    let dest_width = dest_type.into_int_type().get_bit_width();
                    if src_width < dest_width {
                        return Ok(self.builder.build_int_z_extend(val.into_int_value(), dest_type.into_int_type(), "cast").unwrap().into());
                    } else if src_width > dest_width {
                        return Ok(self.builder.build_int_truncate(val.into_int_value(), dest_type.into_int_type(), "cast").unwrap().into());
                    } else {
                        return Ok(val);
                    }
                }

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
            Expression::TypeRef { .. } => Ok(i64_type.const_zero().into()),
            Expression::Deref { expr } => {
                let val = self.compile_expr(expr, variables, function)?;
                let type_name = self.get_expr_type_name(expr, variables);
                let elem_type_name = if type_name.starts_with('*') { &type_name[1..] } else { "i64" };
                let llvm_elem_type = self.aion_type_to_llvm(elem_type_name);
                if val.is_pointer_value() {
                    let ptr = val.into_pointer_value();
                    Ok(self.builder.build_load(llvm_elem_type, ptr, "deref").unwrap())
                } else { Err("Dereference of non-pointer".to_string()) }
            },
            Expression::Intrinsic { name, arguments } => {
                if name == "io_println" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "io_println")) {
                    let fn_val = self.module.get_function("aion_io_println").ok_or("aion_io_println not found")?;
                    let arg_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let mut arg = self.compile_expr(arg_expr, variables, function)?;
                    if !arg.get_type().is_pointer_type() {
                        let int_to_str = self.module.get_function("aion_int_to_str").unwrap();
                        let call = self.builder.build_call(int_to_str, &[arg.into()], "strtmp").unwrap();
                        arg = match call.try_as_basic_value() { ValueKind::Basic(val) => val, _ => return Err("int_to_str failed".to_string()) };
                    }
                    self.builder.build_call(fn_val, &[arg.into()], "").unwrap();
                    Ok(i64_type.const_int(0, false).into())
                } else if name == "io_print" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "io_print")) {
                    let fn_val = self.module.get_function("aion_io_print").ok_or("aion_io_print not found")?;
                    let arg_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let mut arg = self.compile_expr(arg_expr, variables, function)?;
                    if !arg.get_type().is_pointer_type() {
                        let int_to_str = self.module.get_function("aion_int_to_str").unwrap();
                        let call = self.builder.build_call(int_to_str, &[arg.into()], "strtmp").unwrap();
                        arg = match call.try_as_basic_value() { ValueKind::Basic(val) => val, _ => return Err("int_to_str failed".to_string()) };
                    }
                    self.builder.build_call(fn_val, &[arg.into()], "").unwrap();
                    Ok(i64_type.const_int(0, false).into())
                } else if name == "io_read_line" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "io_read_line")) {
                    Ok(self.context.ptr_type(AddressSpace::default()).const_null().into())
                } else if name == "sizeof" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "sizeof")) {
                    let type_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let type_name = if let Expression::Identifier(n) = type_expr { n.clone() }
                        else if let Expression::TypeRef { name, generic_args } = type_expr {
                            if generic_args.is_empty() { name.clone() }
                            else { format!("{}<{}>", name, generic_args.join(", ")) }
                        }
                        else { return Err("sizeof requires type identifier".to_string()); };
                    let llvm_type = self.aion_type_to_llvm(&type_name);
                    let size = llvm_type.size_of().ok_or(format!("Could not determine size of '{}'", type_name))?;
                    Ok(size.into())
                } else if name == "extract_tag" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "extract_tag")) {
                    let enum_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let enum_val = self.compile_expr(enum_expr, variables, function)?;
                    if enum_val.is_struct_value() {
                        let ev = enum_val.into_struct_value();
                        let alloca = self.builder.build_alloca(ev.get_type(), "temp_enum_tag").unwrap();
                        self.builder.build_store(alloca, ev).unwrap();
                        let tag_ptr = self.builder.build_struct_gep(ev.get_type(), alloca, 0, "tagptr").unwrap();
                        Ok(self.builder.build_load(self.context.i64_type(), tag_ptr, "tag").unwrap())
                    } else { Ok(i64_type.const_int(999, false).into()) }
                } else if name == "str_ptr" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "str_ptr")) {
                    let arg = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    Ok(arg)
                } else if name == "int_to_str" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "int_to_str")) {
                    let fn_val = self.module.get_function("aion_int_to_str").ok_or("aion_int_to_str not found")?;
                    let arg = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    let call = self.builder.build_call(fn_val, &[arg.into()], "intstrtmp").unwrap();
                    match call.try_as_basic_value() { ValueKind::Basic(val) => Ok(val), _ => Ok(i64_type.const_int(0, false).into()) }
                } else if name == "float_to_str" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "float_to_str")) {
                    let fn_val = self.module.get_function("aion_float_to_str").ok_or("aion_float_to_str not found")?;
                    let arg = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    let call = self.builder.build_call(fn_val, &[arg.into()], "floatstrtmp").unwrap();
                    match call.try_as_basic_value() { ValueKind::Basic(val) => Ok(val), _ => Ok(i64_type.const_int(0, false).into()) }
                } else if name == "str_eq" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "str_eq")) {
                    let fn_val = self.module.get_function("aion_str_eq").ok_or("aion_str_eq not found")?;
                    let arg1 = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    let arg2 = self.compile_expr(if name == "intrinsic" { &arguments[2] } else { &arguments[1] }, variables, function)?;
                    let call = self.builder.build_call(fn_val, &[arg1.into(), arg2.into()], "streqtmp").unwrap();
                    match call.try_as_basic_value() { ValueKind::Basic(val) => Ok(val), _ => Ok(i64_type.const_int(0, false).into()) }
                } else if name == "str_len" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "str_len")) {
                    let strlen = self.module.get_function("strlen").ok_or("strlen not found")?;
                    let arg = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    let call = self.builder.build_call(strlen, &[arg.into()], "strlentmp").unwrap();
                    match call.try_as_basic_value() { ValueKind::Basic(val) => Ok(val), _ => Ok(i64_type.const_int(0, false).into()) }
                } else if name == "str_concat" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "str_concat")) {
                    let strcat = self.module.get_function("aion_str_concat").ok_or("aion_str_concat not found")?;
                    let arg1 = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    let arg2 = self.compile_expr(if name == "intrinsic" { &arguments[2] } else { &arguments[1] }, variables, function)?;
                    let call = self.builder.build_call(strcat, &[arg1.into(), arg2.into()], "strcattmp").unwrap();
                    match call.try_as_basic_value() { ValueKind::Basic(val) => Ok(val), _ => Ok(i64_type.const_int(0, false).into()) }
                } else if name == "fs_read_to_string" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "fs_read_to_string")) {
                    let read_fn = self.module.get_function("aion_read_file").ok_or("aion_read_file not found")?;
                    let arg = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    let call = self.builder.build_call(read_fn, &[arg.into()], "readtmp").unwrap();
                    match call.try_as_basic_value() { ValueKind::Basic(val) => Ok(val), _ => Ok(i64_type.const_int(0, false).into()) }
                } else if name == "fs_write" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "fs_write")) {
                    let write_fn = self.module.get_function("aion_write_file").ok_or("aion_write_file not found")?;
                    let arg1 = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    let arg2 = self.compile_expr(if name == "intrinsic" { &arguments[2] } else { &arguments[1] }, variables, function)?;
                    let call = self.builder.build_call(write_fn, &[arg1.into(), arg2.into()], "writetmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => {
                            if val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 32 {
                                Ok(self.builder.build_int_s_extend(val.into_int_value(), self.context.i64_type(), "i32toi64").unwrap().into())
                            } else { Ok(val) }
                        },
                        _ => Ok(i64_type.const_int(0, false).into())
                    }
                } else if name == "fs_exists" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "fs_exists")) {
                    let exists_fn = self.module.get_function("aion_fs_exists").ok_or("aion_fs_exists not found")?;
                    let arg = self.compile_expr(if name == "intrinsic" { &arguments[1] } else { &arguments[0] }, variables, function)?;
                    let call = self.builder.build_call(exists_fn, &[arg.into()], "existstmp").unwrap();
                    match call.try_as_basic_value() {
                        ValueKind::Basic(val) => {
                            if val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 32 {
                                Ok(self.builder.build_int_z_extend(val.into_int_value(), self.context.i64_type(), "boolcast").unwrap().into())
                            } else { Ok(val) }
                        },
                        _ => Ok(i64_type.const_int(0, false).into())
                    }
                } else if name == "mem_is_null" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "mem_is_null")) {
                    let ptr_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let arg = self.compile_expr(ptr_expr, variables, function)?;
                    if arg.is_pointer_value() {
                        let ptr = arg.into_pointer_value();
                        let ptr_int = self.builder.build_ptr_to_int(ptr, self.context.i64_type(), "ptrtoint").unwrap();
                        let eq_zero = self.builder.build_int_compare(IntPredicate::EQ, ptr_int, self.context.i64_type().const_zero(), "isnull").unwrap();
                        Ok(self.builder.build_int_z_extend(eq_zero, self.context.i64_type(), "boolcast").unwrap().into())
                    } else { Ok(i64_type.const_int(0, false).into()) }
                } else if name == "ai_tensor_zeros" || name == "ai_tensor_ones" || name == "ai_tensor_rand" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "ai_tensor_zeros" || s == "ai_tensor_ones" || s == "ai_tensor_rand")) {
                    let actual_name = if name == "intrinsic" { match &arguments[0] { Expression::String(s) => s.clone(), _ => "".to_string() } } else { name.clone() };
                    let shape_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let shape_val = self.compile_expr(shape_expr, variables, function)?;
                    
                    let tensor_type_name = "std.ai.tensor.Tensor";
                    let full_tensor_type_name = self.resolve_fuzzy_name(&self.struct_types, tensor_type_name).unwrap_or(tensor_type_name.to_string());
                    let tensor_type = *self.struct_types.get(&full_tensor_type_name).ok_or(format!("Tensor type not found: {}", full_tensor_type_name))?;
                    
                    let fn_name = match actual_name.as_str() {
                        "ai_tensor_zeros" => "aion_ai_tensor_zeros",
                        "ai_tensor_ones" => "aion_ai_tensor_ones",
                        _ => "aion_ai_tensor_rand",
                    };
                    let fn_val = self.module.get_function(fn_name).unwrap_or_else(|| {
                        self.module.add_function(fn_name, tensor_type.fn_type(&[self.context.ptr_type(AddressSpace::default()).into()], false), None)
                    });
                    
                    let shape_ptr = if let Ok(ptr) = self.compile_lvalue(shape_expr, variables, function) {
                        ptr.into()
                    } else {
                        let alloca = self.builder.build_alloca(shape_val.get_type(), "temp_tensor_shape").unwrap();
                        self.builder.build_store(alloca, shape_val).unwrap();
                        alloca.into()
                    };

                    let call = self.builder.build_call(fn_val, &[shape_ptr], "tensortmp").unwrap();
                    Ok(match call.try_as_basic_value() {
                        ValueKind::Basic(val) => val,
                        _ => return Err("Tensor intrinsic failed to return a value".to_string()),
                    })
                } else if name == "ai_tensor_backward" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "ai_tensor_backward")) {
                    let tensor_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let tensor_val = self.compile_expr(tensor_expr, variables, function)?;
                    
                    let fn_name = "aion_ai_tensor_backward";
                    let fn_val = self.module.get_function(fn_name).unwrap_or_else(|| {
                        self.module.add_function(fn_name, self.context.void_type().fn_type(&[self.context.ptr_type(AddressSpace::default()).into()], false), None)
                    });
                    
                    let tensor_ptr = if let Ok(ptr) = self.compile_lvalue(tensor_expr, variables, function) {
                        ptr.into()
                    } else {
                        let alloca = self.builder.build_alloca(tensor_val.get_type(), "temp_tensor_backward").unwrap();
                        self.builder.build_store(alloca, tensor_val).unwrap();
                        alloca.into()
                    };

                    self.builder.build_call(fn_val, &[tensor_ptr], "").unwrap();
                    Ok(i64_type.const_zero().into())
                } else if name == "ai_tensor_move" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "ai_tensor_move")) {
                    let tensor_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let device_expr = if name == "intrinsic" { &arguments[2] } else { &arguments[1] };
                    let tensor_val = self.compile_expr(tensor_expr, variables, function)?;
                    let device_val = self.compile_expr(device_expr, variables, function)?;
                    
                    let fn_name = "aion_ai_tensor_move";
                    let fn_val = self.module.get_function(fn_name).unwrap_or_else(|| {
                        self.module.add_function(fn_name, self.context.void_type().fn_type(&[self.context.ptr_type(AddressSpace::default()).into(), self.context.ptr_type(AddressSpace::default()).into()], false), None)
                    });
                    
                    let tensor_ptr = if let Ok(ptr) = self.compile_lvalue(tensor_expr, variables, function) {
                        ptr.into()
                    } else {
                        let alloca = self.builder.build_alloca(tensor_val.get_type(), "temp_tensor_move").unwrap();
                        self.builder.build_store(alloca, tensor_val).unwrap();
                        alloca.into()
                    };

                    self.builder.build_call(fn_val, &[tensor_ptr, device_val.into()], "").unwrap();
                    Ok(i64_type.const_zero().into())
                } else if name == "ai_tensor_matmul" || name == "ai_tensor_add" || (name == "intrinsic" && matches!(&arguments[0], Expression::String(s) if s == "ai_tensor_matmul" || s == "ai_tensor_add")) {
                    let actual_name = if name == "intrinsic" { match &arguments[0] { Expression::String(s) => s.clone(), _ => "".to_string() } } else { name.clone() };
                    let t1_expr = if name == "intrinsic" { &arguments[1] } else { &arguments[0] };
                    let t2_expr = if name == "intrinsic" { &arguments[2] } else { &arguments[1] };
                    let t1_val = self.compile_expr(t1_expr, variables, function)?;
                    let t2_val = self.compile_expr(t2_expr, variables, function)?;
                    
                    let tensor_type_name = "std.ai.tensor.Tensor";
                    let full_tensor_type_name = self.resolve_fuzzy_name(&self.struct_types, tensor_type_name).unwrap_or(tensor_type_name.to_string());
                    let tensor_type = *self.struct_types.get(&full_tensor_type_name).ok_or(format!("Tensor type not found: {}", full_tensor_type_name))?;

                    let fn_name = if actual_name == "ai_tensor_matmul" { "aion_ai_tensor_matmul" } else { "aion_ai_tensor_add" };
                    let fn_val = self.module.get_function(fn_name).unwrap_or_else(|| {
                        self.module.add_function(fn_name, tensor_type.fn_type(&[self.context.ptr_type(AddressSpace::default()).into(), self.context.ptr_type(AddressSpace::default()).into()], false), None)
                    });
                    
                    let t1_ptr = if let Ok(ptr) = self.compile_lvalue(t1_expr, variables, function) {
                        ptr.into()
                    } else {
                        let alloca = self.builder.build_alloca(t1_val.get_type(), "temp_tensor_op_t1").unwrap();
                        self.builder.build_store(alloca, t1_val).unwrap();
                        alloca.into()
                    };
                    let t2_ptr = if let Ok(ptr) = self.compile_lvalue(t2_expr, variables, function) {
                        ptr.into()
                    } else {
                        let alloca = self.builder.build_alloca(t2_val.get_type(), "temp_tensor_op_t2").unwrap();
                        self.builder.build_store(alloca, t2_val).unwrap();
                        alloca.into()
                    };

                    let call = self.builder.build_call(fn_val, &[t1_ptr, t2_ptr], "tensortmp").unwrap();
                    Ok(match call.try_as_basic_value() {
                        ValueKind::Basic(val) => val,
                        _ => return Err("Tensor operation intrinsic failed to return a value".to_string()),
                    })
                } else {
                    let (intrinsic_name, intrinsic_args) = if name == "intrinsic" {
                        if let Expression::String(actual_name) = &arguments[0] { (actual_name.clone(), arguments[1..].to_vec()) } else { (name.clone(), arguments.clone()) }
                    } else { (name.clone(), arguments.clone()) };
                    let fn_val = self.module.get_function(&intrinsic_name).ok_or(format!("Intrinsic '{}' not found", intrinsic_name))?;
                    let mut compiled_args = Vec::new();
                    for arg in &intrinsic_args { compiled_args.push(self.compile_expr(arg, variables, function)?.into()); }
                    let call = if fn_val.get_type().get_return_type().is_none() { self.builder.build_call(fn_val, &compiled_args, "").unwrap() }
                        else { self.builder.build_call(fn_val, &compiled_args, "intrinsictmp").unwrap() };
                    match call.try_as_basic_value() { ValueKind::Basic(val) => Ok(val), _ => Ok(i64_type.const_int(0, false).into()) }
                }
            },
            Expression::Range { start, .. } => { self.compile_expr(start, variables, function) },
        }
    }

    pub fn print_to_file(&self, path: &Path) -> Result<(), String> {
        self.module.print_to_file(path).map_err(|e| e.to_string())
    }
}

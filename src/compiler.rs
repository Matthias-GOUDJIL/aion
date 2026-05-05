use std::path::Path;
use std::collections::{HashMap, HashSet};
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, PointerValue, FunctionValue, ValueKind, BasicValue};
use inkwell::types::{StructType, BasicTypeEnum, BasicType};
use inkwell::{AddressSpace, IntPredicate};
use inkwell::passes::{PassManager, PassManagerBuilder};
use inkwell::OptimizationLevel;
use crate::ast::*;
use crate::token::TokenKind;
use crate::error::CompileError;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub struct_types: HashMap<String, StructType<'ctx>>,
    pub struct_fields: HashMap<String, HashMap<String, u32>>,
    pub enum_types: HashMap<String, StructType<'ctx>>,
    pub decls: HashMap<String, Declaration>,
    pub compiled_instances: HashSet<String>,
    source: String,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        Self::with_source(context, module_name, "")
    }

    pub fn with_source(context: &'ctx Context, module_name: &str, source: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        let mut compiler = Self { 
            context, 
            module, 
            builder, 
            struct_types: HashMap::new(), 
            struct_fields: HashMap::new(), 
            enum_types: HashMap::new(), 
            decls: HashMap::new(), 
            compiled_instances: HashSet::new(),
            source: source.to_string(),
        };
        compiler.register_builtins();
        compiler
    }

    fn err(&self, msg: impl Into<String>, expr: &Expression) -> CompileError {
        let (line, col) = expr.span();
        CompileError::new(msg, line, col).with_snippet(&self.source)
    }

    fn register_builtins(&mut self) {
        let builtins = vec![
            ("io.println", "aion_io_println", "void", false), ("io.print", "aion_io_print", "void", false), ("io.read_line", "aion_io_read_line", "String", false),
            ("string.from_int", "aion_int_to_str", "String", false), ("string.from_float", "aion_float_to_str", "String", false),
            ("fs_read_to_string", "aion_read_file", "String", false), ("fs_write", "aion_write_file", "i32", false), ("fs_exists", "aion_fs_exists", "i64", false), ("fs_append", "aion_append_file", "i32", false),
            ("aion_getenv", "aion_getenv", "String", false), ("aion_get_argc", "aion_get_argc", "i64", false), ("aion_get_argv_index", "aion_get_argv_index", "String", false),
            ("aion_exit", "exit", "void", false), ("exit", "exit", "void", false), ("aion_malloc", "aion_malloc", "ptr", false), ("aion_realloc", "aion_realloc", "ptr", false), ("aion_free", "aion_free", "void", false),
            ("aion_str_at", "aion_str_at", "i64", false), ("aion_str_substr", "aion_str_substr", "String", false), ("aion_char_to_str", "aion_char_to_str", "String", false),
            ("ai.tensor_zeros", "aion_ai_tensor_zeros", "ptr", false), ("ai.tensor_ones", "aion_ai_tensor_ones", "ptr", false), ("ai.tensor_rand", "aion_ai_tensor_rand", "ptr", false),
            ("ai.tensor_backward", "aion_ai_tensor_backward", "void", false), ("ai.tensor_matmul", "aion_ai_tensor_matmul", "ptr", false), ("ai.tensor_add", "aion_ai_tensor_add", "ptr", false),
            ("ai.tensor_move", "aion_ai_tensor_move", "ptr", false),
            ("i64.abs", "aion_i64_abs", "i64", true), ("i64.max", "aion_i64_max", "i64", true), ("i64.min", "aion_i64_min", "i64", true),
            ("string.len", "aion_string_len", "i64", true), ("String.len", "aion_string_len", "i64", true)
        ];
        for (an, ln, rt, is_method) in builtins {
            let params = if is_method { vec![("self".to_string(), "i64".to_string(), None)] } else { vec![] };
            let d = Declaration::Function(Function { 
                name: an.to_string(), 
                generic_params: vec![], 
                params,
                return_type: rt.to_string(), 
                body: None, 
                modifiers: vec![], 
                attributes: vec![("intrinsic".to_string(), ln.to_string())] 
            });
            self.decls.insert(an.to_string(), d);
        }
    }

    fn resolve_fuzzy_name<T>(&self, map: &HashMap<String, T>, name: &str) -> Option<String> {
        let clean = name.replace(" ", ""); 
        if map.contains_key(&clean) { return Some(clean); }
        
        let p: Vec<&str> = clean.split('.').collect(); 
        let last = p.last()?.to_string(); 
        if map.contains_key(&last) { return Some(last); }
        
        for k in map.keys() { 
            if k.ends_with(&clean) || clean.ends_with(k) { return Some(k.clone()); } 
        }
        None
    }

    fn aion_type_to_llvm(&self, tn: &str) -> BasicTypeEnum<'ctx> {
        let clean = tn.replace(" ", ""); 
        if clean.starts_with('*') || clean == "ptr" || clean == "String" { 
            return self.context.ptr_type(AddressSpace::default()).into(); 
        }
        match clean.as_str() {
            "i64" | "u64" | "Integer" | "bool" | "Boolean" | "Date" | "Duration" | "void" | "Unit" => self.context.i64_type().into(),
            "i32" | "u32" => self.context.i32_type().into(), 
            "i8" | "u8" => self.context.i8_type().into(), 
            "f64" | "Float" => self.context.f64_type().into(),
            _ => self.context.ptr_type(AddressSpace::default()).into(),
        }
    }

    fn substitute_types_in_body(&self, b: &mut [Statement], ph: &[String], conc: &[String]) {
        for s in b.iter_mut() {
            match s {
                Statement::Let { value, .. } => self.substitute_types_in_expr(value, ph, conc),
                Statement::Assignment { target, value } => { 
                    self.substitute_types_in_expr(target, ph, conc); 
                    self.substitute_types_in_expr(value, ph, conc); 
                },
                Statement::Return { value, .. } => self.substitute_types_in_expr(value, ph, conc),
                Statement::ExpressionStmt(e) => self.substitute_types_in_expr(e, ph, conc),
                Statement::If { condition, then_branch, else_branch } => { 
                    self.substitute_types_in_expr(condition, ph, conc); 
                    self.substitute_types_in_body(then_branch, ph, conc); 
                    if let Some(eb) = else_branch { self.substitute_types_in_body(eb, ph, conc); } 
                },
                Statement::While { condition, body } => { 
                    self.substitute_types_in_expr(condition, ph, conc); 
                    self.substitute_types_in_body(body, ph, conc); 
                },
                Statement::For { range, body, .. } => { 
                    self.substitute_types_in_expr(range, ph, conc); 
                    self.substitute_types_in_body(body, ph, conc); 
                },
                Statement::UnsafeBlock(stmts) | Statement::Spawn(stmts) => self.substitute_types_in_body(stmts, ph, conc),
                Statement::Match { condition, arms } => { 
                    self.substitute_types_in_expr(condition, ph, conc); 
                    for arm in arms { self.substitute_types_in_body(&mut arm.body, ph, conc); } 
                },
                _ => {},
            }
        }
    }

    fn substitute_types_in_expr(&self, e: &mut Expression, ph: &[String], conc: &[String]) {
        match e {
            Expression::Infix { left, right, .. } => { 
                self.substitute_types_in_expr(left, ph, conc); 
                self.substitute_types_in_expr(right, ph, conc); 
            },
            Expression::Call { function, generic_args, arguments, .. } => { 
                for i in 0..ph.len() { 
                    *function = function.replace(&ph[i], &conc[i]); 
                    for arg in generic_args.iter_mut() { *arg = arg.replace(&ph[i], &conc[i]); } 
                } 
                for arg in arguments { self.substitute_types_in_expr(arg, ph, conc); } 
            },
            Expression::EnumInst { name, generic_args, arguments, .. } => { 
                for i in 0..ph.len() { 
                    *name = name.replace(&ph[i], &conc[i]); 
                    for arg in generic_args.iter_mut() { *arg = arg.replace(&ph[i], &conc[i]); } 
                } 
                for arg in arguments { self.substitute_types_in_expr(arg, ph, conc); } 
            },
            Expression::StructInst { name, generic_args, fields, .. } => { 
                for i in 0..ph.len() { 
                    *name = name.replace(&ph[i], &conc[i]); 
                    for arg in generic_args.iter_mut() { *arg = arg.replace(&ph[i], &conc[i]); } 
                } 
                for (_, val) in fields { self.substitute_types_in_expr(val, ph, conc); } 
            },
            Expression::Cast { expr, target } => { 
                self.substitute_types_in_expr(expr, ph, conc); 
                for i in 0..ph.len() { *target = target.replace(&ph[i], &conc[i]); } 
            },
            Expression::Deref { expr } => self.substitute_types_in_expr(expr, ph, conc),
            Expression::Intrinsic { arguments, .. } => { 
                for arg in arguments { self.substitute_types_in_expr(arg, ph, conc); } 
            },
            Expression::Block { statements, .. } => self.substitute_types_in_body(statements, ph, conc),
            Expression::Identifier(n) => { 
                for i in 0..ph.len() { *n = n.replace(&ph[i], &conc[i]); } 
            },
            Expression::MemberAccess { receiver, .. } => self.substitute_types_in_expr(receiver, ph, conc),
            Expression::MethodCall { receiver, generic_args, arguments, .. } => { 
                self.substitute_types_in_expr(receiver, ph, conc); 
                for arg in generic_args { 
                    for i in 0..ph.len() { *arg = arg.replace(&ph[i], &conc[i]); } 
                } 
                for arg in arguments { self.substitute_types_in_expr(arg, ph, conc); } 
            },
            _ => {}
        }
    }

    fn instantiate_function(&mut self, bn: &str, ga: &[String]) -> Result<FunctionValue<'ctx>, CompileError> {
        let d = self.decls.get(bn).cloned().ok_or_else(|| CompileError::Internal(format!("Generic function '{}' not found", bn)))?;
        if let Declaration::Function(mut f) = d {
            let ph = f.generic_params.clone(); 
            let nn = format!("{}_{}", bn, ga.join("_"));
            if let Some(e) = self.module.get_function(&nn) { return Ok(e); }
            
            f.name = nn.clone(); 
            f.generic_params = vec![];
            for i in 0..ph.len() { 
                let p = &ph[i]; 
                let c = &ga[i]; 
                for (_, pt, _) in f.params.iter_mut() { *pt = pt.replace(p, c); } 
                f.return_type = f.return_type.replace(p, c); 
            }
            if let Some(body) = &mut f.body { self.substitute_types_in_body(body, &ph, ga); }
            
            self.decls.insert(nn.clone(), Declaration::Function(f.clone()));
            self.compiled_instances.insert(nn.clone()); 
            self.compile_function(&Declaration::Function(f))
        } else { 
            Err(CompileError::Internal(format!("'{}' is not a function", bn))) 
        }
    }

    fn compile_function(&mut self, decl: &Declaration) -> Result<FunctionValue<'ctx>, CompileError> {
        if let Declaration::Function(f) = decl {
            let function = if let Some(e) = self.module.get_function(&f.name) { 
                if e.get_first_basic_block().is_some() { return Ok(e); } 
                e 
            } else {
                let mut pt = Vec::new(); 
                let ptr_t = self.context.ptr_type(AddressSpace::default());
                if f.name == "main" { 
                    pt.push(self.context.i32_type().into()); 
                    pt.push(ptr_t.into()); 
                } else { 
                    for (_, ptn, _) in &f.params { pt.push(self.aion_type_to_llvm(ptn).into()); } 
                }
                self.module.add_function(&f.name, self.aion_type_to_llvm(&f.return_type).fn_type(&pt, false), None)
            };
            
            if let Some(body) = &f.body {
                let pb = self.builder.get_insert_block(); 
                let bb = self.context.append_basic_block(function, "entry"); 
                self.builder.position_at_end(bb);
                let mut local_vars = HashMap::new(); 
                let i64_t = self.context.i64_type();
                
                if f.name == "main" {
                    let gc_init = self.module.get_function("GC_init").ok_or_else(|| CompileError::Internal("GC_init function not found".to_string()))?;
                    self.builder.build_call(gc_init, &[], "")?;
                    
                    if let Some(argc) = function.get_nth_param(0) { 
                        let av = self.builder.build_int_z_extend(argc.into_int_value(), i64_t, "argc_ext")?; 
                        let a = self.builder.build_alloca(i64_t, "argc")?; 
                        self.builder.build_store(a, av)?; 
                        local_vars.insert("argc".to_string(), (a, i64_t.into(), "i64".to_string())); 
                        if let Some(g) = self.module.get_global("aion_argc") { 
                            self.builder.build_store(g.as_pointer_value(), av)?; 
                        } 
                    }
                    if let Some(argv) = function.get_nth_param(1) { 
                        let a = self.builder.build_alloca(self.context.ptr_type(AddressSpace::default()), "argv")?; 
                        self.builder.build_store(a, argv)?; 
                        local_vars.insert("argv".to_string(), (a, self.context.ptr_type(AddressSpace::default()).into(), "ptr".to_string())); 
                        if let Some(g) = self.module.get_global("aion_argv") { 
                            self.builder.build_store(g.as_pointer_value(), argv)?; 
                        } 
                    }
                } else {
                    for (i, arg) in function.get_param_iter().enumerate() { 
                        if i < f.params.len() { 
                            let an = &f.params[i].0; 
                            let atn = &f.params[i].1; 
                            let default_val = &f.params[i].2;
                            arg.set_name(an); 
                            let a = self.builder.build_alloca(arg.get_type(), an)?; 
                            self.builder.build_store(a, arg)?; 
                            local_vars.insert(an.clone(), (a, arg.get_type(), atn.replace(" ", ""))); 
                            
                            // If there's a default value and the arg is zero-initialized, use the default
                            if default_val.is_some() {
                                // For now, we'll handle defaults at call site
                            }
                        } 
                    }
                }
                
                let lbv = self.compile_block(body, &mut local_vars, function)?;
                if let Some(cb) = self.builder.get_insert_block() {
                    if cb.get_terminator().is_none() {
                        let rt = function.get_type().get_return_type();
                        if let Some(mut v) = lbv {
                            if let Some(tt) = rt { 
                                if v.get_type() != tt { 
                                    if tt.is_pointer_type() && v.is_int_value() { 
                                        v = self.builder.build_int_to_ptr(v.into_int_value(), self.context.ptr_type(AddressSpace::default()), "ret_ptr")?.into(); 
                                    } else if tt.is_int_type() && v.is_pointer_value() { 
                                        v = self.builder.build_ptr_to_int(v.into_pointer_value(), i64_t, "ret_int")?.into(); 
                                    } 
                                } 
                            }
                            self.builder.build_return(Some(&v))?;
                        } else { 
                            let def: BasicValueEnum = if rt.map_or(false, |t| t.is_pointer_type()) { 
                                self.context.ptr_type(AddressSpace::default()).const_null().into() 
                            } else { 
                                i64_t.const_zero().into() 
                            }; 
                            self.builder.build_return(Some(&def))?; 
                        }
                    }
                }
                if let Some(p) = pb { self.builder.position_at_end(p); }
            }
            Ok(function)
        } else { 
            Err(CompileError::Internal("Not a function".to_string())) 
        }
    }

    pub fn compile(&mut self, program: &Program) -> Result<(), CompileError> {
        for d in &program.declarations {
            match d {
                Declaration::Function(f) => { self.decls.insert(f.name.clone(), d.clone()); },
                Declaration::Struct(s) => { self.decls.insert(s.name.clone(), d.clone()); },
                Declaration::Enum(e) => { self.decls.insert(e.name.clone(), d.clone()); },
                Declaration::Impl(i) => {
                    let mut ftn = i.target_name.clone(); 
                    if !i.generic_params.is_empty() { ftn = format!("{}<{}>", i.target_name, i.generic_params.join(",")); }
                    let bt = if i.target_name.contains('<') { i.target_name.split('<').next().ok_or_else(|| CompileError::Internal("Invalid target name".to_string()))? } else { &i.target_name };
                    for f in &i.functions { 
                        let mut nf = f.clone(); 
                        nf.name = format!("{}.{}", bt, f.name); 
                        for (_, pt, _) in nf.params.iter_mut() { if pt == "Self" { *pt = ftn.clone(); } } 
                        if nf.return_type == "Self" { nf.return_type = ftn.clone(); } 
                        let mut cg = i.generic_params.clone(); 
                        cg.extend(f.generic_params.clone()); 
                        nf.generic_params = cg; 
                        self.decls.insert(nf.name.clone(), Declaration::Function(nf)); 
                    }
                },
                _ => {}
            }
        }
        
        let pt = self.context.ptr_type(AddressSpace::default()); 
        let i64_t = self.context.i64_type();
        
        self.module.add_function("printf", self.context.i32_type().fn_type(&[pt.into()], true), None); 
        self.module.add_function("strlen", i64_t.fn_type(&[pt.into()], false), None); 
        self.module.add_function("exit", self.context.void_type().fn_type(&[self.context.i32_type().into()], false), None); 
        self.module.add_function("malloc", pt.fn_type(&[i64_t.into()], false), None); 
        self.module.add_function("realloc", pt.fn_type(&[pt.into(), i64_t.into()], false), None); 
        self.module.add_function("free", self.context.void_type().fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_io_print", self.context.void_type().fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_io_println", self.context.void_type().fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_io_read_line", pt.fn_type(&[], false), None); 
        self.module.add_function("GC_init", self.context.void_type().fn_type(&[], false), None); 
        self.module.add_function("aion_str_eq", i64_t.fn_type(&[pt.into(), pt.into()], false), None); 
        self.module.add_function("aion_str_concat", pt.fn_type(&[pt.into(), pt.into()], false), None); 
        self.module.add_function("aion_int_to_str", pt.fn_type(&[i64_t.into()], false), None); 
        self.module.add_function("aion_float_to_str", pt.fn_type(&[self.context.f64_type().into()], false), None); 
        self.module.add_function("aion_read_file", pt.fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_write_file", i64_t.fn_type(&[pt.into(), pt.into()], false), None); 
        self.module.add_function("aion_append_file", i64_t.fn_type(&[pt.into(), pt.into()], false), None); 
        self.module.add_function("aion_fs_exists", i64_t.fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_getenv", pt.fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_get_argc", i64_t.fn_type(&[], false), None); 
        self.module.add_function("aion_get_argv_index", pt.fn_type(&[i64_t.into()], false), None); 
        self.module.add_function("aion_malloc", pt.fn_type(&[i64_t.into()], false), None); 
        self.module.add_function("aion_str_at", i64_t.fn_type(&[pt.into(), i64_t.into()], false), None); 
        self.module.add_function("aion_str_substr", pt.fn_type(&[pt.into(), i64_t.into(), i64_t.into()], false), None); 
        self.module.add_function("aion_char_to_str", pt.fn_type(&[i64_t.into()], false), None);
        self.module.add_function("aion_ai_tensor_zeros", pt.fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_ai_tensor_ones", pt.fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_ai_tensor_rand", pt.fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_ai_tensor_backward", self.context.void_type().fn_type(&[pt.into()], false), None); 
        self.module.add_function("aion_ai_tensor_move", pt.fn_type(&[pt.into(), pt.into()], false), None);
        
        self.module.add_global(i64_t, Some(AddressSpace::default()), "aion_argc").set_initializer(&i64_t.const_zero()); 
        self.module.add_global(pt, Some(AddressSpace::default()), "aion_argv").set_initializer(&pt.const_null());
        
        for d in &program.declarations { 
            match d { 
                Declaration::Struct(s) => { self.struct_types.insert(s.name.clone(), self.context.opaque_struct_type(&s.name)); }, 
                Declaration::Enum(e) => { self.enum_types.insert(e.name.clone(), self.context.struct_type(&[i64_t.into(), self.context.i8_type().array_type(512).into()], false)); }, 
                _ => {} 
            } 
        }
        
        for d in &program.declarations { 
            if let Declaration::Struct(s) = d { 
                let mut fm = HashMap::new(); 
                for (i, (n, _)) in s.fields.iter().enumerate() { fm.insert(n.clone(), i as u32); } 
                self.struct_fields.insert(s.name.clone(), fm); 
                let st = *self.struct_types.get(&s.name).ok_or_else(|| CompileError::Internal(format!("Struct type '{}' not found", s.name)))?; 
                let mut ft_list = Vec::new(); 
                for (_, tnm) in &s.fields { ft_list.push(self.aion_type_to_llvm(tnm)); } 
                st.set_body(&ft_list, false); 
            } 
        }
        
        if self.resolve_fuzzy_name(&self.enum_types, "Option").is_none() { 
            self.enum_types.insert("Option".to_string(), self.context.struct_type(&[i64_t.into(), self.context.i8_type().array_type(512).into()], false)); 
        }
        
        let ad: Vec<Declaration> = self.decls.values().cloned().collect();
        for d in &ad { 
            if let Declaration::Function(f) = d { 
                if f.generic_params.is_empty() { 
                    let mut pv = Vec::new(); 
                    if f.name == "main" { pv.push(self.context.i32_type().into()); pv.push(pt.into()); } 
                    else { for (_, ptn, _) in &f.params { pv.push(self.aion_type_to_llvm(ptn).into()); } } 
                    let mut ln = f.name.clone(); 
                    for (an, av) in &f.attributes { if an == "intrinsic" { ln = av.replace("libc.", ""); break; } } 
                    self.module.add_function(&ln, self.aion_type_to_llvm(&f.return_type).fn_type(&pv, false), None); 
                } 
            } 
        }
        
        for d in &ad { 
            if let Declaration::Function(f) = d { 
                if f.generic_params.is_empty() { self.compile_function(d)?; } 
            } 
        }
        Ok(())
    }

    fn compile_block(&mut self, body: &[Statement], variables: &mut HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>, function: FunctionValue<'ctx>) -> Result<Option<BasicValueEnum<'ctx>>, CompileError> {
        let mut lv = None; 
        let i64_t = self.context.i64_type(); 
        let pt = self.context.ptr_type(AddressSpace::default());
        
        for s in body {
            match s {
                Statement::Let { name, value, .. } => {
                    let v = self.compile_expr(value, variables, function)?;
                    let vt = v.get_type();
                    let vtn = self.get_expr_type_name(value, variables).replace(" ", "");
                    let a = self.builder.build_alloca(vt, name)?;
                    self.builder.build_store(a, v)?;
                    variables.insert(name.clone(), (a, vt, vtn));
                    lv = None;
                },
                Statement::Assignment { target, value } => { 
                    let (ptr, tt) = self.compile_lvalue(target, variables, function)?; 
                    let mut v = self.compile_expr(value, variables, function)?; 
                    if tt.is_struct_type() && v.get_type().is_pointer_type() { 
                        v = self.builder.build_load(tt, v.into_pointer_value(), "ld_assign")?; 
                    } 
                    self.builder.build_store(ptr, v)?; 
                    lv = None; 
                },
                Statement::Return { value, .. } => { 
                    let mut v = self.compile_expr(value, variables, function)?; 
                    if self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?.get_terminator().is_none() { 
                        let rt = function.get_type().get_return_type();
                        if let Some(tt) = rt { 
                            if v.get_type() != tt { 
                                if tt.is_pointer_type() && v.is_int_value() { 
                                    v = self.builder.build_int_to_ptr(v.into_int_value(), pt, "ret_ptr")?.into(); 
                                } else if tt.is_int_type() && v.is_pointer_value() { 
                                    v = self.builder.build_ptr_to_int(v.into_pointer_value(), i64_t, "ret_int")?.into(); 
                                } 
                            } 
                        }
                        self.builder.build_return(Some(&v))?; 
                    } 
                    lv = Some(v); 
                },
                Statement::If { condition, then_branch, else_branch } => {
                    let cv = self.compile_expr(condition, variables, function)?.into_int_value(); 
                    let comp = self.builder.build_int_compare(IntPredicate::NE, cv, i64_t.const_int(0, false), "ifcond")?;
                    let tb = self.context.append_basic_block(function, "then"); 
                    let eb = self.context.append_basic_block(function, "else"); 
                    let mb = self.context.append_basic_block(function, "ifcont");
                    self.builder.build_conditional_branch(comp, tb, eb)?; 
                    let mut phis = Vec::new();
                    
                    self.builder.position_at_end(tb); 
                    let mut tv = variables.clone(); 
                    let tr = self.compile_block(then_branch, &mut tv, function)?; 
                    let tf = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?; 
                    if tf.get_terminator().is_none() { 
                        let v = tr.unwrap_or(i64_t.const_zero().into()); 
                        phis.push((v, tf)); 
                    }
                    
                    self.builder.position_at_end(eb); 
                    let mut ev = variables.clone(); 
                    let er = if let Some(e) = else_branch { self.compile_block(e, &mut ev, function)? } else { None }; 
                    let ef = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?; 
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
                                    v = self.builder.build_int_to_ptr(v.into_int_value(), pt, "phi_ptr")?.into(); 
                                } else if target_type.is_int_type() && v.is_pointer_value() { 
                                    v = self.builder.build_ptr_to_int(v.into_pointer_value(), i64_t, "phi_int")?.into(); 
                                }
                            }
                            self.builder.build_unconditional_branch(mb)?;
                            final_phis.push((v, b));
                        }
                        self.builder.position_at_end(mb);
                        let phi = self.builder.build_phi(target_type, "ifres")?; 
                        for (v, b) in final_phis { phi.add_incoming(&[(&v, b)]); } 
                        lv = Some(phi.as_basic_value());
                    } else { 
                        if self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?.get_terminator().is_none() { 
                            self.builder.build_unreachable()?; 
                        } 
                        lv = None; 
                    }
                },
                Statement::While { condition, body } => {
                    let cb = self.context.append_basic_block(function, "while_cond"); 
                    let bb = self.context.append_basic_block(function, "while_body"); 
                    let eb = self.context.append_basic_block(function, "while_exit");
                    self.builder.build_unconditional_branch(cb)?; 
                    self.builder.position_at_end(cb); 
                    let cv = self.compile_expr(condition, variables, function)?.into_int_value(); 
                    self.builder.build_conditional_branch(self.builder.build_int_compare(IntPredicate::NE, cv, i64_t.const_int(0, false), "loopcond")?, bb, eb)?;
                    self.builder.position_at_end(bb); 
                    let mut bvars = variables.clone(); 
                    self.compile_block(body, &mut bvars, function)?; 
                    if self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?.get_terminator().is_none() { 
                        self.builder.build_unconditional_branch(cb)?; 
                    } 
                    self.builder.position_at_end(eb); 
                    lv = None;
                },
                Statement::Match { condition, arms } => {
                    let cv = self.compile_expr(condition, variables, function)?; 
                    let exit_bb = self.context.append_basic_block(function, "matchexit"); 
                    let mut phis = Vec::new();
                    let ctn = self.get_expr_type_name(condition, variables); 
                    let cbn = if ctn.contains('<') { ctn.split('<').next().ok_or_else(|| CompileError::Internal("Invalid type name".to_string()))?.to_string() } else { ctn.clone() };
                    let fen = self.resolve_fuzzy_name(&self.enum_types, &cbn).unwrap_or(cbn.clone());
                    
                    if let Some(et_ref) = self.enum_types.get(&fen) {
                        let et = *et_ref; 
                        let ep = cv.into_pointer_value(); 
                        let tag = self.builder.build_load(i64_t, self.builder.build_struct_gep(et, ep, 0, "tagptr")?, "tag")?.into_int_value();
                        let na = arms.len();
                        for (i, arm) in arms.iter().enumerate() {
                            let ab = self.context.append_basic_block(function, &format!("arm_{}_{}", i, arm.pattern)); 
                            let is_last = i == na - 1; 
                            let nb = if is_last { exit_bb } else { self.context.append_basic_block(function, "match_next") };
                            
                            // Get all patterns to check
                            let all_patterns: Vec<String> = if arm.patterns.is_empty() {
                                vec![arm.pattern.clone()]
                            } else {
                                arm.patterns.clone()
                            };
                            
                            let is_default = all_patterns.iter().any(|p| p == "_");
                            let mut arm_match_cond: Option<inkwell::values::IntValue<'ctx>> = None;
                            
                            if !is_default {
                                if let Some(Declaration::Enum(e_decl)) = self.decls.get(&fen) {
                                    for pat in &all_patterns {
                                        let mut at = i as u64;
                                        for (vi, v) in e_decl.variants.iter().enumerate() {
                                            if pat == &v.name || pat.ends_with(&format!(".{}", v.name)) || pat.ends_with(&format!("::{}", v.name)) {
                                                at = vi as u64;
                                                break;
                                            }
                                        }
                                        // Fallback for common variants
                                        if at == i as u64 && (pat == "Some" || pat == "Ok" || pat.ends_with(".Some") || pat.ends_with("::Some")) { at = 0; }
                                        if at == i as u64 && (pat == "None" || pat == "Err" || pat.ends_with(".None") || pat.ends_with("::None")) { at = 1; }
                                        
                                        let cond = self.builder.build_int_compare(IntPredicate::EQ, tag, i64_t.const_int(at, false), "is_arm")?;
                                        arm_match_cond = Some(match arm_match_cond {
                                            Some(prev) => self.builder.build_or(prev, cond, "arm_or")?,
                                            None => cond,
                                        });
                                    }
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
                                let test_bb = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?; 
                                phis.push((i64_t.const_zero().into(), test_bb)); 
                            }
                            self.builder.position_at_end(ab); 
                            let mut av = variables.clone();
                            if !arm.params.is_empty() {
                                let dp = self.builder.build_struct_gep(et, ep, 1, "arm_dataptr")?; 
                                let mut ptn = "i64".to_string();
                                if let Some(Declaration::Enum(e_decl)) = self.decls.get(&fen) { 
                                    for v in &e_decl.variants { 
                                        if arm.pattern == v.name || arm.pattern.ends_with(&format!(".{}", v.name)) || arm.pattern.ends_with(&format!("::{}", v.name)) { 
                                            if !v.data_types.is_empty() { ptn = v.data_types[0].clone(); } 
                                            break; 
                                        } 
                                    } 
                                }
                                let lt = self.aion_type_to_llvm(&ptn); 
                                let cp = self.builder.build_bit_cast(dp, self.context.ptr_type(AddressSpace::default()), "arm_datacast")?; 
                                let lv_val = self.builder.build_load(lt, cp.into_pointer_value(), &arm.params[0])?;
                                let pa = self.builder.build_alloca(lt, &arm.params[0])?; 
                                self.builder.build_store(pa, lv_val)?; 
                                av.insert(arm.params[0].clone(), (pa, lt, ptn));
                            }
                            
                            // Evaluate guard condition if present
                            if let Some(guard_expr) = &arm.guard {
                                let guard_val = self.compile_expr(guard_expr, &av, function)?.into_int_value();
                                let guard_pass_bb = self.context.append_basic_block(function, "guard_pass");
                                let guard_fail_bb = nb;
                                let guard_cond = self.builder.build_int_compare(IntPredicate::NE, guard_val, i64_t.const_zero(), "guard_cond")?;
                                self.builder.build_conditional_branch(guard_cond, guard_pass_bb, guard_fail_bb)?;
                                self.builder.position_at_end(guard_pass_bb);
                            }
                            
                            let ar = self.compile_block(&arm.body, &mut av, function)?; 
                            let abf = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?; 
                            if abf.get_terminator().is_none() { 
                                let v = ar.unwrap_or(i64_t.const_zero().into()); 
                                phis.push((v, abf)); 
                            }
                            if !is_last { self.builder.position_at_end(nb); }
                        }
                    } else {
                        // Match on primitives (i64, String)
                        let na = arms.len();
                        for (i, arm) in arms.iter().enumerate() {
                            let pattern_clean = arm.pattern.chars().filter(|c| c.is_alphanumeric()).collect::<String>();
                            let ab = self.context.append_basic_block(function, &format!("arm_{}_{}", i, pattern_clean));
                            let is_last = i == na - 1;
                            let nb = if is_last { exit_bb } else { self.context.append_basic_block(function, "match_next") };
                            
                            // Get all patterns
                            let all_patterns: Vec<String> = if arm.patterns.is_empty() {
                                vec![arm.pattern.clone()]
                            } else {
                                arm.patterns.clone()
                            };
                            
                            let is_default = all_patterns.iter().any(|p| p == "_");
                            // If pattern is a binding variable (not a number) and we have params or guard, treat as wildcard
                            let is_binding_var = arm.params.len() > 0 || arm.guard.is_some();
                            let mut prim_match_cond: Option<inkwell::values::IntValue<'ctx>> = None;
                            
                            if !is_default && !is_binding_var {
                                if ctn == "i64" || ctn == "Integer" {
                                    for pat in &all_patterns {
                                        if let Some((start_str, end_str)) = pat.split_once("..") {
                                            if let (Ok(start), Ok(end)) = (start_str.parse::<i64>(), end_str.parse::<i64>()) {
                                                let cv_val = cv.into_int_value();
                                                let cond_start = self.builder.build_int_compare(IntPredicate::SGE, cv_val, i64_t.const_int(start as u64, false), "range_start")?;
                                                let cond_end = self.builder.build_int_compare(IntPredicate::SLE, cv_val, i64_t.const_int(end as u64, false), "range_end")?;
                                                let range_cond = self.builder.build_and(cond_start, cond_end, "range_cond")?;
                                                prim_match_cond = Some(match prim_match_cond {
                                                    Some(prev) => self.builder.build_or(prev, range_cond, "range_or")?,
                                                    None => range_cond,
                                                });
                                            }
                                        } else if let Ok(val) = pat.parse::<i64>() {
                                            let cond = self.builder.build_int_compare(IntPredicate::EQ, cv.into_int_value(), i64_t.const_int(val as u64, false), "match_cond")?;
                                            prim_match_cond = Some(match prim_match_cond {
                                                Some(prev) => self.builder.build_or(prev, cond, "match_or")?,
                                                None => cond,
                                            });
                                        }
                                    }
                                } else if ctn == "String" {
                                    for pat in &all_patterns {
                                        let pattern_str = if pat.starts_with('"') && pat.ends_with('"') {
                                            pat[1..pat.len()-1].to_string()
                                        } else { pat.clone() };
                                        
                                        let ps = self.builder.build_global_string_ptr(&pattern_str, "match_pattern")?;
                                        let fnc = self.module.get_function("aion_str_eq").ok_or_else(|| CompileError::Internal("aion_str_eq not found".to_string()))?;
                                        let cmp = self.builder.build_call(fnc, &[cv.into(), ps.as_basic_value_enum().into()], "streq")?.try_as_basic_value().unwrap_basic().into_int_value();
                                        let cond = self.builder.build_int_compare(IntPredicate::NE, cmp, i64_t.const_zero(), "match_cond")?;
                                        prim_match_cond = Some(match prim_match_cond {
                                            Some(prev) => self.builder.build_or(prev, cond, "match_or")?,
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
                                let test_bb = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?;
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
                                    av.insert(arm.params[0].clone(), (pa, ptr_type.into(), format!("*{}", ctn)));
                                } else {
                                    self.builder.build_store(pa, cv)?;
                                    av.insert(arm.params[0].clone(), (pa, cv_type, ctn.to_string()));
                                }
                            }
                            
                            // Evaluate guard condition if present
                            if let Some(guard_expr) = &arm.guard {
                                let guard_val = self.compile_expr(guard_expr, &av, function)?.into_int_value();
                                let guard_pass_bb = self.context.append_basic_block(function, "guard_pass");
                                let guard_fail_bb = nb;
                                let guard_cond = self.builder.build_int_compare(IntPredicate::NE, guard_val, i64_t.const_zero(), "guard_cond")?;
                                self.builder.build_conditional_branch(guard_cond, guard_pass_bb, guard_fail_bb)?;
                                self.builder.position_at_end(guard_pass_bb);
                            }
                            
                            let ar = self.compile_block(&arm.body, &mut av, function)?;
                            let abf = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?;
                            if abf.get_terminator().is_none() {
                                let v = ar.unwrap_or(i64_t.const_zero().into());
                                phis.push((v, abf));
                            }
                            if !is_last { self.builder.position_at_end(nb); }
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
                                        v = self.builder.build_int_to_ptr(v.into_int_value(), pt, "phi_ptr")?.into(); 
                                    } else if target_type.is_int_type() && v.is_pointer_value() { 
                                        v = self.builder.build_ptr_to_int(v.into_pointer_value(), i64_t, "phi_int")?.into(); 
                                    }
                                }
                                if b.get_terminator().is_none() {
                                    self.builder.build_unconditional_branch(exit_bb)?;
                                }
                                final_phis.push((v, b));
                            }
                            self.builder.position_at_end(exit_bb);
                            let phi = self.builder.build_phi(target_type, "matchres")?; 
                            for (v, b) in final_phis { phi.add_incoming(&[(&v, b)]); } 
                            lv = Some(phi.as_basic_value());
                        }
                    } else { lv = None; }
                },
                Statement::ExpressionStmt(e) => { lv = Some(self.compile_expr(e, variables, function)?); },
                Statement::UnsafeBlock(stmts) | Statement::Spawn(stmts) => { lv = self.compile_block(stmts, variables, function)?; },
                _ => { lv = None; }
            }
        }
        Ok(lv)
    }

    fn compile_lvalue(&mut self, e: &Expression, variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>, function: FunctionValue<'ctx>) -> Result<(PointerValue<'ctx>, BasicTypeEnum<'ctx>), CompileError> {
        let pt = self.context.ptr_type(AddressSpace::default());
        match e {
            Expression::Identifier(name) => {
                if let Some((vn, fnm)) = name.split_once('.') { 
                    if let Some((vptr, vt, vtn)) = variables.get(vn) { 
                        let btn = if vtn.contains('<') { vtn.split('<').next().ok_or_else(|| CompileError::Internal("Invalid type name".to_string()))? } else { vtn }; 
                        let ftn = self.resolve_fuzzy_name(&self.struct_types, btn).unwrap_or(btn.to_string()); 
                        if let Some(flds) = self.struct_fields.get(ftn.as_str()) { 
                            if let Some(&idx) = flds.get(fnm) { 
                                let st = *self.struct_types.get(&ftn).ok_or_else(|| CompileError::Internal(format!("LLVM struct type '{}' not found", ftn)))?; 
                                let st_ptr = self.builder.build_load(*vt, *vptr, "st_load")?.into_pointer_value();
                                return Ok((self.builder.build_struct_gep(st, st_ptr, idx, "fldptr")?, self.aion_type_to_llvm(&self.get_field_type(vtn, fnm)))); 
                            } 
                        } 
                    } 
                }
                if let Some((ptr, vt, _)) = variables.get(name) { Ok((*ptr, *vt)) } else { Err(self.err(format!("variable '{}' not found", name), e)) }
            },
            Expression::MemberAccess { receiver, member } => {
                let (rp, rt_llvm) = self.compile_lvalue(receiver, variables, function)?; 
                let rtn = self.get_expr_type_name(receiver, variables); 
                let ftn = self.get_field_type(&rtn, member);
                let mut bc = if rtn.contains('<') { rtn.split('<').next().ok_or_else(|| CompileError::Internal("Invalid type name".to_string()))? } else { &rtn }; 
                while bc.starts_with('*') { bc = &bc[1..]; }
                let ft = self.resolve_fuzzy_name(&self.decls, bc).ok_or_else(|| CompileError::Internal(format!("Struct '{}' not found (rec_type={}, receiver={:?})", bc, rtn, receiver)))?;
                let st = *self.struct_types.get(&ft).ok_or_else(|| CompileError::Internal(format!("LLVM type not found for '{}'", ft)))?;
                let idx = *self.struct_fields.get(&ft).ok_or_else(|| CompileError::Internal(format!("Fields for struct '{}' not found", ft)))?.get(member).ok_or_else(|| CompileError::Internal(format!("Field '{}' not found", member)))?;
                
                let st_ptr = self.builder.build_load(rt_llvm, rp, "st_load")?.into_pointer_value();
                Ok((self.builder.build_struct_gep(st, st_ptr, idx, member)?, self.aion_type_to_llvm(&ftn)))
            },
            Expression::Deref { expr } => { 
                let v = self.compile_expr(expr, variables, function)?; 
                let tn = self.get_expr_type_name(expr, variables); 
                let et = self.aion_type_to_llvm(if tn.starts_with('*') { &tn[1..] } else { "i64" }); 
                let p = if v.is_int_value() { self.builder.build_int_to_ptr(v.into_int_value(), pt, "i2p")? } else { v.into_pointer_value() }; 
                Ok((p, et)) 
            },
            _ => Err(CompileError::Internal(format!("Not an lvalue: {:?}", e))),
        }
    }

    fn compile_expr(&mut self, e: &Expression, variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>, function: FunctionValue<'ctx>) -> Result<BasicValueEnum<'ctx>, CompileError> {
        let i64_t = self.context.i64_type(); 
        let pt = self.context.ptr_type(AddressSpace::default());
        match e {
            Expression::Integer(n) => Ok(i64_t.const_int(*n as u64, false).into()),
            Expression::Float(f_val) => Ok(self.context.f64_type().const_float(*f_val).into()),
            Expression::Boolean(b) => Ok(i64_t.const_int(if *b { 1 } else { 0 }, false).into()),
            Expression::String(s) => Ok(self.builder.build_global_string_ptr(&format!("{}\0", s), "aion_str")?.as_basic_value_enum()),
            Expression::Identifier(name) => { 
                if let Some((ptr, vt, _)) = variables.get(name) { 
                    Ok(self.builder.build_load(*vt, *ptr, name)?) 
                } else { 
                    if let Ok((ptr, vt)) = self.compile_lvalue(e, variables, function) { 
                        return Ok(self.builder.build_load(vt, ptr, name)?); 
                    } 
                    if name == "argc" { if let Some(g) = self.module.get_global("aion_argc") { return Ok(self.builder.build_load(i64_t, g.as_pointer_value(), "argc")?); } } 
                    if name == "argv" { if let Some(g) = self.module.get_global("aion_argv") { return Ok(self.builder.build_load(pt, g.as_pointer_value(), "argv")?); } } 
                    Err(self.err(format!("variable '{}' not found", name), e)) 
                } 
            },
            Expression::Call { function: fnm, generic_args, arguments, .. } => {
                let mut afn = fnm.clone(); 
                let mut aga = generic_args.clone(); 
                let mut aa = arguments.clone(); 
                let mut is_mc = false;
                if let Some((rn, mn)) = fnm.rsplit_once('.') {
                     let re = Expression::Identifier(rn.to_string());
                     let tn = self.get_expr_type_name(&re, variables);
                     if (tn.starts_with('*') || tn.contains("ptr")) && mn == "offset" && arguments.len() == 1 {
                         let idx = self.compile_expr(&arguments[0], variables, function)?.into_int_value();
                         let ptr = if let Ok((p, _)) = self.compile_lvalue(&re, variables, function) {
                             self.builder.build_load(self.context.ptr_type(AddressSpace::default()), p, "ptrload")?.into_pointer_value()
                         } else {
                             self.compile_expr(&re, variables, function)?.into_pointer_value()
                         };
                         let element_type = if tn.starts_with('*') { self.aion_type_to_llvm(&tn[1..]) } else { self.context.i64_type().into() };
                         return Ok(unsafe { self.builder.build_gep(element_type, ptr, &[idx], "offset_ptr")? }.into());
                     }
                     if tn != "unknown" { 
                         let (btn, tga) = if tn.contains('<') { let p: Vec<&str> = tn.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect(); (p[0].to_string(), p[1..].iter().map(|s| s.trim().to_string()).collect()) } else { (tn.clone(), vec![]) }; 
                         let mut bc = btn.as_str(); while bc.starts_with('*') { bc = &bc[1..]; } 
                         let tp = self.resolve_fuzzy_name(&self.struct_types, bc).or_else(|| self.resolve_fuzzy_name(&self.enum_types, bc)).unwrap_or(btn.clone()); 
                         let fc = self.resolve_fuzzy_name(&self.decls, &format!("{}.{}", tp, mn)).unwrap_or(format!("{}.{}", tp, mn)); 
                         if let Some(Declaration::Function(f_decl)) = self.decls.get(&fc) { 
                             let has_self = f_decl.params.get(0).map_or(false, |(n, _, _)| n == "self");
                             if has_self {
                                 aa.insert(0, re); 
                                 if aga.is_empty() { aga = tga; } 
                                 is_mc = true; 
                             }
                             afn = fc; 
                         } 
                     }
                }
                let full = self.resolve_fuzzy_name(&self.decls, &afn).unwrap_or(afn.clone()); 
                let mut lnm = full.clone(); 
                if let Some(Declaration::Function(f_decl)) = self.decls.get(&full) { 
                    for (an, av) in &f_decl.attributes { if an == "intrinsic" { lnm = av.replace("libc.", ""); break; } } 
                }
                let fv = if !aga.is_empty() { 
                    let gn = format!("{}_{}", full, aga.join("_")); 
                    if let Some(e) = self.module.get_function(&gn) { e } else { self.instantiate_function(&full, &aga)? } 
                } else {
                    self.module.get_function(&lnm).ok_or_else(|| self.err(format!("function '{}' not found", afn), e))?
                };
                let mut ca = Vec::new(); 
                let pts = fv.get_type().get_param_types();
                for (i, arg) in aa.iter().enumerate() { 
                    let ep = pts.get(i).map_or(false, |t: &inkwell::types::BasicMetadataTypeEnum| t.is_pointer_type()); 
                    let val = if i == 0 && is_mc { 
                        self.compile_expr(arg, variables, function)?
                    } else { 
                        let v = self.compile_expr(arg, variables, function)?; 
                        // Auto-convert i64 to string for io.println/io.print
                        if (fnm == "io.println" || fnm == "io.print") && v.is_int_value() {
                            let conv_fn = self.module.get_function("aion_int_to_str").ok_or_else(|| CompileError::Internal("aion_int_to_str not found".to_string()))?;
                            self.builder.build_call(conv_fn, &[v.into()], "to_str")?.try_as_basic_value().unwrap_basic()
                        } else if ep && !v.get_type().is_pointer_type() { 
                            let a = self.builder.build_alloca(v.get_type(), "temp_arg")?; 
                            self.builder.build_store(a, v)?; 
                            a.into() 
                        } else { v } 
                    }; 
                    ca.push(val.into()); 
                }
                let call = if fv.get_type().get_return_type().is_none() { 
                    self.builder.build_call(fv, &ca, "") 
                } else { 
                    self.builder.build_call(fv, &ca, "calltmp") 
                }?; 
                Ok(match call.try_as_basic_value() { ValueKind::Basic(v) => v, _ => i64_t.const_zero().into() })
            },
            Expression::Infix { left, operator, right } => {
                if operator.kind == TokenKind::And || operator.kind == TokenKind::Or { 
                    let lhs = self.compile_expr(left, variables, function)?; 
                    let li = match lhs { BasicValueEnum::IntValue(i) => i, _ => return Err(CompileError::Internal("Expected boolean".to_string())) }; 
                    let rb = self.context.append_basic_block(function, "logic_rhs"); 
                    let mb = self.context.append_basic_block(function, "logic_merge"); 
                    let cond = if operator.kind == TokenKind::And { 
                        self.builder.build_int_compare(IntPredicate::NE, li, i64_t.const_zero(), "and_cond")? 
                    } else { 
                        self.builder.build_int_compare(IntPredicate::EQ, li, i64_t.const_zero(), "or_cond")? 
                    }; 
                    self.builder.build_conditional_branch(cond, rb, mb)?; 
                    let lfb = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?; 
                    self.builder.position_at_end(rb); 
                    let rhs = self.compile_expr(right, variables, function)?; 
                    let ri = match rhs { BasicValueEnum::IntValue(i) => i, _ => return Err(CompileError::Internal("Expected boolean".to_string())) }; 
                    if self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?.get_terminator().is_none() { 
                        self.builder.build_unconditional_branch(mb)?; 
                    } 
                    let rfb = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?; 
                    self.builder.position_at_end(mb); 
                    let phi = self.builder.build_phi(i64_t, "logic_res")?; 
                    let lv_v = if operator.kind == TokenKind::And { i64_t.const_zero() } else { i64_t.const_int(1, false) }; 
                    phi.add_incoming(&[(&lv_v, lfb), (&ri, rfb)]); 
                    return Ok(phi.as_basic_value().into()); 
                }

                let lhs = self.compile_expr(left, variables, function)?; 

                // Special case for 'inside' which needs to peek at 'right' before compiling it
                if operator.kind == TokenKind::Inside {
                    if let Expression::Infix { left: r_left, operator: r_op, right: r_right } = &**right {
                        if r_op.kind == TokenKind::Range {
                            let l = lhs.into_int_value();
                            let min = self.compile_expr(r_left, variables, function)?.into_int_value();
                            let max = self.compile_expr(r_right, variables, function)?.into_int_value();
                            let c1 = self.builder.build_int_compare(IntPredicate::SGE, l, min, "in_ge")?;
                            let c2 = self.builder.build_int_compare(IntPredicate::SLT, l, max, "in_lt")?;
                            let res = self.builder.build_and(c1, c2, "in_res")?;
                            return Ok(self.builder.build_int_z_extend(res, i64_t, "bool")?.into());
                        }
                    }
                    return Err(CompileError::Internal("Operator 'inside' currently only supports ranges (min..max)".to_string()));
                }

                let rhs = self.compile_expr(right, variables, function)?;
                if lhs.is_int_value() && rhs.is_int_value() {
                    let mut l = lhs.into_int_value(); 
                    let mut r = rhs.into_int_value(); 
                    if l.get_type().get_bit_width() < 64 { l = self.builder.build_int_s_extend(l, i64_t, "l_ext")?; } 
                    if r.get_type().get_bit_width() < 64 { r = self.builder.build_int_s_extend(r, i64_t, "r_ext")?; }
                    match &operator.kind {
                        TokenKind::Plus => Ok(self.builder.build_int_add(l, r, "add")?.into()), 
                        TokenKind::Minus => Ok(self.builder.build_int_sub(l, r, "sub")?.into()), 
                        TokenKind::Star => Ok(self.builder.build_int_mul(l, r, "mul")?.into()), 
                        TokenKind::Slash => Ok(self.builder.build_int_signed_div(l, r, "div")?.into()),
                        TokenKind::EqEq => Ok(self.builder.build_int_z_extend(self.builder.build_int_compare(IntPredicate::EQ, l, r, "eq")?, i64_t, "bool")?.into()), 
                        TokenKind::NotEq => Ok(self.builder.build_int_z_extend(self.builder.build_int_compare(IntPredicate::NE, l, r, "ne")?, i64_t, "bool")?.into()),
                        TokenKind::Lt => Ok(self.builder.build_int_z_extend(self.builder.build_int_compare(IntPredicate::SLT, l, r, "lt")?, i64_t, "bool")?.into()), 
                        TokenKind::Gt => Ok(self.builder.build_int_z_extend(self.builder.build_int_compare(IntPredicate::SGT, l, r, "gt")?, i64_t, "bool")?.into()),
                        TokenKind::LtEq => Ok(self.builder.build_int_z_extend(self.builder.build_int_compare(IntPredicate::SLE, l, r, "lteq")?, i64_t, "bool")?.into()), 
                        TokenKind::GtEq => Ok(self.builder.build_int_z_extend(self.builder.build_int_compare(IntPredicate::SGE, l, r, "gteq")?, i64_t, "bool")?.into()),
                        TokenKind::Percent => Ok(self.builder.build_int_signed_rem(l, r, "rem")?.into()), 
                        TokenKind::Caret => Ok(self.builder.build_xor(l, r, "xor")?.into()), 
                        _ => Err(CompileError::Internal(format!("Operator {:?} not supported", operator.kind))),
                    }
                } else if (lhs.is_pointer_value() || lhs.is_int_value()) && (rhs.is_pointer_value() || rhs.is_int_value()) {
                    let l = if lhs.is_int_value() { self.builder.build_int_to_ptr(lhs.into_int_value(), pt, "i2p")?.into() } else { lhs };
                    let r = if rhs.is_int_value() { self.builder.build_int_to_ptr(rhs.into_int_value(), pt, "i2p")?.into() } else { rhs };
                    let lp = l.into_pointer_value(); 
                    let rp = r.into_pointer_value();
                    match &operator.kind { 
                        TokenKind::EqEq | TokenKind::NotEq => { 
                            let ltn = self.get_expr_type_name(left, variables);
                            let rtn = self.get_expr_type_name(right, variables);
                            if ltn == "String" && rtn == "String" {
                                let fnc = self.module.get_function("aion_str_eq").ok_or_else(|| CompileError::Internal("aion_str_eq not found".to_string()))?;
                                let cmp = self.builder.build_call(fnc, &[lp.into(), rp.into()], "streq")?.try_as_basic_value().unwrap_basic().into_int_value();
                                let res = if operator.kind == TokenKind::EqEq {
                                    self.builder.build_int_compare(IntPredicate::NE, cmp, i64_t.const_zero(), "eq")?
                                } else {
                                    self.builder.build_int_compare(IntPredicate::EQ, cmp, i64_t.const_zero(), "ne")?
                                };
                                Ok(self.builder.build_int_z_extend(res, i64_t, "bool")?.into())
                            } else {
                                let pred = if operator.kind == TokenKind::EqEq { IntPredicate::EQ } else { IntPredicate::NE }; 
                                let cmp = self.builder.build_int_compare(pred, lp, rp, "ptrcmp")?; 
                                Ok(self.builder.build_int_z_extend(cmp, i64_t, "bool")?.into()) 
                            }
                        }, 
                        TokenKind::Plus if lhs.is_pointer_value() && rhs.is_pointer_value() => { 
                            let fnc = self.module.get_function("aion_str_concat").ok_or_else(|| CompileError::Internal("aion_str_concat not found".to_string()))?; 
                            let call = self.builder.build_call(fnc, &[lp.into(), rp.into()], "strconcat")?; 
                            Ok(match call.try_as_basic_value() { ValueKind::Basic(v) => v, _ => i64_t.const_zero().into() }) 
                        },
                        TokenKind::Plus if lhs.is_pointer_value() && rhs.is_int_value() => { 
                            let c2s = self.module.get_function("aion_char_to_str").ok_or_else(|| CompileError::Internal("aion_char_to_str not found".to_string()))?; 
                            let s2 = self.builder.build_call(c2s, &[rhs.into()], "char2str")?.try_as_basic_value().unwrap_basic(); 
                            let fnc = self.module.get_function("aion_str_concat").ok_or_else(|| CompileError::Internal("aion_str_concat not found".to_string()))?; 
                            let call = self.builder.build_call(fnc, &[lp.into(), s2.into()], "strconcat")?; 
                            Ok(match call.try_as_basic_value() { ValueKind::Basic(v) => v, _ => i64_t.const_zero().into() }) 
                        }, 
                        _ => Err(CompileError::Internal(format!("Mixed operator {:?} not supported", operator.kind))),
                    }
                } else { Err(CompileError::Internal("Type mismatch".to_string())) }
            },
            Expression::StructInst { name, fields, .. } => {
                let sn = self.resolve_fuzzy_name(&self.struct_types, name).ok_or_else(|| CompileError::Internal(format!("Struct type '{}' not found in StructInst", name)))?; 
                let st = *self.struct_types.get(&sn).ok_or_else(|| CompileError::Internal(format!("LLVM struct type '{}' not found", sn)))?; 
                let fm = self.struct_fields.get(&sn).ok_or_else(|| CompileError::Internal(format!("Fields for struct '{}' not found", sn)))?.clone();
                let mfn = self.module.get_function("aion_malloc").ok_or_else(|| CompileError::Internal("aion_malloc not found".to_string()))?; 
                let pr = self.builder.build_call(mfn, &[st.size_of().ok_or_else(|| CompileError::Internal("Struct size unknown".to_string()))?.into()], "struct_alloc")?.try_as_basic_value();
                let ptr = match pr { ValueKind::Basic(v) => v.into_pointer_value(), _ => return Err(CompileError::Internal("malloc failed".to_string())) };
                for (fnm, fe) in fields { 
                    let val = self.compile_expr(fe, variables, function)?; 
                    let idx = *fm.get(fnm).ok_or_else(|| CompileError::Internal(format!("Field '{}' not found in struct '{}'", fnm, sn)))?; 
                    self.builder.build_store(self.builder.build_struct_gep(st, ptr, idx, fnm)?, val)?; 
                }
                Ok(ptr.into())
            },
            Expression::EnumInst { name, variant, arguments, generic_args } => {
                if let Some(en) = self.resolve_fuzzy_name(&self.enum_types, name) {
                    let et = *self.enum_types.get(&en).ok_or_else(|| CompileError::Internal(format!("Enum type '{}' not found", en)))?;
                    let pr = match self.builder.build_call(self.module.get_function("aion_malloc").ok_or_else(|| CompileError::Internal("aion_malloc not found".to_string()))?, &[et.size_of().ok_or_else(|| CompileError::Internal("Enum size unknown".to_string()))?.into()], "enum_alloc")?.try_as_basic_value() { 
                        ValueKind::Basic(v) => v.into_pointer_value(), 
                        _ => return Err(CompileError::Internal("malloc failed".to_string())) 
                    };
                    let mut tv = 0; 
                    if let Some(Declaration::Enum(e_decl)) = self.decls.get(&en) { 
                        for (idx, v) in e_decl.variants.iter().enumerate() { 
                            if v.name == *variant { tv = idx as u64; break; } 
                        } 
                    }
                    self.builder.build_store(self.builder.build_struct_gep(et, pr, 0, "tag")?, i64_t.const_int(tv, false))?;
                    if !arguments.is_empty() { 
                        let val = self.compile_expr(&arguments[0], variables, function)?; 
                        let cp = self.builder.build_bit_cast(self.builder.build_struct_gep(et, pr, 1, "data")?, pt, "datacast")?; 
                        self.builder.build_store(cp.into_pointer_value(), val)?; 
                    }
                    Ok(pr.into())
                } else {
                    let call_expr = Expression::Call { function: format!("{}.{}", name, variant), generic_args: generic_args.clone(), arguments: arguments.clone(), line: 0, col: 0 };
                    self.compile_expr(&call_expr, variables, function)
                }
            },
            Expression::MemberAccess { .. } => {
                let (rp, rt_llvm) = self.compile_lvalue(e, variables, function)?; 
                Ok(self.builder.build_load(rt_llvm, rp, "load_member")?)
            },
            Expression::MethodCall { receiver, method, generic_args, arguments, .. } => {
                let rtn = self.get_expr_type_name(receiver, variables);
                if method == "offset" && rtn.starts_with('*') { 
                    let p = self.compile_expr(receiver, variables, function)?.into_pointer_value(); 
                    let o = self.compile_expr(&arguments[0], variables, function)?.into_int_value(); 
                    return Ok(unsafe { self.builder.build_gep(self.aion_type_to_llvm(&rtn[1..]), p, &[o], "offset_ptr")? }.into()); 
                }
                let (btn, tga) = if rtn.contains('<') { 
                    let p: Vec<&str> = rtn.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect(); 
                    (p[0].to_string(), p[1..].iter().map(|s| s.trim().to_string()).collect()) 
                } else { (rtn.clone(), vec![]) };
                let mut bc = btn.as_str(); while bc.starts_with('*') { bc = &bc[1..]; }
                let tp = self.resolve_fuzzy_name(&self.struct_types, bc).or_else(|| self.resolve_fuzzy_name(&self.enum_types, bc)).unwrap_or(btn.clone()); 
                let fm = self.resolve_fuzzy_name(&self.decls, &format!("{}.{}", tp, method)).unwrap_or(format!("{}.{}", tp, method));
                let mut cg = tga; cg.extend(generic_args.clone());
                let fv = if !cg.is_empty() { 
                    let gn = format!("{}_{}", fm, cg.join("_")); 
                    if let Some(e) = self.module.get_function(&gn) { e } else { self.instantiate_function(&fm, &cg)? } 
                } else { 
                    self.module.get_function(&fm).ok_or_else(|| CompileError::Internal(format!("Method '{}' not found", fm)))? 
                };
                let mut ca = Vec::new(); 
                let rv = self.compile_expr(receiver, variables, function)?;
                ca.push(rv.into()); 
                for arg in arguments { 
                    let compiled = self.compile_expr(arg, variables, function)?;
                    // Auto-convert i64 to string for io.println/io.print
                    if fm == "io.println" || fm == "io.print" {
                        if compiled.is_int_value() {
                            let conv_fn = self.module.get_function("aion_int_to_str").ok_or_else(|| CompileError::Internal("aion_int_to_str not found".to_string()))?;
                            let converted = self.builder.build_call(conv_fn, &[compiled.into()], "to_str")?.try_as_basic_value().unwrap_basic();
                            ca.push(converted.into());
                        } else {
                            ca.push(compiled.into());
                        }
                    } else {
                        ca.push(compiled.into());
                    }
                }
                let call = self.builder.build_call(fv, &ca, "call")?; 
                Ok(match call.try_as_basic_value() { ValueKind::Basic(v) => v, _ => i64_t.const_zero().into() })
            },
            Expression::Cast { target, expr } => {
                let v = self.compile_expr(expr, variables, function)?;
                let t_clean = target.replace(" ", "");
                let dest = self.aion_type_to_llvm(&t_clean);
                if v.is_int_value() && dest.is_int_type() {
                    let sw = v.into_int_value().get_type().get_bit_width();
                    let dw = dest.into_int_type().get_bit_width();
                    if sw < dw { Ok(self.builder.build_int_z_extend(v.into_int_value(), dest.into_int_type(), "ext")?.into()) }
                    else if sw > dw { Ok(self.builder.build_int_truncate(v.into_int_value(), dest.into_int_type(), "trunc")?.into()) }
                    else { Ok(v) }
                } else if v.is_pointer_value() && dest.is_int_type() {
                    Ok(self.builder.build_ptr_to_int(v.into_pointer_value(), dest.into_int_type(), "p2i")?.into())
                } else if v.is_int_value() && dest.is_pointer_type() {
                    Ok(self.builder.build_int_to_ptr(v.into_int_value(), dest.into_pointer_type(), "i2p")?.into())
                } else if v.get_type() == dest {
                    Ok(v)
                } else {
                    Ok(self.builder.build_bit_cast(v, dest, "cast")?)
                }
            },
            Expression::Deref { expr } => { 
                let v = self.compile_expr(expr, variables, function)?; 
                let tn = self.get_expr_type_name(expr, variables); 
                let et = self.aion_type_to_llvm(if tn.starts_with('*') { &tn[1..] } else { "i64" }); 
                let p = if v.is_int_value() { self.builder.build_int_to_ptr(v.into_int_value(), pt, "i2p")? } else { v.into_pointer_value() }; 
                Ok(self.builder.build_load(et, p, "deref")?) 
            },
            Expression::If { condition, then_branch, else_branch } => {
                let cv = self.compile_expr(condition, variables, function)?.into_int_value(); 
                let comp = self.builder.build_int_compare(IntPredicate::NE, cv, i64_t.const_int(0, false), "ifcond")?;
                let tb = self.context.append_basic_block(function, "then"); 
                let eb = self.context.append_basic_block(function, "else"); 
                let mb = self.context.append_basic_block(function, "ifcont");
                self.builder.build_conditional_branch(comp, tb, eb)?; 
                let mut phis = Vec::new();
                
                self.builder.position_at_end(tb); 
                let mut tv = variables.clone(); 
                let tr = self.compile_block(then_branch, &mut tv, function)?; 
                let tf = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?; 
                if tf.get_terminator().is_none() { 
                    let v = tr.unwrap_or(i64_t.const_zero().into()); 
                    phis.push((v, tf)); 
                }
                
                self.builder.position_at_end(eb); 
                let mut ev = variables.clone(); 
                let er = if let Some(e) = else_branch { self.compile_block(e, &mut ev, function)? } else { None }; 
                let ef = self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?; 
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
                                v = self.builder.build_int_to_ptr(v.into_int_value(), pt, "phi_ptr")?.into(); 
                            } else if target_type.is_int_type() && v.is_pointer_value() { 
                                v = self.builder.build_ptr_to_int(v.into_pointer_value(), i64_t, "phi_int")?.into(); 
                            }
                        }
                        self.builder.build_unconditional_branch(mb)?;
                        final_phis.push((v, b));
                    }
                    self.builder.position_at_end(mb);
                    let phi = self.builder.build_phi(target_type, "ifres")?; 
                    for (v, b) in final_phis { phi.add_incoming(&[(&v, b)]); } 
                    Ok(phi.as_basic_value())
                } else { 
                    if self.builder.get_insert_block().ok_or_else(|| CompileError::Internal("No active insert block".to_string()))?.get_terminator().is_none() { 
                        self.builder.build_unconditional_branch(mb)?; 
                    } 
                    self.builder.position_at_end(mb);
                    Ok(i64_t.const_zero().into())
                }
            },
            Expression::Block { statements, .. } => { 
                let mut lv_vars = variables.clone(); 
                Ok(self.compile_block(statements, &mut lv_vars, function)?.unwrap_or(i64_t.const_zero().into())) 
            },
            Expression::Intrinsic { name, arguments } => {
                let mut an = name.clone(); 
                let mut aa = arguments.clone(); 
                if name == "intrinsic" && !arguments.is_empty() { 
                    if let Expression::String(s) = &arguments[0] { an = s.clone(); aa.remove(0); } 
                }
                if an == "sizeof" && !aa.is_empty() { 
                    let tnm = match &aa[0] { 
                        Expression::Identifier(s) => s.clone(), 
                        Expression::TypeRef { name, .. } => name.clone(), 
                        _ => "i64".to_string() 
                    }; 
                    let clean = tnm.replace(" ", "");
                    let btn = if clean.contains('<') { clean.split('<').next().ok_or_else(|| CompileError::Internal("Invalid type name".to_string()))? } else { &clean };
                    if let Some(sn) = self.resolve_fuzzy_name(&self.struct_types, btn) {
                        if let Some(st) = self.struct_types.get(&sn) {
                            return Ok(st.size_of().ok_or_else(|| CompileError::Internal("Struct size unknown".to_string()))?.into());
                        }
                    }
                    if let Some(en) = self.resolve_fuzzy_name(&self.enum_types, btn) {
                        if let Some(et) = self.enum_types.get(&en) {
                            return Ok(et.size_of().ok_or_else(|| CompileError::Internal("Enum size unknown".to_string()))?.into());
                        }
                    }
                    let lt = self.aion_type_to_llvm(&tnm);
                    let res = if lt.is_pointer_type() { i64_t.const_int(8, false).into() } 
                              else { lt.size_of().unwrap_or(i64_t.const_zero()).into() };
                    return Ok(res);
                }
                if an == "mem_is_null" && !aa.is_empty() {
                    let ptr = self.compile_expr(&aa[0], variables, function)?.into_pointer_value();
                    let cmp = self.builder.build_int_compare(IntPredicate::EQ, self.builder.build_ptr_to_int(ptr, i64_t, "p2i")?, i64_t.const_zero(), "isnull")?;
                    return Ok(self.builder.build_int_z_extend(cmp, i64_t, "zext")?.into());
                }
                if an == "mem_zero" {
                    if !aa.is_empty() {
                        let tnm = match &aa[0] { 
                            Expression::Identifier(s) => s.clone(), 
                            Expression::TypeRef { name, .. } => name.clone(), 
                            _ => "i64".to_string() 
                        };
                        let lt = self.aion_type_to_llvm(&tnm);
                        if lt.is_pointer_type() { return Ok(lt.into_pointer_type().const_null().into()); }
                        if lt.is_int_type() { return Ok(lt.into_int_type().const_zero().into()); }
                        if lt.is_float_type() { return Ok(lt.into_float_type().const_zero().into()); }
                    }
                    return Ok(pt.const_null().into());
                }
                let lnm = match an.as_str() { "str_len" => "strlen".to_string(), "str_ptr" => return Ok(self.compile_expr(&aa[0], variables, function)?), "fs_read_to_string" => "aion_read_file".to_string(), "fs_write" => "aion_write_file".to_string(), "fs_append" => "aion_append_file".to_string(), "exit" => "exit".to_string(), _ if an.starts_with("libc.") => an.replace("libc.", ""), _ => format!("aion_{}", an) };
                let fv = self.module.get_function(&lnm).ok_or_else(|| CompileError::Internal(format!("Intrinsic '{}' not found", lnm)))?; 
                let mut cargs = Vec::new(); 
                for arg in aa { cargs.push(self.compile_expr(&arg, variables, function)?.into()); }
                let call = self.builder.build_call(fv, &cargs, "intrinsic_call")?; 
                Ok(match call.try_as_basic_value() { ValueKind::Basic(v) => v, _ => i64_t.const_zero().into() })
            },
            _ => Ok(i64_t.const_zero().into()),
        }
    }

    fn get_field_type(&self, it: &String, fnm: &str) -> String {
        let clean = it.replace(" ", "");
        let mut cs = clean.as_str();
        while cs.starts_with('*') { cs = &cs[1..]; }
        let (btn, tga) = if cs.contains('<') {
            let p: Vec<&str> = cs.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
            (p[0].to_string(), p[1..].iter().map(|s| s.trim().to_string()).collect::<Vec<String>>())
        } else { (cs.to_string(), vec![]) };
        let full = self.resolve_fuzzy_name(&self.struct_types, &btn).unwrap_or(btn);
        if let Some(Declaration::Struct(s)) = self.decls.get(&full) {
            for (f_nm, ft) in &s.fields {
                if f_nm == fnm {
                    let mut rft = ft.clone();
                    for (i, p) in s.generic_params.iter().enumerate() {
                        if i < tga.len() { rft = rft.replace(p, &tga[i]); }
                    }
                    return rft.replace(" ", "");
                }
            }
        }
        "unknown".to_string()
    }
    fn get_expr_type_name(&self, e: &Expression, variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>) -> String {
        let res = match e {
            Expression::Integer(_) => "i64".to_string(), 
            Expression::Float(_) => "f64".to_string(), 
            Expression::Boolean(_) => "bool".to_string(), 
            Expression::String(_) => "String".to_string(),
            Expression::Identifier(name) => {
                let parts: Vec<&str> = name.split('.').collect();
                if parts.len() > 1 {
                    let mut ct = if let Some((_, _, t)) = variables.get(parts[0]) { t.clone() } else { "unknown".to_string() };
                    for i in 1..parts.len() { if ct == "unknown" { break; } ct = self.get_field_type(&ct, parts[i]); }
                    return ct.replace(" ", "");
                }
                if let Some((_, _, t)) = variables.get(name) { t.clone().replace(" ", "") } else { "unknown".to_string() }
            },
            Expression::Call { function: name, generic_args, .. } => {
                if let Some((rn, mn)) = name.rsplit_once('.') {
                    if mn == "offset" {
                        let rt = self.get_expr_type_name(&Expression::Identifier(rn.to_string()), variables);
                        if rt.starts_with('*') { return rt.replace(" ", ""); }
                    }
                    let rt = self.get_expr_type_name(&Expression::Identifier(rn.to_string()), variables);
                    if rt != "unknown" {
                        let (btn, tga) = if rt.contains('<') {
                            let p: Vec<&str> = rt.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                            (p[0].to_string(), p[1..].iter().map(|s| s.trim().to_string()).collect::<Vec<String>>())
                        } else { (rt.clone(), vec![]) };
                        let mut bc = btn.as_str(); while bc.starts_with('*') { bc = &bc[1..]; }
                        let tp = self.resolve_fuzzy_name(&self.struct_types, bc).or_else(|| self.resolve_fuzzy_name(&self.enum_types, bc)).unwrap_or(btn.clone());
                        let method_name = format!("{}.{}", tp, mn);
                        if let Some(Declaration::Function(f_decl)) = self.decls.get(&method_name) {
                            let mut res_type = f_decl.return_type.clone();
                            let combined_args = if generic_args.is_empty() { &tga } else { generic_args };
                            for (i, p) in f_decl.generic_params.iter().enumerate() { if i < combined_args.len() { res_type = res_type.replace(p, &combined_args[i]); } }
                            return res_type.replace(" ", "");
                        }
                    }
                }
                let full = self.resolve_fuzzy_name(&self.decls, name).unwrap_or(name.clone());
                if let Some(Declaration::Function(f_decl)) = self.decls.get(&full) {
                    let mut r = f_decl.return_type.clone(); 
                    for (i, p) in f_decl.generic_params.iter().enumerate() { if i < generic_args.len() { r = r.replace(p, &generic_args[i]); } } 
                    return r.replace(" ", "");
                }
                "unknown".to_string()
            },
            Expression::MemberAccess { receiver, member } => { let rt = self.get_expr_type_name(receiver, variables); self.get_field_type(&rt, member) },
            Expression::MethodCall { receiver, method, generic_args, .. } => {
                let rt = self.get_expr_type_name(receiver, variables); 
                if method == "offset" && rt.starts_with('*') { return rt.clone(); }
                let (btn, tga) = if rt.contains('<') {
                    let p: Vec<&str> = rt.split(['<', '>', ',']).filter(|s| !s.is_empty()).collect();
                    (p[0].to_string(), p[1..].iter().map(|s| s.trim().to_string()).collect::<Vec<String>>())
                } else { (rt.clone(), vec![]) };
                let mut bc = btn.as_str(); while bc.starts_with('*') { bc = &bc[1..]; }
                let full = self.resolve_fuzzy_name(&self.decls, bc).unwrap_or(btn.clone());
                let method_name = format!("{}.{}", full, method);
                if let Some(Declaration::Function(f_decl)) = self.decls.get(&method_name) {
                    let mut res_type = f_decl.return_type.clone();
                    for (i, p) in f_decl.generic_params.iter().enumerate() { if i < generic_args.len() { res_type = res_type.replace(p, &generic_args[i]); } }
                    if let Some(decl) = self.decls.get(&full) { 
                        let bp = match decl { Declaration::Struct(s) => &s.generic_params, Declaration::Enum(e) => &e.generic_params, _ => &vec![] }; 
                        for (i, p) in bp.iter().enumerate() { if i < tga.len() { res_type = res_type.replace(p, &tga[i]); } } 
                    }
                    return res_type.replace(" ", "");
                }
                "unknown".to_string()
            },
            Expression::StructInst { name, generic_args, .. } => {
                let sn = self.resolve_fuzzy_name(&self.struct_types, name).unwrap_or(name.clone());
                if generic_args.is_empty() { sn.replace(" ", "") } else { format!("{}<{}>", sn, generic_args.join(",")).replace(" ", "") }
            },
            Expression::EnumInst { name, variant, arguments, generic_args } => {
                if let Some(en) = self.resolve_fuzzy_name(&self.enum_types, name) {
                    if generic_args.is_empty() { en.replace(" ", "") } else { format!("{}<{}>", en, generic_args.join(",")).replace(" ", "") }
                } else {
                    let call_expr = Expression::Call { function: format!("{}.{}", name, variant), generic_args: generic_args.clone(), arguments: arguments.clone(), line: 0, col: 0 };
                    self.get_expr_type_name(&call_expr, variables)
                }
            },
            Expression::Cast { target, .. } => target.replace(" ", ""),
            Expression::Block { statements, .. } => {
                if let Some(s) = statements.last() {
                    match s {
                        Statement::ExpressionStmt(e) | Statement::Return { value: e, .. } => self.get_expr_type_name(e, variables),
                        _ => "unknown".to_string()
                    }
                } else { "unknown".to_string() }
            },
            Expression::Deref { expr } => {
                let t = self.get_expr_type_name(expr, variables);
                if t.starts_with('*') { t[1..].to_string().replace(" ", "") } else { "unknown".to_string() }
            },
            Expression::TypeRef { name, generic_args } => { if generic_args.is_empty() { name.clone() } else { format!("{}<{}>", name, generic_args.join(",")) } },
            _ => "unknown".to_string(),
        };
        res.replace(" ", "")
    }

    pub fn optimize(&self) -> Result<(), CompileError> {
        let builder = PassManagerBuilder::create();
        builder.set_optimization_level(OptimizationLevel::Aggressive);

        let fpm = PassManager::create(&self.module);
        builder.populate_function_pass_manager(&fpm);

        for function in self.module.get_functions() {
            fpm.run_on(&function);
        }

        let mpm = PassManager::create(());
        builder.populate_module_pass_manager(&mpm);
        mpm.run_on(&self.module);

        Ok(())
    }

    pub fn print_to_file(&self, path: &Path) -> Result<(), CompileError> { 
        self.module.print_to_file(path).map_err(|e| CompileError::Io(e.to_string())) 
    }
}

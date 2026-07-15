use crate::ast::*;
use crate::error::CompileError;
use inkwell::AddressSpace;
use inkwell::OptimizationLevel;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::passes::{PassManager, PassManagerBuilder};
use inkwell::types::{BasicType, StructType};
use inkwell::values::{BasicValueEnum, FunctionValue};
use std::collections::{HashMap, HashSet};
use std::path::Path;

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
    pub(in crate::codegen) loop_exit_blocks: Vec<BasicBlock<'ctx>>,
    pub(in crate::codegen) loop_cond_blocks: Vec<BasicBlock<'ctx>>,
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
            loop_exit_blocks: Vec::new(),
            loop_cond_blocks: Vec::new(),
        };
        compiler.register_builtins();
        compiler
    }

    pub(in crate::codegen) fn err(
        &self,
        msg: impl Into<String>,
        expr: &Expression,
    ) -> CompileError {
        let span = expr.span();
        CompileError::new(msg, span.line, span.col).with_snippet(&self.source)
    }

    pub(in crate::codegen) fn resolve_fuzzy_name<T>(
        &self,
        map: &HashMap<String, T>,
        name: &str,
    ) -> Option<String> {
        let clean = name.replace(" ", "");
        if map.contains_key(&clean) {
            return Some(clean);
        }

        let p: Vec<&str> = clean.split('.').collect();
        let last = p.last()?.to_string();
        if map.contains_key(&last) {
            return Some(last);
        }

        for k in map.keys() {
            if k.ends_with(&clean) || clean.ends_with(k) {
                return Some(k.clone());
            }
        }
        None
    }

    pub(in crate::codegen) fn compile_function(
        &mut self,
        decl: &Declaration,
    ) -> Result<FunctionValue<'ctx>, CompileError> {
        if let Declaration::Function(f) = decl {
            let function = if let Some(e) = self.module.get_function(&f.name) {
                if e.get_first_basic_block().is_some() {
                    return Ok(e);
                }
                e
            } else {
                let mut pt = Vec::new();
                let ptr_t = self.context.ptr_type(AddressSpace::default());
                if f.name == "main" {
                    pt.push(self.context.i32_type().into());
                    pt.push(ptr_t.into());
                } else {
                    for (_, ptn, _) in &f.params {
                        pt.push(self.aion_type_to_llvm(ptn).into());
                    }
                }
                self.module.add_function(
                    &f.name,
                    self.aion_type_to_llvm(&f.return_type).fn_type(&pt, false),
                    None,
                )
            };

            if let Some(body) = &f.body {
                let pb = self.builder.get_insert_block();
                let bb = self.context.append_basic_block(function, "entry");
                self.builder.position_at_end(bb);
                let mut local_vars = HashMap::new();
                let i64_t = self.context.i64_type();

                if f.name == "main" {
                    let gc_init = self.module.get_function("GC_init").ok_or_else(|| {
                        CompileError::internal("GC_init function not found".to_string())
                    })?;
                    self.builder.build_call(gc_init, &[], "")?;

                    if let Some(argc) = function.get_nth_param(0) {
                        let av = self.builder.build_int_z_extend(
                            argc.into_int_value(),
                            i64_t,
                            "argc_ext",
                        )?;
                        let a = self.builder.build_alloca(i64_t, "argc")?;
                        self.builder.build_store(a, av)?;
                        local_vars.insert("argc".to_string(), (a, i64_t.into(), "i64".to_string()));
                        if let Some(g) = self.module.get_global("aion_argc") {
                            self.builder.build_store(g.as_pointer_value(), av)?;
                        }
                    }
                    if let Some(argv) = function.get_nth_param(1) {
                        let a = self
                            .builder
                            .build_alloca(self.context.ptr_type(AddressSpace::default()), "argv")?;
                        self.builder.build_store(a, argv)?;
                        local_vars.insert(
                            "argv".to_string(),
                            (
                                a,
                                self.context.ptr_type(AddressSpace::default()).into(),
                                "ptr".to_string(),
                            ),
                        );
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
                            local_vars
                                .insert(an.clone(), (a, arg.get_type(), atn.replace(" ", "")));

                            // If there's a default value and the arg is zero-initialized, use the default
                            if default_val.is_some() {
                                // For now, we'll handle defaults at call site
                            }
                        }
                    }
                }

                let lbv = self.compile_block(body, &mut local_vars, function)?;
                if let Some(cb) = self.builder.get_insert_block()
                    && cb.get_terminator().is_none()
                {
                    let rt = function.get_type().get_return_type();
                    if let Some(mut v) = lbv {
                        if let Some(tt) = rt
                            && v.get_type() != tt
                        {
                            if tt.is_pointer_type() && v.is_int_value() {
                                v = self
                                    .builder
                                    .build_int_to_ptr(
                                        v.into_int_value(),
                                        self.context.ptr_type(AddressSpace::default()),
                                        "ret_ptr",
                                    )?
                                    .into();
                            } else if tt.is_int_type() && v.is_pointer_value() {
                                v = self
                                    .builder
                                    .build_ptr_to_int(v.into_pointer_value(), i64_t, "ret_int")?
                                    .into();
                            }
                        }
                        self.builder.build_return(Some(&v))?;
                    } else {
                        let def: BasicValueEnum = if rt.is_some_and(|t| t.is_pointer_type()) {
                            self.context
                                .ptr_type(AddressSpace::default())
                                .const_null()
                                .into()
                        } else {
                            i64_t.const_zero().into()
                        };
                        self.builder.build_return(Some(&def))?;
                    }
                }
                if let Some(p) = pb {
                    self.builder.position_at_end(p);
                }
            }
            Ok(function)
        } else {
            Err(CompileError::internal("Not a function".to_string()))
        }
    }

    pub fn compile(&mut self, program: &Program) -> Result<(), CompileError> {
        for d in &program.declarations {
            match d {
                Declaration::Function(f) => {
                    self.decls.insert(f.name.clone(), d.clone());
                }
                Declaration::Struct(s) => {
                    self.decls.insert(s.name.clone(), d.clone());
                }
                Declaration::Enum(e) => {
                    self.decls.insert(e.name.clone(), d.clone());
                }
                Declaration::Impl(i) => {
                    let mut ftn = i.target_name.clone();
                    if !i.generic_params.is_empty() {
                        ftn = format!("{}<{}>", i.target_name, i.generic_params.join(","));
                    }
                    let bt = if i.target_name.contains('<') {
                        i.target_name.split('<').next().ok_or_else(|| {
                            CompileError::internal("Invalid target name".to_string())
                        })?
                    } else {
                        &i.target_name
                    };
                    for f in &i.functions {
                        let mut nf = f.clone();
                        nf.name = format!("{}.{}", bt, f.name);
                        for (_, pt, _) in nf.params.iter_mut() {
                            if pt == "Self" {
                                *pt = ftn.clone();
                            }
                        }
                        if nf.return_type == "Self" {
                            nf.return_type = ftn.clone();
                        }
                        let mut cg = i.generic_params.clone();
                        cg.extend(f.generic_params.clone());
                        nf.generic_params = cg;
                        self.decls
                            .insert(nf.name.clone(), Declaration::Function(nf));
                    }
                }
                _ => {}
            }
        }

        let pt = self.context.ptr_type(AddressSpace::default());
        let i64_t = self.context.i64_type();

        self.module.add_function(
            "printf",
            self.context.i32_type().fn_type(&[pt.into()], true),
            None,
        );
        self.module
            .add_function("strlen", i64_t.fn_type(&[pt.into()], false), None);
        self.module.add_function(
            "exit",
            self.context
                .void_type()
                .fn_type(&[self.context.i32_type().into()], false),
            None,
        );
        self.module
            .add_function("malloc", pt.fn_type(&[i64_t.into()], false), None);
        self.module.add_function(
            "realloc",
            pt.fn_type(&[pt.into(), i64_t.into()], false),
            None,
        );
        self.module.add_function(
            "free",
            self.context.void_type().fn_type(&[pt.into()], false),
            None,
        );
        self.module.add_function(
            "aion_io_print",
            self.context.void_type().fn_type(&[pt.into()], false),
            None,
        );
        self.module.add_function(
            "aion_io_println",
            self.context.void_type().fn_type(&[pt.into()], false),
            None,
        );
        self.module
            .add_function("aion_io_read_line", pt.fn_type(&[], false), None);
        self.module.add_function(
            "GC_init",
            self.context.void_type().fn_type(&[], false),
            None,
        );
        self.module.add_function(
            "aion_str_eq",
            i64_t.fn_type(&[pt.into(), pt.into()], false),
            None,
        );
        self.module.add_function(
            "aion_str_concat",
            pt.fn_type(&[pt.into(), pt.into()], false),
            None,
        );
        self.module
            .add_function("aion_int_to_str", pt.fn_type(&[i64_t.into()], false), None);
        self.module.add_function(
            "aion_float_to_str",
            pt.fn_type(&[self.context.f64_type().into()], false),
            None,
        );
        self.module.add_function(
            "aion_str_to_float",
            self.context.f64_type().fn_type(&[pt.into()], false),
            None,
        );
        self.module
            .add_function("aion_read_file", pt.fn_type(&[pt.into()], false), None);
        self.module.add_function(
            "aion_write_file",
            i64_t.fn_type(&[pt.into(), pt.into()], false),
            None,
        );
        self.module.add_function(
            "aion_append_file",
            i64_t.fn_type(&[pt.into(), pt.into()], false),
            None,
        );
        self.module
            .add_function("aion_fs_exists", i64_t.fn_type(&[pt.into()], false), None);
        self.module
            .add_function("aion_getenv", pt.fn_type(&[pt.into()], false), None);
        self.module
            .add_function("aion_get_argc", i64_t.fn_type(&[], false), None);
        self.module.add_function(
            "aion_get_argv_index",
            pt.fn_type(&[i64_t.into()], false),
            None,
        );
        self.module
            .add_function("aion_malloc", pt.fn_type(&[i64_t.into()], false), None);
        self.module.add_function(
            "aion_memzero",
            self.context
                .void_type()
                .fn_type(&[pt.into(), i64_t.into()], false),
            None,
        );
        self.module.add_function(
            "aion_str_at",
            i64_t.fn_type(&[pt.into(), i64_t.into()], false),
            None,
        );
        self.module.add_function(
            "aion_str_substr",
            pt.fn_type(&[pt.into(), i64_t.into(), i64_t.into()], false),
            None,
        );
        self.module
            .add_function("aion_char_to_str", pt.fn_type(&[i64_t.into()], false), None);
        self.module.add_function(
            "aion_ai_tensor_zeros",
            pt.fn_type(&[pt.into()], false),
            None,
        );
        self.module
            .add_function("aion_ai_tensor_ones", pt.fn_type(&[pt.into()], false), None);
        self.module
            .add_function("aion_ai_tensor_rand", pt.fn_type(&[pt.into()], false), None);
        self.module.add_function(
            "aion_ai_tensor_backward",
            self.context.void_type().fn_type(&[pt.into()], false),
            None,
        );
        self.module.add_function(
            "aion_ai_tensor_move",
            pt.fn_type(&[pt.into(), pt.into()], false),
            None,
        );
        self.module.add_function(
            "aion_array_oob",
            self.context
                .void_type()
                .fn_type(&[i64_t.into(), i64_t.into()], false),
            None,
        );

        self.module
            .add_global(i64_t, Some(AddressSpace::default()), "aion_argc")
            .set_initializer(&i64_t.const_zero());
        self.module
            .add_global(pt, Some(AddressSpace::default()), "aion_argv")
            .set_initializer(&pt.const_null());

        for d in &program.declarations {
            match d {
                Declaration::Struct(s) => {
                    self.struct_types
                        .insert(s.name.clone(), self.context.opaque_struct_type(&s.name));
                }
                Declaration::Enum(e) => {
                    self.enum_types.insert(
                        e.name.clone(),
                        self.context.struct_type(
                            &[i64_t.into(), self.context.i8_type().array_type(512).into()],
                            false,
                        ),
                    );
                }
                _ => {}
            }
        }

        for d in &program.declarations {
            if let Declaration::Struct(s) = d {
                let mut fm = HashMap::new();
                for (i, (n, _)) in s.fields.iter().enumerate() {
                    fm.insert(n.clone(), i as u32);
                }
                self.struct_fields.insert(s.name.clone(), fm);
                let st = *self.struct_types.get(&s.name).ok_or_else(|| {
                    CompileError::internal(format!("Struct type '{}' not found", s.name))
                })?;
                let mut ft_list = Vec::new();
                for (_, tnm) in &s.fields {
                    ft_list.push(self.aion_type_to_llvm(tnm));
                }
                st.set_body(&ft_list, false);
            }
        }

        if self
            .resolve_fuzzy_name(&self.enum_types, "Option")
            .is_none()
        {
            self.enum_types.insert(
                "Option".to_string(),
                self.context.struct_type(
                    &[i64_t.into(), self.context.i8_type().array_type(512).into()],
                    false,
                ),
            );
        }

        let ad: Vec<Declaration> = self.decls.values().cloned().collect();
        for d in &ad {
            if let Declaration::Function(f) = d
                && f.generic_params.is_empty()
            {
                let mut pv = Vec::new();
                if f.name == "main" {
                    pv.push(self.context.i32_type().into());
                    pv.push(pt.into());
                } else {
                    for (_, ptn, _) in &f.params {
                        pv.push(self.aion_type_to_llvm(ptn).into());
                    }
                }
                let mut ln = f.name.clone();
                for (an, av) in &f.attributes {
                    if an == "intrinsic" {
                        ln = av.replace("libc.", "");
                        break;
                    }
                }
                self.module.add_function(
                    &ln,
                    self.aion_type_to_llvm(&f.return_type).fn_type(&pv, false),
                    None,
                );
            }
        }

        for d in &ad {
            if let Declaration::Function(f) = d
                && f.generic_params.is_empty()
            {
                self.compile_function(d)?;
            }
        }
        Ok(())
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
        self.module
            .print_to_file(path)
            .map_err(|e| CompileError::io(e.to_string()))
    }
}

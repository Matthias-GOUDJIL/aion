use crate::ast::*;
use crate::error::CompileError;
use inkwell::AddressSpace;
use inkwell::OptimizationLevel;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::passes::{PassManager, PassManagerBuilder};
use inkwell::types::{BasicType, BasicTypeEnum, StructType};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
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

    pub(in crate::codegen) fn get_field_type(&self, it: &str, fnm: &str) -> String {
        let clean = it.replace(" ", "");
        let mut cs = clean.as_str();
        while cs.starts_with('*') {
            cs = &cs[1..];
        }
        // Tuple field access by numeric index: `(i64,String)` + "0" -> "i64".
        // Depth-aware split so nested tuples work. #53.
        if cs.starts_with('(') && cs.ends_with(')') {
            if let Ok(idx) = fnm.parse::<usize>() {
                let inner = &cs[1..cs.len() - 1];
                let mut depth = 0i32;
                let mut cur = String::new();
                let mut parts: Vec<String> = Vec::new();
                for c in inner.chars() {
                    match c {
                        '(' | '<' => {
                            depth += 1;
                            cur.push(c);
                        }
                        ')' | '>' => {
                            depth -= 1;
                            cur.push(c);
                        }
                        ',' if depth == 0 => {
                            parts.push(cur.clone());
                            cur.clear();
                        }
                        _ => cur.push(c),
                    }
                }
                parts.push(cur);
                if idx < parts.len() {
                    return parts[idx].trim().to_string();
                }
            }
            return "unknown".to_string();
        }
        let (btn, tga) = if cs.contains('<') {
            let p: Vec<&str> = cs
                .split(['<', '>', ','])
                .filter(|s| !s.is_empty())
                .collect();
            (
                p[0].to_string(),
                p[1..]
                    .iter()
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<String>>(),
            )
        } else {
            (cs.to_string(), vec![])
        };
        let full = self
            .resolve_fuzzy_name(&self.struct_types, &btn)
            .unwrap_or(btn);
        if let Some(Declaration::Struct(s)) = self.decls.get(&full) {
            for (f_nm, ft) in &s.fields {
                if f_nm == fnm {
                    let rft = Self::substitute_generic_params(ft, &s.generic_params, &tga);
                    return rft.replace(" ", "");
                }
            }
        }
        "unknown".to_string()
    }
    pub(in crate::codegen) fn get_expr_type_name(
        &self,
        e: &Expression,
        variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>,
    ) -> String {
        let res = match e {
            Expression::Integer(_, _) => "i64".to_string(),
            Expression::Float(_, _) => "f64".to_string(),
            Expression::Char(_, _) => "i64".to_string(),
            Expression::Boolean(_, _) => "bool".to_string(),
            Expression::String(_, _) => "String".to_string(),
            Expression::Identifier(name, _) => {
                let parts: Vec<&str> = name.split('.').collect();
                if parts.len() > 1 {
                    let mut ct = if let Some((_, _, t)) = variables.get(parts[0]) {
                        t.clone()
                    } else {
                        "unknown".to_string()
                    };
                    for p in parts.iter().skip(1) {
                        if ct == "unknown" {
                            break;
                        }
                        ct = self.get_field_type(&ct, p);
                    }
                    return ct.replace(" ", "");
                }
                if let Some((_, _, t)) = variables.get(name) {
                    t.clone().replace(" ", "")
                } else {
                    "unknown".to_string()
                }
            }
            Expression::Call {
                function: name,
                generic_args,
                ..
            } => {
                if let Some((rn, mn)) = name.rsplit_once('.') {
                    if mn == "offset" {
                        let rt = self.get_expr_type_name(
                            &Expression::Identifier(rn.to_string(), Span::zero()),
                            variables,
                        );
                        if rt.starts_with('*') {
                            return rt.replace(" ", "");
                        }
                    }
                    let rt = self.get_expr_type_name(
                        &Expression::Identifier(rn.to_string(), Span::zero()),
                        variables,
                    );
                    if rt != "unknown" {
                        let (btn, tga) = if rt.contains('<') {
                            let p: Vec<&str> = rt
                                .split(['<', '>', ','])
                                .filter(|s| !s.is_empty())
                                .collect();
                            (
                                p[0].to_string(),
                                p[1..]
                                    .iter()
                                    .map(|s| s.trim().to_string())
                                    .collect::<Vec<String>>(),
                            )
                        } else {
                            (rt.clone(), vec![])
                        };
                        let mut bc = btn.as_str();
                        while bc.starts_with('*') {
                            bc = &bc[1..];
                        }
                        let tp = self
                            .resolve_fuzzy_name(&self.struct_types, bc)
                            .or_else(|| self.resolve_fuzzy_name(&self.enum_types, bc))
                            .unwrap_or(btn.clone());
                        let method_name_colon = format!("{}::{}", tp, mn);
                        let method_name_dot = format!("{}.{}", tp, mn);
                        if let Some(Declaration::Function(f_decl)) = self
                            .decls
                            .get(&method_name_colon)
                            .or_else(|| self.decls.get(&method_name_dot))
                        {
                            let mut res_type = f_decl.return_type.clone();
                            let combined_args = if generic_args.is_empty() {
                                &tga
                            } else {
                                generic_args
                            };
                            res_type = Self::substitute_generic_params(
                                &res_type,
                                &f_decl.generic_params,
                                combined_args,
                            );
                            return res_type.replace(" ", "");
                        }
                    }
                }
                let full = self
                    .resolve_fuzzy_name(&self.decls, name)
                    .unwrap_or(name.clone());
                if let Some(Declaration::Function(f_decl)) = self.decls.get(&full) {
                    let r = Self::substitute_generic_params(
                        &f_decl.return_type,
                        &f_decl.generic_params,
                        generic_args,
                    );
                    return r.replace(" ", "");
                }
                "unknown".to_string()
            }
            Expression::MemberAccess {
                receiver, member, ..
            } => {
                let rt = self.get_expr_type_name(receiver, variables);
                self.get_field_type(&rt, member)
            }
            Expression::MethodCall {
                receiver,
                method,
                generic_args,
                ..
            } => {
                let rt = self.get_expr_type_name(receiver, variables);
                if method == "offset" && rt.starts_with('*') {
                    return rt.clone();
                }
                let (btn, tga) = if rt.contains('<') {
                    let p: Vec<&str> = rt
                        .split(['<', '>', ','])
                        .filter(|s| !s.is_empty())
                        .collect();
                    (
                        p[0].to_string(),
                        p[1..]
                            .iter()
                            .map(|s| s.trim().to_string())
                            .collect::<Vec<String>>(),
                    )
                } else {
                    (rt.clone(), vec![])
                };
                let mut bc = btn.as_str();
                while bc.starts_with('*') {
                    bc = &bc[1..];
                }
                let full = self
                    .resolve_fuzzy_name(&self.decls, bc)
                    .unwrap_or(btn.clone());
                let method_name_colon = format!("{}::{}", full, method);
                let method_name_dot = format!("{}.{}", full, method);
                if let Some(Declaration::Function(f_decl)) = self
                    .decls
                    .get(&method_name_colon)
                    .or_else(|| self.decls.get(&method_name_dot))
                {
                    let mut res_type = Self::substitute_generic_params(
                        &f_decl.return_type,
                        &f_decl.generic_params,
                        generic_args,
                    );
                    if let Some(decl) = self.decls.get(&full) {
                        let bp = match decl {
                            Declaration::Struct(s) => &s.generic_params,
                            Declaration::Enum(e) => &e.generic_params,
                            _ => &vec![],
                        };
                        res_type = Self::substitute_generic_params(&res_type, bp, &tga);
                    }
                    return res_type.replace(" ", "");
                }
                "unknown".to_string()
            }
            Expression::StructInst {
                name, generic_args, ..
            } => {
                let sn = self
                    .resolve_fuzzy_name(&self.struct_types, name)
                    .unwrap_or(name.clone());
                if generic_args.is_empty() {
                    sn.replace(" ", "")
                } else {
                    format!("{}<{}>", sn, generic_args.join(",")).replace(" ", "")
                }
            }
            Expression::EnumInst {
                name,
                variant,
                arguments,
                generic_args,
                ..
            } => {
                if let Some(en) = self.resolve_fuzzy_name(&self.enum_types, name) {
                    if !generic_args.is_empty() {
                        format!("{}<{}>", en, generic_args.join(",")).replace(" ", "")
                    } else {
                        // Infer generic args from variant arguments
                        let mut inferred_args: Vec<String> = vec![];
                        if let Some(Declaration::Enum(_)) = self.decls.get(&en) {
                            for arg in arguments.iter() {
                                inferred_args.push(self.get_expr_type_name(arg, variables));
                            }
                        }
                        if inferred_args.is_empty() {
                            en.replace(" ", "")
                        } else {
                            format!("{}<{}>", en, inferred_args.join(",")).replace(" ", "")
                        }
                    }
                } else {
                    let call_expr = Expression::Call {
                        function: format!("{}.{}", name, variant),
                        generic_args: generic_args.clone(),
                        arguments: arguments.clone(),
                        span: Span::zero(),
                    };
                    self.get_expr_type_name(&call_expr, variables)
                }
            }
            Expression::Cast { target, .. } => target.replace(" ", ""),
            Expression::Block { statements, .. } => {
                if let Some(s) = statements.last() {
                    match s {
                        Statement::ExpressionStmt(e, _) | Statement::Return { value: e, .. } => {
                            self.get_expr_type_name(e, variables)
                        }
                        _ => "unknown".to_string(),
                    }
                } else {
                    "unknown".to_string()
                }
            }
            Expression::Deref { expr, .. } => {
                let t = self.get_expr_type_name(expr, variables);
                if let Some(stripped) = t.strip_prefix('*') {
                    stripped.to_string().replace(" ", "")
                } else {
                    "unknown".to_string()
                }
            }
            Expression::TypeRef {
                name, generic_args, ..
            } => {
                if generic_args.is_empty() {
                    name.clone()
                } else {
                    format!("{}<{}>", name, generic_args.join(","))
                }
            }
            Expression::Match { arms, .. } => {
                if let Some(arm) = arms.first() {
                    if let Some(s) = arm.body.last() {
                        match s {
                            Statement::ExpressionStmt(e, _)
                            | Statement::Return { value: e, .. } => {
                                self.get_expr_type_name(e, variables)
                            }
                            _ => "unknown".to_string(),
                        }
                    } else {
                        "unknown".to_string()
                    }
                } else {
                    "unknown".to_string()
                }
            }
            Expression::TupleLiteral { elements, .. } => {
                // Render the tuple type name `(T, U, ...)` so downstream
                // lookups can find the registered struct type. #53.
                let elems: Vec<String> = elements
                    .iter()
                    .map(|e| self.get_expr_type_name(e, variables))
                    .collect();
                format!("({})", elems.join(","))
            }
            Expression::TupleAccess { tuple, index, .. } => {
                // The element type name of field `index`. #53.
                let tn = self.get_expr_type_name(tuple, variables);
                self.get_field_type(&tn, &index.to_string())
            }
            Expression::ArrayLiteral { elements, .. } => {
                // Render `[T; N]` from the first element's type so that
                // downstream `arr[i]` codegen can parse the array type. #54.
                let elem_tn = if elements.is_empty() {
                    "i64".to_string()
                } else {
                    self.get_expr_type_name(&elements[0], variables)
                };
                format!("[{}; {}]", elem_tn, elements.len())
            }
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
        self.module
            .print_to_file(path)
            .map_err(|e| CompileError::io(e.to_string()))
    }
}

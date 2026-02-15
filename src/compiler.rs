use std::path::Path;
use std::collections::HashMap;
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, PointerValue, FunctionValue, BasicValue};
use inkwell::types::{StructType, BasicTypeEnum};
use inkwell::{AddressSpace, IntPredicate};
use crate::ast::*;
use crate::token::Token;
use crate::checker::TypeChecker;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub struct_types: HashMap<String, StructType<'ctx>>,
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
        }
    }

    pub fn compile(&mut self, program: &Program) -> Result<(), String> {
        // 1. Run Type Checker (Safety Pass)
        let mut checker = TypeChecker::new();
        if let Err(e) = checker.check_program(program) {
            return Err(format!("Type/Safety Error: {}", e));
        }

        let i64_type = self.context.i64_type();
        let f64_type = self.context.f64_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());

        // Register built-in functions
        let printf_type = self.context.i32_type().fn_type(&[ptr_type.into()], true);
        self.module.add_function("printf", printf_type, None);

        let spawn_type = self.context.void_type().fn_type(&[ptr_type.into()], false);
        self.module.add_function("aion_spawn", spawn_type, None);

        let pow_type = f64_type.fn_type(&[f64_type.into(), f64_type.into()], false);
        self.module.add_function("pow", pow_type, None);

        // Register Structs
        for decl in &program.declarations {
            if let Declaration::Struct(s) = decl {
                let struct_type = self.context.opaque_struct_type(&s.name);
                self.struct_types.insert(s.name.clone(), struct_type);
            }
        }

        // Process Functions
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                let mut param_types = Vec::new();
                for _ in &f.params {
                    param_types.push(i64_type.into());
                }
                
                let fn_type = i64_type.fn_type(&param_types, false);
                let function = self.module.add_function(&f.name, fn_type, None);

                for (i, arg) in function.get_param_iter().enumerate() {
                    arg.set_name(&f.params[i].0);
                }

                if let Some(body) = &f.body {
                    let basic_block = self.context.append_basic_block(function, "entry");
                    self.builder.position_at_end(basic_block);
                    
                    let mut local_vars = HashMap::new(); 
                    for (i, arg) in function.get_param_iter().enumerate() {
                        let arg_name = &f.params[i].0;
                        let alloca = self.builder.build_alloca(i64_type, arg_name).unwrap();
                        self.builder.build_store(alloca, arg).unwrap();
                        local_vars.insert(arg_name.clone(), (alloca, i64_type.into()));
                    }

                    self.compile_block(body, &mut local_vars, function)?;
                    
                    if basic_block.get_terminator().is_none() {
                        self.builder.build_return(Some(&i64_type.const_int(0, false))).unwrap();
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
    ) -> Result<(), String> {
        for stmt in body {
            match stmt {
                Statement::Let { name, value, .. } => {
                    let val = self.compile_expr(value, variables, function)?;
                    let val_type = val.get_type();
                    let alloca = self.builder.build_alloca(val_type, name).unwrap();
                    self.builder.build_store(alloca, val).unwrap();
                    variables.insert(name.clone(), (alloca, val_type));
                },
                Statement::Return { value, .. } => {
                    let val = self.compile_expr(value, variables, function)?;
                    self.builder.build_return(Some(&val)).unwrap();
                },
                Statement::If { condition, then_branch, else_branch } => {
                    let cond_val = self.compile_expr(condition, variables, function)?.into_int_value();
                    let comparison = self.builder.build_int_compare(IntPredicate::NE, cond_val, self.context.i64_type().const_int(0, false), "ifcond").unwrap();
                    let then_bb = self.context.append_basic_block(function, "then");
                    let else_bb = self.context.append_basic_block(function, "else");
                    let merge_bb = self.context.append_basic_block(function, "ifcont");
                    self.builder.build_conditional_branch(comparison, then_bb, else_bb).unwrap();
                    
                    self.builder.position_at_end(then_bb);
                    self.compile_block(then_branch, variables, function)?;
                    if then_bb.get_terminator().is_none() { self.builder.build_unconditional_branch(merge_bb).unwrap(); }
                    
                    self.builder.position_at_end(else_bb);
                    if let Some(eb) = else_branch { self.compile_block(eb, variables, function)?; }
                    if else_bb.get_terminator().is_none() { self.builder.build_unconditional_branch(merge_bb).unwrap(); }
                    
                    self.builder.position_at_end(merge_bb);
                },
                Statement::Match { condition, arms } => {
                    let _cond_val = self.compile_expr(condition, variables, function)?.into_int_value();
                    let exit_bb = self.context.append_basic_block(function, "matchexit");
                    
                    for arm in arms {
                        let arm_bb = self.context.append_basic_block(function, &format!("arm_{}", arm.pattern));
                        self.builder.position_at_end(arm_bb);
                        self.compile_block(&arm.body, variables, function)?;
                        if arm_bb.get_terminator().is_none() { self.builder.build_unconditional_branch(exit_bb).unwrap(); }
                    }
                    
                    self.builder.position_at_end(exit_bb);
                },
                Statement::ExpressionStmt(expr) => {
                    self.compile_expr(expr, variables, function)?;
                },
                Statement::UnsafeBlock(body) => {
                    self.compile_block(body, variables, function)?;
                },
                _ => {}
            }
        }
        Ok(())
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
                let s_with_newline = format!("{}\n\0", s);
                let global_str = self.builder.build_global_string_ptr(&s_with_newline, "aion_str").unwrap();
                Ok(global_str.as_basic_value_enum())
            },
            Expression::Identifier(name) => {
                let (ptr, basic_type) = variables.get(name).ok_or_else(|| format!("Var '{}' not found", name))?;
                Ok(self.builder.build_load(*basic_type, *ptr, name).unwrap())
            },
            Expression::Call { function: func_name, arguments } => {
                if func_name == "io.println" {
                    let printf = self.module.get_function("printf").ok_or("printf not found")?;
                    let arg = self.compile_expr(&arguments[0], variables, function)?;
                    
                    let printf_arg = if arg.get_type().is_pointer_type() {
                        arg.into()
                    } else {
                        arg.into()
                    };
                    
                    self.builder.build_call(printf, &[printf_arg], "printftmp").unwrap();
                    Ok(i64_type.const_int(0, false).into())
                } else { 
                    let fn_val = self.module.get_function(func_name).ok_or(format!("Function '{}' not found", func_name))?;
                    let mut compiled_args = Vec::new();
                    for arg in arguments {
                        compiled_args.push(self.compile_expr(arg, variables, function)?.into());
                    }
                    let call = self.builder.build_call(fn_val, &compiled_args, "calltmp").unwrap();
                    match call.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(val) => Ok(val),
                        inkwell::values::ValueKind::Instruction(_) => Ok(i64_type.const_int(0, false).into()),
                    }
                }
            },
            Expression::Infix { left, operator, right } => {
                let lhs = self.compile_expr(left, variables, function)?.into_int_value();
                let rhs = self.compile_expr(right, variables, function)?.into_int_value();
                
                match operator {
                    Token::Plus => Ok(self.builder.build_int_add(lhs, rhs, "addtmp").unwrap().into()),
                    Token::Minus => Ok(self.builder.build_int_sub(lhs, rhs, "subtmp").unwrap().into()),
                    Token::Star => Ok(self.builder.build_int_mul(lhs, rhs, "multmp").unwrap().into()),
                    Token::Slash => Ok(self.builder.build_int_signed_div(lhs, rhs, "divtmp").unwrap().into()),
                    Token::Percent => Ok(self.builder.build_int_signed_rem(lhs, rhs, "remtmp").unwrap().into()),
                    
                    Token::And => Ok(self.builder.build_and(lhs, rhs, "andtmp").unwrap().into()),
                    Token::Or => Ok(self.builder.build_or(lhs, rhs, "ortmp").unwrap().into()),
                    Token::Bang => {
                        Ok(self.builder.build_xor(rhs, i64_type.const_int(1, false), "nottmp").unwrap().into())
                    },
                    
                    Token::EqEq => {
                        let res = self.builder.build_int_compare(IntPredicate::EQ, lhs, rhs, "eqtmp").unwrap();
                        Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                    },
                    Token::NotEq => {
                        let res = self.builder.build_int_compare(IntPredicate::NE, lhs, rhs, "netmp").unwrap();
                        Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                    },
                    Token::Gt => {
                        let res = self.builder.build_int_compare(IntPredicate::SGT, lhs, rhs, "gttmp").unwrap();
                        Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                    },
                    Token::Lt => {
                        let res = self.builder.build_int_compare(IntPredicate::SLT, lhs, rhs, "lttmp").unwrap();
                        Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                    },
                    Token::GtEq => {
                        let res = self.builder.build_int_compare(IntPredicate::SGE, lhs, rhs, "getmp").unwrap();
                        Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                    },
                    Token::LtEq => {
                        let res = self.builder.build_int_compare(IntPredicate::SLE, lhs, rhs, "letmp").unwrap();
                        Ok(self.builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                    },
                    
                    Token::Caret => {
                        Ok(self.builder.build_int_mul(lhs, rhs, "pow_hack").unwrap().into())
                    },
                    _ => Err(format!("Operator {:?} not supported", operator)),
                }
            },
            Expression::Block { statements, .. } => {
                let mut local_vars = variables.clone();
                self.compile_block(statements, &mut local_vars, function)?;
                Ok(i64_type.const_int(0, false).into())
            },
            _ => Ok(i64_type.const_int(0, false).into()),
        }
    }

    pub fn print_to_file(&self, path: &Path) -> Result<(), String> {
        self.module.print_to_file(path).map_err(|e| e.to_string())
    }
}

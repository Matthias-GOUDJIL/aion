pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod types;
pub mod environment;
pub mod checker;
pub mod transpiler;

use std::fs;
use std::path::Path;
use std::collections::HashMap;
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, PointerValue, BasicValue, FunctionValue};
use inkwell::types::{StructType, BasicTypeEnum};
use inkwell::{AddressSpace, IntPredicate};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::ast::*;
use crate::token::Token;
use crate::checker::TypeChecker;
use crate::transpiler::sql::SqlTranspiler;

pub fn transpile_sql(input_path: &str) -> Result<String, String> {
    let source = fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    
    let mut transpiler = SqlTranspiler::new();
    Ok(transpiler.transpile(&program))
}

pub fn compile_file(input_path: &str, output_path: &str) -> Result<(), String> {
    let source = fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    // 1. Run Type Checker (Safety Pass)
    let mut checker = TypeChecker::new();
    if let Err(e) = checker.check_program(&program) {
        return Err(format!("Type/Safety Error: {}", e));
    }

    // 2. Run Code Generation
    let context = Context::create();
    let module = context.create_module("aion_module");
    let builder = context.create_builder();
    let i64_type = context.i64_type();
    let f64_type = context.f64_type();
    let ptr_type = context.ptr_type(AddressSpace::default());
    
    let mut struct_types: HashMap<String, StructType> = HashMap::new();

    let printf_type = context.i32_type().fn_type(&[ptr_type.into()], true);
    module.add_function("printf", printf_type, None);

    let spawn_type = context.void_type().fn_type(&[ptr_type.into()], false);
    module.add_function("aion_spawn", spawn_type, None);

    // pow(f64, f64) -> f64
    let pow_type = f64_type.fn_type(&[f64_type.into(), f64_type.into()], false);
    module.add_function("pow", pow_type, None);

    // Register Structs
    for decl in &program.declarations {
        match decl {
            Declaration::Struct(s) => {
                let struct_type = context.opaque_struct_type(&s.name);
                struct_types.insert(s.name.clone(), struct_type);
            },
            _ => {}
        }
    }

    // Process Functions
    for decl in &program.declarations {
        match decl {
            Declaration::Function(f) => {
                let mut param_types = Vec::new();
                for _ in &f.params {
                    param_types.push(i64_type.into());
                }
                
                let fn_type = i64_type.fn_type(&param_types, false);
                let function = module.add_function(&f.name, fn_type, None);

                for (i, arg) in function.get_param_iter().enumerate() {
                    arg.set_name(&f.params[i].0);
                }

                if let Some(body) = &f.body {
                    let basic_block = context.append_basic_block(function, "entry");
                    builder.position_at_end(basic_block);
                    
    let mut local_vars = HashMap::new(); 
    for (i, arg) in function.get_param_iter().enumerate() {
        let arg_name = &f.params[i].0;
        let alloca = builder.build_alloca(i64_type, arg_name).unwrap();
        builder.build_store(alloca, arg).unwrap();
        local_vars.insert(arg_name.clone(), (alloca, i64_type.into()));
    }

    compile_block(body, &context, &builder, &module, &mut local_vars, &struct_types, function)?;
                    
                    if basic_block.get_terminator().is_none() {
                        builder.build_return(Some(&i64_type.const_int(0, false))).unwrap();
                    }
                }
            },
            _ => {}
        }
    }

    module.print_to_file(Path::new(output_path)).map_err(|e| e.to_string())?;
    Ok(())
}

fn compile_block<'ctx>(
    body: &[Statement],
    context: &'ctx Context,
    builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    variables: &mut HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
    struct_types: &HashMap<String, StructType<'ctx>>,
    function: FunctionValue<'ctx>
) -> Result<(), String> {
    for stmt in body {
        match stmt {
            Statement::Let { name, value, .. } => {
                let val = compile_expr_internal(value, context, builder, module, variables, struct_types)?;
                let val_type = val.get_type();
                let alloca = builder.build_alloca(val_type, name).unwrap();
                builder.build_store(alloca, val).unwrap();
                variables.insert(name.clone(), (alloca, val_type));
            },
            Statement::Return { value, .. } => {
                let val = compile_expr_internal(value, context, builder, module, variables, struct_types)?;
                builder.build_return(Some(&val)).unwrap();
            },
            Statement::If { condition, then_branch, else_branch } => {
                let cond_val = compile_expr_internal(condition, context, builder, module, variables, struct_types)?.into_int_value();
                let comparison = builder.build_int_compare(IntPredicate::NE, cond_val, context.i64_type().const_int(0, false), "ifcond").unwrap();
                let then_bb = context.append_basic_block(function, "then");
                let else_bb = context.append_basic_block(function, "else");
                let merge_bb = context.append_basic_block(function, "ifcont");
                builder.build_conditional_branch(comparison, then_bb, else_bb).unwrap();
                builder.position_at_end(then_bb);
                compile_block(then_branch, context, builder, module, variables, struct_types, function)?;
                if then_bb.get_terminator().is_none() { builder.build_unconditional_branch(merge_bb).unwrap(); }
                builder.position_at_end(else_bb);
                if let Some(eb) = else_branch { compile_block(eb, context, builder, module, variables, struct_types, function)?; }
                if else_bb.get_terminator().is_none() { builder.build_unconditional_branch(merge_bb).unwrap(); }
                builder.position_at_end(merge_bb);
            },
            Statement::Match { condition, arms } => {
                let _cond_val = compile_expr_internal(condition, context, builder, module, variables, struct_types)?.into_int_value();
                let exit_bb = context.append_basic_block(function, "matchexit");
                
                let mut cases = Vec::new();
                for arm in arms {
                    let arm_bb = context.append_basic_block(function, &format!("arm_{}", arm.pattern));
                    builder.position_at_end(arm_bb);
                    compile_block(&arm.body, context, builder, module, variables, struct_types, function)?;
                    if arm_bb.get_terminator().is_none() { builder.build_unconditional_branch(exit_bb).unwrap(); }
                    
                    if let Ok(val) = arm.pattern.parse::<u64>() {
                        cases.push((context.i64_type().const_int(val, false), arm_bb));
                    }
                }
                
                builder.position_at_end(builder.get_insert_block().unwrap()); 
                builder.position_at_end(exit_bb);
            },
            Statement::ExpressionStmt(expr) => {
                compile_expr_internal(expr, context, builder, module, variables, struct_types)?;
            },
            _ => {}
        }
    }
    Ok(())
}

fn compile_expr_internal<'ctx>(
    expr: &Expression, 
    context: &'ctx Context, 
    builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
    struct_types: &HashMap<String, StructType<'ctx>>
) -> Result<BasicValueEnum<'ctx>, String> {
    let i64_type = context.i64_type();
    let f64_type = context.f64_type();
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
            let global_str = builder.build_global_string_ptr(&s_with_newline, "aion_str").unwrap();
            Ok(global_str.as_basic_value_enum())
        },
        Expression::Identifier(name) => {
            let (ptr, basic_type) = variables.get(name).ok_or_else(|| format!("Var '{}' not found", name))?;
            Ok(builder.build_load(*basic_type, *ptr, name).unwrap())
        },
        Expression::Call { function, arguments } => {
            if function == "io.println" {
                let printf = module.get_function("printf").ok_or("printf not found")?;
                let arg = compile_expr_internal(&arguments[0], context, builder, module, variables, struct_types)?;
                
                // Ensure arg is a pointer for printf %s
                let printf_arg = if arg.get_type().is_pointer_type() {
                    arg.into()
                } else {
                    // Fallback or cast? For now just try passing it
                    arg.into()
                };
                
                builder.build_call(printf, &[printf_arg], "printftmp").unwrap();
                Ok(i64_type.const_int(0, false).into())
            } else { 
                let fn_val = module.get_function(function).ok_or(format!("Function '{}' not found", function))?;
                let mut compiled_args = Vec::new();
                for arg in arguments {
                    compiled_args.push(compile_expr_internal(arg, context, builder, module, variables, struct_types)?.into());
                }
                let _call = builder.build_call(fn_val, &compiled_args, "calltmp").unwrap();
                Ok(i64_type.const_int(0, false).into())
            }
        },
        Expression::Infix { left, operator, right } => {
            let lhs = compile_expr_internal(left, context, builder, module, variables, struct_types)?.into_int_value();
            let rhs = compile_expr_internal(right, context, builder, module, variables, struct_types)?.into_int_value();
            
            match operator {
                Token::Plus => Ok(builder.build_int_add(lhs, rhs, "addtmp").unwrap().into()),
                Token::Minus => Ok(builder.build_int_sub(lhs, rhs, "subtmp").unwrap().into()),
                Token::Star => Ok(builder.build_int_mul(lhs, rhs, "multmp").unwrap().into()),
                Token::Slash => Ok(builder.build_int_signed_div(lhs, rhs, "divtmp").unwrap().into()),
                Token::Percent => Ok(builder.build_int_signed_rem(lhs, rhs, "remtmp").unwrap().into()),
                
                Token::And => Ok(builder.build_and(lhs, rhs, "andtmp").unwrap().into()),
                Token::Or => Ok(builder.build_or(lhs, rhs, "ortmp").unwrap().into()),
                Token::Bang => {
                    // Logic NOT using XOR with 1
                    Ok(builder.build_xor(rhs, i64_type.const_int(1, false), "nottmp").unwrap().into())
                },
                
                Token::EqEq => {
                    let res = builder.build_int_compare(IntPredicate::EQ, lhs, rhs, "eqtmp").unwrap();
                    Ok(builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                },
                Token::NotEq => {
                    let res = builder.build_int_compare(IntPredicate::NE, lhs, rhs, "netmp").unwrap();
                    Ok(builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                },
                Token::Gt => {
                    let res = builder.build_int_compare(IntPredicate::SGT, lhs, rhs, "gttmp").unwrap();
                    Ok(builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                },
                Token::Lt => {
                    let res = builder.build_int_compare(IntPredicate::SLT, lhs, rhs, "lttmp").unwrap();
                    Ok(builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                },
                Token::GtEq => {
                    let res = builder.build_int_compare(IntPredicate::SGE, lhs, rhs, "getmp").unwrap();
                    Ok(builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                },
                Token::LtEq => {
                    let res = builder.build_int_compare(IntPredicate::SLE, lhs, rhs, "letmp").unwrap();
                    Ok(builder.build_int_z_extend(res, i64_type, "boolcast").unwrap().into())
                },
                
                Token::Caret => {
                    // Simple multiplication hack for power operator
                    Ok(builder.build_int_mul(lhs, rhs, "pow_hack").unwrap().into())
                },
                _ => Err(format!("Operator {:?} not supported", operator)),
            }
        },
        _ => Ok(i64_type.const_int(0, false).into()),
    }
}

pub fn generate_docs(_: &str) -> Result<String, String> { Ok("Documentation placeholder".to_string()) }

pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod types;
pub mod environment;
pub mod checker;

use std::fs;
use std::path::Path;
use std::collections::HashMap;
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, PointerValue, BasicValue, FunctionValue};
use inkwell::types::StructType;
use inkwell::{AddressSpace, IntPredicate};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::ast::*;
use crate::token::Token;

pub fn compile_file(input_path: &str, output_path: &str) -> Result<(), String> {
    let source = fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    let context = Context::create();
    let module = context.create_module("aion_module");
    let builder = context.create_builder();
    let i64_type = context.i64_type();
    let ptr_type = context.ptr_type(AddressSpace::default());
    
    let mut variables = HashMap::new();
    let mut struct_types: HashMap<String, StructType> = HashMap::new();

    let printf_type = context.i32_type().fn_type(&[ptr_type.into()], true);
    module.add_function("printf", printf_type, None);

    let spawn_type = context.void_type().fn_type(&[ptr_type.into()], false);
    module.add_function("aion_spawn", spawn_type, None);

    // Déclaration des types et enums
    for decl in &program.declarations {
        match decl {
            Declaration::Struct(s) => {
                let struct_type = context.opaque_struct_type(&s.name);
                struct_types.insert(s.name.clone(), struct_type);
            },
            _ => {}
        }
    }

    for decl in &program.declarations {
        match decl {
            Declaration::Function(f) => {
                let fn_type = i64_type.fn_type(&[], false);
                let function = module.add_function(&f.name, fn_type, None);

                if let Some(body) = &f.body {
                    let basic_block = context.append_basic_block(function, "entry");
                    builder.position_at_end(basic_block);
                    compile_block(body, &context, &builder, &module, &mut variables, &struct_types, function)?;
                    
                    if basic_block.get_terminator().is_none() {
                        builder.build_return(Some(&i64_type.const_int(0, false))).map_err(|e| e.to_string())?;
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
    variables: &mut HashMap<String, PointerValue<'ctx>>,
    struct_types: &HashMap<String, StructType<'ctx>>,
    function: FunctionValue<'ctx>
) -> Result<(), String> {
    for stmt in body {
        match stmt {
            Statement::Let { name, value, .. } => {
                let val = compile_expr_internal(value, context, builder, module, variables, struct_types)?;
                let alloca = builder.build_alloca(val.get_type(), name).map_err(|e| e.to_string())?;
                builder.build_store(alloca, val).map_err(|e| e.to_string())?;
                variables.insert(name.clone(), alloca);
            },
            Statement::Return { value, .. } => {
                let val = compile_expr_internal(value, context, builder, module, variables, struct_types)?;
                builder.build_return(Some(&val)).map_err(|e| e.to_string())?;
            },
            Statement::If { condition, then_branch, else_branch } => {
                let cond_val = compile_expr_internal(condition, context, builder, module, variables, struct_types)?.into_int_value();
                let comparison = builder.build_int_compare(IntPredicate::NE, cond_val, context.i64_type().const_int(0, false), "ifcond").map_err(|e| e.to_string())?;
                let then_bb = context.append_basic_block(function, "then");
                let else_bb = context.append_basic_block(function, "else");
                let merge_bb = context.append_basic_block(function, "ifcont");
                builder.build_conditional_branch(comparison, then_bb, else_bb).map_err(|e| e.to_string())?;
                builder.position_at_end(then_bb);
                compile_block(then_branch, context, builder, module, variables, struct_types, function)?;
                if then_bb.get_terminator().is_none() { builder.build_unconditional_branch(merge_bb).map_err(|e| e.to_string())?; }
                builder.position_at_end(else_bb);
                if let Some(eb) = else_branch { compile_block(eb, context, builder, module, variables, struct_types, function)?; }
                if else_bb.get_terminator().is_none() { builder.build_unconditional_branch(merge_bb).map_err(|e| e.to_string())?; }
                builder.position_at_end(merge_bb);
            },
            Statement::Match { condition, arms } => {
                let cond_val = compile_expr_internal(condition, context, builder, module, variables, struct_types)?.into_int_value();
                let exit_bb = context.append_basic_block(function, "matchexit");
                
                let mut cases = Vec::new();
                for arm in arms {
                    let arm_bb = context.append_basic_block(function, &format!("arm_{}", arm.pattern));
                    builder.position_at_end(arm_bb);
                    compile_block(&arm.body, context, builder, module, variables, struct_types, function)?;
                    if arm_bb.get_terminator().is_none() { builder.build_unconditional_branch(exit_bb).map_err(|e| e.to_string())?; }
                    
                    // On suppose que le pattern est un entier pour le prototype
                    if let Ok(val) = arm.pattern.parse::<u64>() {
                        cases.push((context.i64_type().const_int(val, false), arm_bb));
                    }
                }
                
                builder.position_at_end(builder.get_insert_block().unwrap()); // Reset builder to before match
                // Note: En vrai, il faudrait que le switch soit généré APRES avoir collecté les cases
                // On va tricher pour le prototype en générant un if/else chain ou un vrai switch si possible
                // TODO: Real LLVM Switch
                builder.position_at_end(exit_bb);
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
    variables: &HashMap<String, PointerValue<'ctx>>,
    struct_types: &HashMap<String, StructType<'ctx>>
) -> Result<BasicValueEnum<'ctx>, String> {
    let i64_type = context.i64_type();
    match expr {
        Expression::Integer(n) => Ok(i64_type.const_int(*n as u64, false).into()),
        Expression::String(s) => {
            let s_with_newline = format!("{}\n\0", s);
            let global_str = builder.build_global_string_ptr(&s_with_newline, "aion_str").map_err(|e| e.to_string())?;
            Ok(global_str.as_basic_value_enum())
        },
        Expression::Identifier(name) => {
            let ptr = variables.get(name).ok_or_else(|| format!("Var '{}' not found", name))?;
            builder.build_load(i64_type, *ptr, name).map_err(|e| e.to_string())
        },
        Expression::Call { function, arguments } => {
            if function == "io.println" {
                let printf = module.get_function("printf").ok_or("printf not found")?;
                let arg = compile_expr_internal(&arguments[0], context, builder, module, variables, struct_types)?;
                builder.build_call(printf, &[arg.into()], "printftmp").map_err(|e| e.to_string())?;
                Ok(i64_type.const_int(0, false).into())
            } else { Err(format!("Function '{}' not supported", function)) }
        },
        _ => Ok(i64_type.const_int(0, false).into()),
    }
}

pub fn generate_docs(_: &str) -> Result<String, String> { Ok("Documentation placeholder".to_string()) }

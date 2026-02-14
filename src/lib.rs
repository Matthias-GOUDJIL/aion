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

    for decl in &program.declarations {
        match decl {
            Declaration::Struct(s) => {
                let struct_type = context.opaque_struct_type(&s.name);
                let mut field_types = Vec::new();
                for _ in &s.fields { field_types.push(i64_type.into()); }
                struct_type.set_body(&field_types, false);
                struct_types.insert(s.name.clone(), struct_type);
            },
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
            }
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
            Statement::ExpressionStmt(expr) => {
                compile_expr_internal(expr, context, builder, module, variables, struct_types)?;
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
            Statement::For { var, range, body } => {
                if let Expression::Range { start, end } = range {
                    let start_val = compile_expr_internal(start, context, builder, module, variables, struct_types)?.into_int_value();
                    let end_val = compile_expr_internal(end, context, builder, module, variables, struct_types)?.into_int_value();
                    
                    let i64_type = context.i64_type();
                    let alloca = builder.build_alloca(i64_type, var).map_err(|e| e.to_string())?;
                    builder.build_store(alloca, start_val).map_err(|e| e.to_string())?;
                    variables.insert(var.clone(), alloca);

                    let loop_bb = context.append_basic_block(function, "loop");
                    let after_bb = context.append_basic_block(function, "afterloop");

                    builder.build_unconditional_branch(loop_bb).map_err(|e| e.to_string())?;
                    builder.position_at_end(loop_bb);

                    // Body
                    compile_block(body, context, builder, module, variables, struct_types, function)?;

                    // Increment
                    let curr = builder.build_load(i64_type, alloca, "counter").map_err(|e| e.to_string())?.into_int_value();
                    let next = builder.build_int_add(curr, i64_type.const_int(1, false), "next").map_err(|e| e.to_string())?;
                    builder.build_store(alloca, next).map_err(|e| e.to_string())?;

                    // Condition
                    let cond = builder.build_int_compare(IntPredicate::SLT, next, end_val, "loopcond").map_err(|e| e.to_string())?;
                    builder.build_conditional_branch(cond, loop_bb, after_bb).map_err(|e| e.to_string())?;

                    builder.position_at_end(after_bb);
                }
            },
            Statement::Spawn(spark_body) => {
                let spark_fn_type = context.void_type().fn_type(&[], false);
                let spark_fn = module.add_function("aion_spark_handler", spark_fn_type, None);
                let spark_bb = context.append_basic_block(spark_fn, "entry");
                let current_bb = builder.get_insert_block().unwrap();
                builder.position_at_end(spark_bb);
                let mut spark_vars = variables.clone();
                compile_block(spark_body, context, builder, module, &mut spark_vars, struct_types, spark_fn)?;
                builder.build_return(None).map_err(|e| e.to_string())?;
                builder.position_at_end(current_bb);
                let spawn_fn = module.get_function("aion_spawn").unwrap();
                builder.build_call(spawn_fn, &[spark_fn.as_global_value().as_pointer_value().into()], "spawntmp").map_err(|e| e.to_string())?;
            }
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
        Expression::Float(f) => Ok(context.f64_type().const_float(*f).into()),
        Expression::Boolean(b) => Ok(context.bool_type().const_int(*b as u64, false).into()),
        Expression::String(s) => {
            let s_with_newline = format!("{}\n\0", s);
            let global_str = builder.build_global_string_ptr(&s_with_newline, "aion_str").map_err(|e| e.to_string())?;
            Ok(global_str.as_basic_value_enum())
        },
        Expression::Identifier(name) => {
            let ptr = variables.get(name).ok_or_else(|| format!("Var '{}' not found", name))?;
            builder.build_load(i64_type, *ptr, name).map_err(|e| e.to_string())
        },
        Expression::Infix { left, operator, right } => {
            let lhs = compile_expr_internal(left, context, builder, module, variables, struct_types)?.into_int_value();
            if *operator == Token::Inside {
                if let Expression::Range { start, end } = &**right {
                    let r_start = compile_expr_internal(start, context, builder, module, variables, struct_types)?.into_int_value();
                    let r_end = compile_expr_internal(end, context, builder, module, variables, struct_types)?.into_int_value();
                    let c1 = builder.build_int_compare(IntPredicate::SGE, lhs, r_start, "ge").map_err(|e| e.to_string())?;
                    let c2 = builder.build_int_compare(IntPredicate::SLE, lhs, r_end, "le").map_err(|e| e.to_string())?;
                    let res = builder.build_and(c1, c2, "inside_res").map_err(|e| e.to_string())?;
                    return Ok(builder.build_int_z_extend(res, i64_type, "zext").map_err(|e| e.to_string())?.into());
                }
                return Err("Operator 'inside' requires a range".to_string());
            }
            let rhs = compile_expr_internal(right, context, builder, module, variables, struct_types)?.into_int_value();
            match operator {
                Token::Plus => Ok(builder.build_int_add(lhs, rhs, "addtmp").map_err(|e| e.to_string())?.into()),
                Token::Minus => Ok(builder.build_int_sub(lhs, rhs, "subtmp").map_err(|e| e.to_string())?.into()),
                Token::EqEq => {
                    let cmp = builder.build_int_compare(IntPredicate::EQ, lhs, rhs, "eqtmp").map_err(|e| e.to_string())?;
                    Ok(builder.build_int_z_extend(cmp, i64_type, "zext").map_err(|e| e.to_string())?.into())
                },
                _ => Err(format!("Operator {:?} not supported", operator)),
            }
        },
        Expression::Call { function, arguments } => {
            if function == "io.println" {
                let printf = module.get_function("printf").ok_or("printf not found")?;
                let arg = compile_expr_internal(&arguments[0], context, builder, module, variables, struct_types)?;
                builder.build_call(printf, &[arg.into()], "printftmp").map_err(|e| e.to_string())?;
                Ok(i64_type.const_int(0, false).into())
            } else { Err(format!("Function '{}' not supported", function)) }
        },
        Expression::Range { .. } => Ok(i64_type.const_int(0, false).into()),
        Expression::StructInst { name, fields } => {
            let st = struct_types.get(name).ok_or_else(|| format!("Struct type '{}' not found", name))?;
            let alloca = builder.build_alloca(*st, "structtmp").map_err(|e| e.to_string())?;
            for (idx, (_f_name, f_val)) in fields.iter().enumerate() {
                let val = compile_expr_internal(f_val, context, builder, module, variables, struct_types)?;
                let field_ptr = builder.build_struct_gep(*st, alloca, idx as u32, "field").map_err(|e| e.to_string())?;
                builder.build_store(field_ptr, val).map_err(|e| e.to_string())?;
            }
            builder.build_load(*st, alloca, "structload").map_err(|e| e.to_string())
        },
    }
}

pub fn generate_docs(input_path: &str) -> Result<String, String> {
    let source = fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    let mut doc = format!("# Module `{}`\n\n", program.module_name.unwrap_or("Main".to_string()));
    for decl in program.declarations {
        match decl {
            Declaration::Function(f) => { doc.push_str(&format!("### Function `{}`\n- Returns: `{}`\n\n", f.name, f.return_type)); },
            Declaration::Struct(s) => { 
                doc.push_str(&format!("### Struct `{}`\n", s.name));
                for (n, t) in &s.fields { doc.push_str(&format!("- `{}`: `{}`\n", n, t)); }
                doc.push_str("\n");
            }
        }
    }
    Ok(doc)
}

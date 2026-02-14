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
use inkwell::values::{BasicValueEnum, PointerValue, BasicValue};
use inkwell::types::StructType;
use inkwell::AddressSpace;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::ast::*;

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

    // Déclaration de printf
    let printf_type = context.i32_type().fn_type(&[ptr_type.into()], true);
    module.add_function("printf", printf_type, None);

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
                    
                    for stmt in body {
                        match stmt {
                            Statement::Let { name, value, .. } => {
                                let val = compile_expr_internal(value, &context, &builder, &module, &variables, &struct_types)?;
                                let alloca = builder.build_alloca(val.get_type(), name).map_err(|e| e.to_string())?;
                                builder.build_store(alloca, val).map_err(|e| e.to_string())?;
                                variables.insert(name.clone(), alloca);
                            },
                            Statement::Return { value, .. } => {
                                let val = compile_expr_internal(value, &context, &builder, &module, &variables, &struct_types)?;
                                builder.build_return(Some(&val)).map_err(|e| e.to_string())?;
                            },
                            Statement::ExpressionStmt(expr) => {
                                compile_expr_internal(expr, &context, &builder, &module, &variables, &struct_types)?;
                            }
                            _ => {}
                        }
                    }
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
            } else {
                Err(format!("Function '{}' not supported", function))
            }
        },
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
        _ => Err("Expression not supported in codegen".to_string()),
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

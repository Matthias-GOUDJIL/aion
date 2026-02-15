pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod types;
pub mod environment;
pub mod checker;
pub mod transpiler;
pub mod compiler;

use std::fs;
use std::path::Path;
use inkwell::context::Context;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::checker::TypeChecker;
use crate::transpiler::sql::SqlTranspiler;
use crate::compiler::Compiler;

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
    let mut compiler = Compiler::new(&context, "aion_module");
    compiler.compile(&program)?;
    compiler.print_to_file(Path::new(output_path))?;

    Ok(())
}

pub fn generate_docs(_: &str) -> Result<String, String> { Ok("Documentation placeholder".to_string()) }

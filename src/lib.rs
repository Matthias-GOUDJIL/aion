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
use crate::ast::Program;

use std::collections::HashSet;
use std::path::PathBuf;

fn resolve_import_path(import_path: &[String]) -> PathBuf {
    let mut path = if let Some(first) = import_path.first() {
        if first == "compiler" { PathBuf::from(".") } else { PathBuf::from("stdlib") }
    } else {
        PathBuf::from("stdlib")
    };
    
    for part in import_path {
        path.push(part);
    }
    path.set_extension("ai");
    path
}

fn process_imports(program: &mut Program, visited: &mut HashSet<PathBuf>) -> Result<(), String> {
    let imports = std::mem::take(&mut program.imports);
    for import in imports {
        let path = resolve_import_path(&import.path);
        if visited.contains(&path) { 
            eprintln!("Visited: {:?}", path);
            continue; 
        }
        
        if !path.exists() {
            return Err(format!("Import not found: {:?}", path));
        }

        eprintln!("Reading: {:?}", path);
        let source = fs::read_to_string(&path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
        let lexer = Lexer::new(&source);
        let mut parser = Parser::new(lexer);
        let mut imported_program = parser.parse_program();
        
        // Rename local declarations before recursion to avoid double-prefixing
        let prefix = import.path.join(".");
        for decl in &mut imported_program.declarations {
            match decl {
                crate::ast::Declaration::Function(f) => { f.name = format!("{}.{}", prefix, f.name); },
                crate::ast::Declaration::Struct(s) => { s.name = format!("{}.{}", prefix, s.name); },
                crate::ast::Declaration::Enum(e) => { e.name = format!("{}.{}", prefix, e.name); },
                crate::ast::Declaration::Impl(i) => { i.target_name = format!("{}.{}", prefix, i.target_name); },
                _ => {}
            }
        }

        visited.insert(path.clone());
        process_imports(&mut imported_program, visited)?;
        
        program.declarations.extend(imported_program.declarations);
    }
    Ok(())
}

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
    let mut program = parser.parse_program();

    // 0. Resolve Imports
    let mut visited = HashSet::new();
    process_imports(&mut program, &mut visited)?;

    // 1. Run Type Checker (Safety Pass)
    let mut checker = TypeChecker::new();
    if let Err(e) = checker.check_program(&program) {
        return Err(format!("Type/Safety Error: {}", e));
    }

    // 2. Run Code Generation
    let context = Context::create();
    let mut compiler = Compiler::new(&context, "aion_module");
    compiler.compile(&program)?;

    // 3. Run Optimization Passes
    compiler.optimize()?;

    compiler.print_to_file(Path::new(output_path))?;

    Ok(())
}

pub fn generate_docs(_: &str) -> Result<String, String> { Ok("Documentation placeholder".to_string()) }

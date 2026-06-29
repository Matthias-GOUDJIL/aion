pub mod analysis;
pub mod ast;
pub mod lexer;
pub mod parser;

pub mod codegen;

pub mod error;

use crate::analysis::checker::TypeChecker;
use crate::ast::Program;
use crate::codegen::compiler::Compiler;
use crate::codegen::transpiler::sql::SqlTranspiler;
use crate::error::CompileError;
use crate::lexer::Lexer;
use crate::parser::Parser;
use inkwell::context::Context;
use std::fs;
use std::path::Path;

use std::collections::HashSet;
use std::path::PathBuf;

fn resolve_import_path(import_path: &[String]) -> PathBuf {
    let mut path = if let Some(first) = import_path.first() {
        if first == "compiler" {
            PathBuf::from(".")
        } else {
            PathBuf::from("stdlib")
        }
    } else {
        PathBuf::from("stdlib")
    };

    for part in import_path {
        path.push(part);
    }
    path.set_extension("ai");
    path
}

fn process_imports(
    program: &mut Program,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), CompileError> {
    let imports = std::mem::take(&mut program.imports);
    for import in imports {
        let path = resolve_import_path(&import.path);
        if visited.contains(&path) {
            continue;
        }

        if !path.exists() {
            return Err(CompileError::import(format!(
                "Import not found: {:?}",
                path
            )));
        }

        let source = fs::read_to_string(&path)
            .map_err(|e| CompileError::io(format!("Failed to read {:?}: {}", path, e)))?;
        let lexer = Lexer::new(&source);
        let mut parser = Parser::new(lexer);
        let mut imported_program = parser.parse_program().map_err(|e| {
            let msgs: Vec<String> = e.iter().map(|e| e.to_string()).collect();
            CompileError::import(format!(
                "Import parse errors in {:?}: {}",
                path,
                msgs.join("; ")
            ))
        })?;

        // Rename local declarations before recursion to avoid double-prefixing
        let prefix = import.path.join(".");
        for decl in &mut imported_program.declarations {
            match decl {
                crate::ast::Declaration::Function(f) => {
                    f.name = format!("{}.{}", prefix, f.name);
                }
                crate::ast::Declaration::Struct(s) => {
                    s.name = format!("{}.{}", prefix, s.name);
                }
                crate::ast::Declaration::Enum(e) => {
                    e.name = format!("{}.{}", prefix, e.name);
                }
                crate::ast::Declaration::Impl(i) => {
                    i.target_name = format!("{}.{}", prefix, i.target_name);
                }
                _ => {}
            }
        }

        visited.insert(path.clone());
        process_imports(&mut imported_program, visited)?;

        program.declarations.extend(imported_program.declarations);
    }
    Ok(())
}

pub fn transpile_sql(input_path: &str) -> Result<String, CompileError> {
    let source = fs::read_to_string(input_path).map_err(|e| CompileError::io(e.to_string()))?;
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program().map_err(|e| CompileError::Type {
        message: format!(
            "Parse errors: {}",
            e.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        ),
        line: 0,
        col: 0,
        snippet: None,
    })?;

    let mut transpiler = SqlTranspiler::new();
    Ok(transpiler.transpile(&program))
}

pub fn compile_file(input_path: &str, output_path: &str) -> Result<(), CompileError> {
    let source = fs::read_to_string(input_path).map_err(|e| CompileError::io(e.to_string()))?;
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let mut program = parser.parse_program().map_err(|e| CompileError::Type {
        message: format!(
            "Parse errors: {}",
            e.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        ),
        line: 0,
        col: 0,
        snippet: None,
    })?;

    // 0. Resolve Imports
    let mut visited = HashSet::new();
    process_imports(&mut program, &mut visited)?;

    // 1. Run Type Checker (Safety Pass)
    let mut checker = TypeChecker::with_source(&source);
    checker.check_program(&program)?;

    // 2. Run Code Generation
    let context = Context::create();
    let mut compiler = Compiler::with_source(&context, "aion_module", &source);
    compiler.compile(&program)?;

    // 3. Run Optimization Passes
    compiler.optimize()?;

    compiler.print_to_file(Path::new(output_path))?;

    Ok(())
}

pub fn generate_docs(input_path: &str) -> Result<String, CompileError> {
    let source = fs::read_to_string(input_path).map_err(|e| CompileError::io(e.to_string()))?;
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program().map_err(|e| CompileError::Type {
        message: format!(
            "Parse errors: {}",
            e.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        ),
        line: 0,
        col: 0,
        snippet: None,
    })?;

    let mut doc = String::new();
    let module_name = program.module_name.as_deref().unwrap_or("Module");
    doc.push_str(&format!("# {}\n\n", module_name));

    if !program.imports.is_empty() {
        doc.push_str("## Imports\n\n");
        for imp in &program.imports {
            let path = imp.path.join(".");
            doc.push_str(&format!("- `{}`\n", path));
        }
        doc.push('\n');
    }

    for decl in &program.declarations {
        match decl {
            ast::Declaration::Function(f) => {
                doc.push_str(&format!("## Function `{}`\n\n", f.name));
                if let Some(ref comment) = f.doc_comment {
                    doc.push_str(&format!("{}\n\n", comment));
                }
                doc.push_str(&generate_function_signature(f));
                doc.push('\n');
            }
            ast::Declaration::Struct(s) => {
                doc.push_str(&format!("## Struct `{}`\n\n", s.name));
                if let Some(ref comment) = s.doc_comment {
                    doc.push_str(&format!("{}\n\n", comment));
                }
                if !s.generic_params.is_empty() {
                    doc.push_str(&format!(
                        "**Generics:** `<{}>`\n\n",
                        s.generic_params.join(", ")
                    ));
                }
                if !s.fields.is_empty() {
                    doc.push_str("| Field | Type |\n|-------|------|\n");
                    for (name, ty) in &s.fields {
                        doc.push_str(&format!("| `{}` | `{}` |\n", name, ty));
                    }
                    doc.push('\n');
                }
            }
            ast::Declaration::Enum(e) => {
                doc.push_str(&format!("## Enum `{}`\n\n", e.name));
                if let Some(ref comment) = e.doc_comment {
                    doc.push_str(&format!("{}\n\n", comment));
                }
                if !e.generic_params.is_empty() {
                    doc.push_str(&format!(
                        "**Generics:** `<{}>`\n\n",
                        e.generic_params.join(", ")
                    ));
                }
                if !e.variants.is_empty() {
                    doc.push_str("| Variant | Data |\n|---------|------|\n");
                    for v in &e.variants {
                        let data = if v.data_types.is_empty() {
                            String::new()
                        } else {
                            format!("`({})`", v.data_types.join(", "))
                        };
                        doc.push_str(&format!("| `{}` | {} |\n", v.name, data));
                    }
                    doc.push('\n');
                }
            }
            ast::Declaration::Interface(iface) => {
                doc.push_str(&format!("## Interface `{}`\n\n", iface.name));
                if let Some(ref comment) = iface.doc_comment {
                    doc.push_str(&format!("{}\n\n", comment));
                }
                for method in &iface.methods {
                    doc.push_str(&format!("### Method `{}`\n\n", method.name));
                    if let Some(ref mc) = method.doc_comment {
                        doc.push_str(&format!("{}\n\n", mc));
                    }
                    doc.push_str(&generate_function_signature(method));
                    doc.push('\n');
                }
            }
            ast::Declaration::Impl(impl_block) => {
                let generics = if impl_block.generic_params.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", impl_block.generic_params.join(", "))
                };
                let iface = impl_block
                    .interface_name
                    .as_deref()
                    .map(|i| format!(" for `{}`", i))
                    .unwrap_or_default();
                doc.push_str(&format!(
                    "## Impl `{}`{}{}\n\n",
                    impl_block.target_name, generics, iface
                ));
                for method in &impl_block.functions {
                    if let Some(ref mc) = method.doc_comment {
                        doc.push_str(&format!("{}\n\n", mc));
                    }
                    doc.push_str(&generate_function_signature(method));
                    doc.push('\n');
                }
            }
        }
    }

    Ok(doc)
}

fn generate_function_signature(f: &ast::Function) -> String {
    let mut sig = String::from("```aion\n");

    let modifiers: Vec<&str> = f
        .modifiers
        .iter()
        .filter_map(|t| match t.kind {
            crate::lexer::TokenKind::Extern => Some("extern"),
            crate::lexer::TokenKind::Spawn => Some("spawn"),
            crate::lexer::TokenKind::Unsafe => Some("unsafe"),
            _ => None,
        })
        .collect();
    if !modifiers.is_empty() {
        sig.push_str(&format!("{} ", modifiers.join(" ")));
    }

    sig.push_str(&format!("fn {}", f.name));

    if !f.generic_params.is_empty() {
        sig.push_str(&format!("<{}>", f.generic_params.join(", ")));
    }

    sig.push('(');
    for (i, (name, ty, default)) in f.params.iter().enumerate() {
        if i > 0 {
            sig.push_str(", ");
        }
        sig.push_str(&format!("{}: {}", name, ty));
        if let Some(default) = default {
            sig.push_str(&format!(" = {:?}", default));
        }
    }
    sig.push(')');

    if f.return_type != "void" && f.return_type != "()" {
        sig.push_str(&format!(" -> {}", f.return_type));
    }

    if f.body.is_none() {
        sig.push(';');
    }

    sig.push_str("\n```\n");
    sig
}

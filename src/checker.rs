use crate::ast::{Statement, Expression, Program, Declaration};
use crate::types::Type;
use crate::environment::Environment;
use crate::token::Token;

use std::collections::HashMap;

pub struct TypeChecker {
    pub env: Environment,
    pub type_params: HashMap<String, Type>, // Mapping from 'T' to concrete type
    in_unsafe_context: bool,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self { 
            env: Environment::new(), 
            type_params: HashMap::new(),
            in_unsafe_context: false 
        };
        checker.register_builtins();
        checker
    }

    fn resolve_type(&self, name: &str) -> Type {
        if let Some(t) = self.type_params.get(name) {
            return t.clone();
        }
        
        match name {
            "i64" => Type::Integer,
            "f64" => Type::Float,
            "bool" => Type::Boolean,
            "String" => Type::String,
            "Date" => Type::Date,
            "Duration" => Type::Duration,
            "void" | "Unit" => Type::Unit,
            _ => {
                if name.contains('<') {
                    // Primitive generic resolution for prototype
                    // e.g. "Vector<i64>"
                    let parts: Vec<&str> = name.split('<').collect();
                    let base = parts[0].to_string();
                    return Type::GenericInstance(base, vec![Type::Unknown]);
                }
                if let Some(t) = self.env.get(name) {
                    return t;
                }
                Type::Placeholder(name.to_string())
            }
        }
    }

    fn register_builtins(&mut self) {
        self.env.set("aion_read_file".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::String) });
        self.env.set("aion_write_file".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::Integer) });
        self.env.set("aion_fs_exists".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::Boolean) });
        self.env.set("aion_getenv".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::String) });
        self.env.set("aion_get_argv_index".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::String) });
        self.env.set("io.println".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Unit) });
        self.env.set("io.print".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Unit) });
        self.env.set("fs.read_to_string".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::String) });
        self.env.set("fs.write".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Integer) });
        self.env.set("fs.exists".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Boolean) });
        self.env.set("env.var".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::GenericInstance("Option".to_string(), vec![Type::String])) });
        self.env.set("mem.is_null".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Boolean) });
        self.env.set("string.len".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Integer) });
        self.env.set("string.concat".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::String) });
        
        // Globals for stdlib
        self.env.set("argc".to_string(), Type::Integer);
        self.env.set("argv".to_string(), Type::String); // Pointer proxy
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), String> {
        // First pass: register functions, structs, and enums
        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => {
                    let is_unsafe = f.modifiers.contains(&Token::Unsafe);
                    let ret_type = self.resolve_type(&f.return_type);
                    self.env.set(f.name.clone(), Type::Function { is_unsafe, return_type: Box::new(ret_type) });
                },
                Declaration::Enum(e) => {
                    self.env.set(e.name.clone(), Type::Enum { name: e.name.clone() });
                },
                Declaration::Impl(i) => {
                    for f in &i.functions {
                        let name = format!("{}.{}", i.target_name, f.name);
                        let is_unsafe = f.modifiers.contains(&Token::Unsafe);
                        let ret_type = self.resolve_type(&f.return_type);
                        self.env.set(name, Type::Function { is_unsafe, return_type: Box::new(ret_type) });
                    }
                },
                _ => {}
            }
        }

        // Second pass: check function bodies
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                // Create a new scope for the function
                let outer_env = self.env.clone();
                self.env = Environment::new_enclosed(outer_env.clone());
                
                // Track if the function itself is unsafe
                let was_in_unsafe = self.in_unsafe_context;
                if f.modifiers.contains(&Token::Unsafe) {
                    self.in_unsafe_context = true;
                }
                
                // Add parameters to scope
                if f.name == "main" {
                    self.env.set("argc".to_string(), Type::Integer);
                    self.env.set("argv".to_string(), Type::String); // Using String as a proxy for ptr
                } else {
                    for (param_name, param_type) in &f.params {
                        self.env.set(param_name.clone(), self.resolve_type(param_type)); 
                    }
                }
                
                if let Some(body) = &f.body {
                    for stmt in body {
                        self.check_statement(stmt)?;
                    }
                }
                
                // Restore state
                self.in_unsafe_context = was_in_unsafe;
                self.env = outer_env;
            }
        }
        Ok(())
    }

    fn check_statement(&mut self, stmt: &Statement) -> Result<Type, String> {
        match stmt {
            Statement::Let { name, value, .. } => {
                let value_type = self.check_expression(value)?;
                self.env.set(name.clone(), value_type);
                Ok(Type::Unit)
            },
            Statement::Assignment { target, value } => {
                self.check_expression(target)?;
                self.check_expression(value)?;
                Ok(Type::Unit)
            },
            Statement::Return { value, .. } => self.check_expression(value),
            Statement::ExpressionStmt(expr) => self.check_expression(expr),
            Statement::If { condition, then_branch, else_branch } => {
                self.check_expression(condition)?;
                for s in then_branch { self.check_statement(s)?; }
                if let Some(eb) = else_branch {
                    for s in eb { self.check_statement(s)?; }
                }
                Ok(Type::Unit)
            },
            Statement::While { condition, body } => {
                self.check_expression(condition)?;
                for s in body { self.check_statement(s)?; }
                Ok(Type::Unit)
            },
            Statement::UnsafeBlock(body) => {
                let was_in_unsafe = self.in_unsafe_context;
                self.in_unsafe_context = true;
                for s in body {
                    self.check_statement(s)?;
                }
                self.in_unsafe_context = was_in_unsafe;
                Ok(Type::Unit)
            },
            Statement::Match { condition, arms } => {
                let _cond_type = self.check_expression(condition)?;
                for arm in arms {
                    // Create scope for the arm if it has parameters
                    let outer_env = self.env.clone();
                    if !arm.params.is_empty() {
                        self.env = Environment::new_enclosed(outer_env.clone());
                        for param in &arm.params {
                            // Heuristic for prototype: if pattern is Some or Ok, it's likely a String payload
                            let param_type = if arm.pattern == "Some" || arm.pattern == "Ok" {
                                Type::String
                            } else {
                                Type::Integer
                            };
                            self.env.set(param.clone(), param_type);
                        }
                    }
                    
                    for s in &arm.body { self.check_statement(s)?; }
                    
                    // Restore scope
                    self.env = outer_env;
                }
                Ok(Type::Unit)
            },
            Statement::NoOp => Ok(Type::Unit),
            _ => Ok(Type::Unit),
        }
    }

    fn check_expression(&mut self, expr: &Expression) -> Result<Type, String> {
        match expr {
            Expression::Integer(_) => Ok(Type::Integer),
            Expression::Float(_) => Ok(Type::Float),
            Expression::Boolean(_) => Ok(Type::Boolean),
            Expression::String(_) => Ok(Type::String),
            Expression::Duration(_, _) => Ok(Type::Duration),
            Expression::Date(_) => Ok(Type::Date),
            Expression::Identifier(name) => {
                self.env.get(name).ok_or(format!("Error: Variable '{}' not defined.", name))
            },
            Expression::Call { function, arguments, .. } => {
                let func_type = if let Some(ft) = self.env.get(function) {
                    ft
                } else if let Some((var_name, method_name)) = function.split_once('.') {
                    if let Some(var_type) = self.env.get(var_name) {
                        let type_name = match var_type {
                            Type::GenericInstance(name, _) => name,
                            Type::Enum { name } => name,
                            Type::Placeholder(name) => name,
                            Type::String => "String".to_string(),
                            _ => "Unknown".to_string(),
                        };
                        let candidate = format!("{}.{}", type_name, method_name);
                        self.env.get(&candidate).ok_or(format!("Error: Method '{}' not defined for type '{}'.", method_name, type_name))?
                    } else {
                         return Err(format!("Error: Variable '{}' not defined.", var_name));
                    }
                } else {
                    return Err(format!("Error: Function '{}' not defined.", function));
                };

                if let Type::Function { is_unsafe, return_type } = func_type {
                    if is_unsafe && !self.in_unsafe_context {
                        return Err(format!("Security Error: Call to unsafe function '{}' requires an unsafe block.", function));
                    }
                    for arg in arguments { self.check_expression(arg)?; }
                    Ok(*return_type.clone())
                } else {
                    Err(format!("Error: '{}' is not a function.", function))
                }
            },
            Expression::Infix { left, operator, right } => {
                let t1 = self.check_expression(left)?;
                let t2 = self.check_expression(right)?;
                self.check_compatibility(t1, t2, operator)
            },
            Expression::If { condition, then_branch, else_branch } => {
                self.check_expression(condition)?;
                for s in then_branch { self.check_statement(s)?; }
                if let Some(eb) = else_branch {
                    for s in eb { self.check_statement(s)?; }
                }
                Ok(Type::Unknown) // Should return unified type of branches
            },
            Expression::Cast { expr, target } => {
                self.check_expression(expr)?;
                Ok(self.resolve_type(target))
            },
            Expression::Deref { expr } => {
                self.check_expression(expr)?;
                Ok(Type::Unknown)
            },
            Expression::Block { statements, is_unsafe } => {
                let was_in_unsafe = self.in_unsafe_context;
                if *is_unsafe { self.in_unsafe_context = true; }
                
                let mut last_type = Type::Unit;
                for stmt in statements {
                    last_type = self.check_statement(stmt)?;
                }
                
                self.in_unsafe_context = was_in_unsafe;
                Ok(last_type)
            },
            Expression::Intrinsic { name: _, arguments } => {
                for arg in arguments { self.check_expression(arg)?; }
                Ok(Type::Unknown)
            },
            Expression::EnumInst { name, variant: _, arguments, .. } => {
                let enum_type = self.env.get(name).ok_or(format!("Error: Enum '{}' not defined.", name))?;
                for arg in arguments { self.check_expression(arg)?; }
                Ok(enum_type)
            },
            _ => Ok(Type::Unknown),
        }
    }

    fn check_compatibility(&self, t1: Type, t2: Type, op: &Token) -> Result<Type, String> {
        match (t1, t2) {
            (Type::Integer, Type::Integer) => Ok(Type::Integer),
            (Type::Boolean, Type::Boolean) => Ok(Type::Boolean),
            (Type::Date, Type::Duration) if *op == Token::Plus => Ok(Type::Date),
            _ => Ok(Type::Unknown),
        }
    }
}

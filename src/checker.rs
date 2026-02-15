use crate::ast::{Statement, Expression, Program, Declaration};
use crate::types::Type;
use crate::environment::Environment;
use crate::token::Token;

pub struct TypeChecker {
    pub env: Environment,
    in_unsafe_context: bool,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self { env: Environment::new(), in_unsafe_context: false };
        checker.register_builtins();
        checker
    }

    fn register_builtins(&mut self) {
        self.env.set("aion_read_file".to_string(), Type::Function { is_unsafe: true });
        self.env.set("aion_write_file".to_string(), Type::Function { is_unsafe: true });
        self.env.set("aion_get_argv_index".to_string(), Type::Function { is_unsafe: true });
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), String> {
        // First pass: register functions and their safety status
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                let is_unsafe = f.modifiers.contains(&Token::Unsafe);
                self.env.set(f.name.clone(), Type::Function { is_unsafe });
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
                    for (param_name, _) in &f.params {
                        self.env.set(param_name.clone(), Type::Integer); 
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
            Statement::UnsafeBlock(body) => {
                let was_in_unsafe = self.in_unsafe_context;
                self.in_unsafe_context = true;
                for s in body {
                    self.check_statement(s)?;
                }
                self.in_unsafe_context = was_in_unsafe;
                Ok(Type::Unit)
            }
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
            Expression::Call { function, arguments } => {
                if function == "io.println" { return Ok(Type::Unit); }
                
                let func_type = self.env.get(function).ok_or(format!("Error: Function '{}' not defined.", function))?;
                if let Type::Function { is_unsafe } = func_type {
                    if is_unsafe && !self.in_unsafe_context {
                        return Err(format!("Security Error: Call to unsafe function '{}' requires an unsafe block.", function));
                    }
                }

                for arg in arguments { self.check_expression(arg)?; }
                Ok(Type::Unknown) 
            },
            Expression::Infix { left, operator, right } => {
                let t1 = self.check_expression(left)?;
                let t2 = self.check_expression(right)?;
                self.check_compatibility(t1, t2, operator)
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

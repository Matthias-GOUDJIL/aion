use crate::ast::{Statement, Expression, Program, Declaration};
use crate::types::Type;
use crate::environment::Environment;
use crate::token::Token;

pub struct TypeChecker {
    pub env: Environment,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self { env: Environment::new() }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), String> {
        // First pass: register functions
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                self.env.set(f.name.clone(), Type::Function { is_unsafe: false });
            }
        }

        // Second pass: check function bodies
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                // Create a new scope for the function
                let outer_env = self.env.clone();
                self.env = Environment::new_enclosed(outer_env.clone());
                
                // Add parameters to scope
                for (param_name, _) in &f.params {
                    self.env.set(param_name.clone(), Type::Integer); 
                }
                
                if let Some(body) = &f.body {
                    for stmt in body {
                        self.check_statement(stmt)?;
                    }
                }
                
                // Restore outer scope
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
                for arg in arguments { self.check_expression(arg)?; }
                Ok(Type::Unknown) 
            },
            Expression::Infix { left, operator, right } => {
                let t1 = self.check_expression(left)?;
                let t2 = self.check_expression(right)?;
                self.check_compatibility(t1, t2, operator)
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

use crate::ast::{Statement, Expression};
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

    pub fn check_program(&mut self, statements: &[Statement]) -> Result<(), String> {
        for stmt in statements {
            self.check_statement(stmt)?;
        }
        Ok(())
    }

    fn check_statement(&mut self, stmt: &Statement) -> Result<Type, String> {
        match stmt {
            Statement::Let { name, value, intent, .. } => {
                let value_type = self.check_expression(value)?;
                if let Some(msg) = intent {
                    println!("AI Intent Analysis for '{}': {}", name, msg);
                }
                self.env.set(name.clone(), value_type);
                Ok(Type::Unit)
            },
            Statement::Return { value, .. } => self.check_expression(value),
            Statement::ExpressionStmt(expr) => self.check_expression(expr),
            _ => Ok(Type::Unit),
        }
    }

    fn check_expression(&mut self, expr: &Expression) -> Result<Type, String> {
        match expr {
            Expression::Integer(_) => Ok(Type::Integer),
            Expression::Float(_) => Ok(Type::Float),
            Expression::String(_) => Ok(Type::String),
            Expression::Identifier(name) => {
                self.env.get(name).ok_or(format!("Error: Variable '{}' not defined.", name))
            },
            Expression::Infix { left, operator, right } => {
                let t1 = self.check_expression(left)?;
                let t2 = self.check_expression(right)?;
                self.check_compatibility(t1, t2, operator)
            }
            _ => Ok(Type::Unknown),
        }
    }

    fn check_compatibility(&self, t1: Type, t2: Type, op: &Token) -> Result<Type, String> {
        match (t1, t2) {
            (Type::Integer, Type::Integer) => Ok(Type::Integer),
            (Type::Float, Type::Float) => Ok(Type::Float),
            (Type::String, Type::String) if *op == Token::Plus => Ok(Type::String),
            (t1, t2) => Err(format!("Type Mismatch: Cannot use {:?} with {:?} and {:?}", t1, op, t2)),
        }
    }
}

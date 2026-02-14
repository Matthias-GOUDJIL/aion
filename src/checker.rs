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
            Statement::If { condition, then_branch, else_branch } => {
                self.check_expression(condition)?;
                for s in then_branch { self.check_statement(s)?; }
                if let Some(eb) = else_branch {
                    for s in eb { self.check_statement(s)?; }
                }
                Ok(Type::Unit)
            },
            Statement::For { var, range, body } => {
                self.check_expression(range)?;
                // Créer un scope pour la boucle
                self.env.set(var.clone(), Type::Integer);
                for s in body { self.check_statement(s)?; }
                Ok(Type::Unit)
            },
            Statement::Spawn(body) => {
                for s in body { self.check_statement(s)?; }
                Ok(Type::Unit)
            },
        }
    }

    fn check_expression(&mut self, expr: &Expression) -> Result<Type, String> {
        match expr {
            Expression::Integer(_) => Ok(Type::Integer),
            Expression::Float(_) => Ok(Type::Float),
            Expression::Boolean(_) => Ok(Type::Boolean),
            Expression::String(_) => Ok(Type::String),
            Expression::Identifier(name) => {
                self.env.get(name).ok_or(format!("Error: Variable '{}' not defined.", name))
            },
            Expression::Infix { left, operator, right } => {
                let t1 = self.check_expression(left)?;
                let t2 = self.check_expression(right)?;
                if *operator == Token::Inside {
                    return Ok(Type::Boolean);
                }
                self.check_compatibility(t1, t2, operator)
            },
            Expression::Range { start, end } => {
                let t1 = self.check_expression(start)?;
                let t2 = self.check_expression(end)?;
                if t1 == t2 { Ok(t1) } else { Err("Range types mismatch".to_string()) }
            },
            Expression::Call { .. } => Ok(Type::Unknown),
            Expression::StructInst { .. } => Ok(Type::Unknown),
        }
    }

    fn check_compatibility(&self, t1: Type, t2: Type, op: &Token) -> Result<Type, String> {
        match (t1, t2) {
            (Type::Integer, Type::Integer) => {
                match op {
                    Token::EqEq | Token::NotEq | Token::Gt | Token::Lt => Ok(Type::Boolean),
                    _ => Ok(Type::Integer)
                }
            },
            (Type::Float, Type::Float) => Ok(Type::Float),
            (Type::String, Type::String) if *op == Token::Plus => Ok(Type::String),
            (t1, t2) => Err(format!("Type Mismatch: Cannot use {:?} with {:?} and {:?}", t1, op, t2)),
        }
    }
}

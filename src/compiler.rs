use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, PointerValue};
use std::collections::HashMap;

use crate::ast::{Expression, Statement};
use crate::token::Token;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub builder: &'ctx Builder<'ctx>,
    pub module: &'ctx Module<'ctx>,
    variables: HashMap<String, PointerValue<'ctx>>,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, builder: &'ctx Builder<'ctx>, module: &'ctx Module<'ctx>) -> Self {
        Self { context, builder, module, variables: HashMap::new() }
    }

    pub fn compile_program(&mut self, statements: &[Statement]) -> Result<(), String> {
        let i64_type = self.context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = self.module.add_function("main", fn_type, None);
        let basic_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(basic_block);

        for stmt in statements {
            self.compile_stmt(stmt)?;
        }

        self.builder.build_return(Some(&i64_type.const_int(0, false))).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Statement) -> Result<(), String> {
        match stmt {
            Statement::Let { name, value, .. } => {
                let val = self.compile_expr(value)?;
                let i64_type = self.context.i64_type();
                let alloca = self.builder.build_alloca(i64_type, name).map_err(|e| e.to_string())?;
                self.builder.build_store(alloca, val).map_err(|e| e.to_string())?;
                self.variables.insert(name.clone(), alloca);
                Ok(())
            },
            _ => Ok(()),
        }
    }

    fn compile_expr(&mut self, expr: &Expression) -> Result<BasicValueEnum<'ctx>, String> {
        match expr {
            Expression::Integer(n) => Ok(self.context.i64_type().const_int(*n as u64, false).into()),
            Expression::Identifier(name) => {
                let ptr = self.variables.get(name).ok_or_else(|| format!("Variable '{}' not found in codegen", name))?;
                let i64_type = self.context.i64_type();
                self.builder.build_load(i64_type, *ptr, name).map_err(|e| e.to_string())
            },
            Expression::Infix { left, operator, right } => {
                let lhs = self.compile_expr(left)?.into_int_value();
                let rhs = self.compile_expr(right)?.into_int_value();
                match operator {
                    Token::Plus => Ok(self.builder.build_int_add(lhs, rhs, "addtmp").map_err(|e| e.to_string())?.into()),
                    Token::Minus => Ok(self.builder.build_int_sub(lhs, rhs, "subtmp").map_err(|e| e.to_string())?.into()),
                    _ => Err(format!("Operator {:?} not supported in codegen yet", operator)),
                }
            }
            _ => Err("Expression type not supported in codegen yet".to_string()),
        }
    }
}

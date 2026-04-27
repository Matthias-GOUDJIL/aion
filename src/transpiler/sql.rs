use crate::ast::*;
use crate::token::TokenKind;

pub struct SqlTranspiler {
    buffer: String,
}

impl Default for SqlTranspiler {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlTranspiler {
    pub fn new() -> Self {
        Self { buffer: String::new() }
    }

    pub fn transpile(&mut self, program: &Program) -> String {
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                self.transpile_function(f);
            }
        }
        self.buffer.clone()
    }

    fn transpile_function(&mut self, f: &Function) {
        let ret_type = self.map_type(&f.return_type);
        self.buffer.push_str(&format!("CREATE OR REPLACE FUNCTION {}(", f.name));

        let params: Vec<String> = f
            .params
            .iter()
            .map(|(n, t)| format!("{} {}", n, self.map_type(t)))
            .collect();

        self.buffer.push_str(&params.join(", "));
        self.buffer.push_str(&format!(") RETURNS {} AS $$\n", ret_type));

        // Collect variable declarations from Let statements
        let vars = self.collect_vars(&f.body);
        if !vars.is_empty() {
            self.buffer.push_str("DECLARE\n");
            for (name, vtype) in &vars {
                self.buffer.push_str(&format!("    {} {};\n", name, self.map_type(vtype)));
            }
        }

        self.buffer.push_str("BEGIN\n");

        if let Some(body) = &f.body {
            for stmt in body {
                self.transpile_statement(stmt);
            }
        }

        self.buffer.push_str("END;\n$$ LANGUAGE plpgsql;\n\n");
    }

    fn collect_vars(&self, body: &Option<Vec<Statement>>) -> Vec<(String, String)> {
        let mut vars = Vec::new();
        if let Some(stmts) = body {
            self.collect_vars_from_stmts(stmts, &mut vars);
        }
        vars
    }

    fn collect_vars_from_stmts(&self, stmts: &[Statement], vars: &mut Vec<(String, String)>) {
        for stmt in stmts {
            match stmt {
                Statement::Let {
                    name, value, ..
                }
                    if !vars.iter().any(|(n, _)| n == name) => {
                        let vtype = self.infer_type(value);
                        vars.push((name.clone(), vtype));
                    }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.collect_vars_from_stmts(then_branch, vars);
                    if let Some(eb) = else_branch {
                        self.collect_vars_from_stmts(eb, vars);
                    }
                }
                Statement::While { body, .. } => {
                    self.collect_vars_from_stmts(body, vars);
                }
                Statement::Match { arms, .. } => {
                    for arm in arms {
                        self.collect_vars_from_stmts(&arm.body, vars);
                    }
                }
                Statement::UnsafeBlock(stmts) | Statement::Spawn(stmts) => {
                    self.collect_vars_from_stmts(stmts, vars);
                }
                _ => {}
            }
        }
    }

    fn infer_type(&self, expr: &Expression) -> String {
        match expr {
            Expression::Integer(_) => "i64",
            Expression::Float(_) => "f64",
            Expression::Boolean(_) => "bool",
            Expression::String(_) => "String",
            Expression::Identifier(_) => "i64",
            Expression::Infix { .. } => "i64",
            Expression::Call { function, .. } => {
                if function.contains("println") || function.contains("print") {
                    "void"
                } else {
                    "i64"
                }
            }
            Expression::Cast { target, .. } => target,
            Expression::MemberAccess { .. } => "i64",
            Expression::MethodCall { .. } => "i64",
            Expression::Block { .. } => "void",
            Expression::If { .. } => "void",
            Expression::EnumInst { .. } => "i64",
            Expression::StructInst { .. } => "i64",
            Expression::Range { .. } => "i64",
            Expression::Deref { .. } => "i64",
            Expression::Intrinsic { .. } => "i64",
            Expression::TypeRef { .. } => "i64",
            Expression::Duration(_, _) => "Duration",
            Expression::Date(_) => "Date",
        }
        .to_string()
    }

    fn map_type(&self, t: &str) -> String {
        let clean = t.replace(" ", "");
        match clean.as_str() {
            "i64" | "u64" | "i32" | "u32" | "i8" | "u8" => "BIGINT",
            "f64" | "f32" => "DOUBLE PRECISION",
            "bool" => "BOOLEAN",
            "String" => "TEXT",
            "Date" => "TIMESTAMP",
            "Duration" => "INTERVAL",
            "void" | "Unit" => "VOID",
            _ => "JSONB",
        }
        .to_string()
    }

    fn transpile_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Return { value, .. } => {
                self.buffer.push_str("    RETURN ");
                self.transpile_expression(value);
                self.buffer.push_str(";\n");
            }
            Statement::Let { name, value, .. } => {
                self.buffer.push_str(&format!("    {} := ", name));
                self.transpile_expression(value);
                self.buffer.push_str(";\n");
            }
            Statement::Assignment { target, value } => {
                self.buffer.push_str("    ");
                self.transpile_expression(target);
                self.buffer.push_str(" := ");
                self.transpile_expression(value);
                self.buffer.push_str(";\n");
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.buffer.push_str("    IF ");
                self.transpile_expression(condition);
                self.buffer.push_str(" THEN\n");
                for s in then_branch {
                    self.transpile_statement(s);
                }
                if let Some(eb) = else_branch {
                    self.buffer.push_str("    ELSE\n");
                    for s in eb {
                        self.transpile_statement(s);
                    }
                }
                self.buffer.push_str("    END IF;\n");
            }
            Statement::While { condition, body } => {
                self.buffer.push_str("    WHILE ");
                self.transpile_expression(condition);
                self.buffer.push_str(" LOOP\n");
                for s in body {
                    self.transpile_statement(s);
                }
                self.buffer.push_str("    END LOOP;\n");
            }
            Statement::ExpressionStmt(expr) => {
                self.buffer.push_str("    PERFORM ");
                self.transpile_expression(expr);
                self.buffer.push_str(";\n");
            }
            Statement::Match { condition, arms } => {
                self.buffer.push_str("    CASE ");
                self.transpile_expression(condition);
                self.buffer.push('\n');
                for arm in arms {
                    self.buffer
                        .push_str(&format!("        WHEN '{}' THEN\n", arm.pattern));
                    for s in &arm.body {
                        self.transpile_statement(s);
                    }
                }
                self.buffer.push_str("        ELSE NULL;\n");
                self.buffer.push_str("    END CASE;\n");
            }
            Statement::UnsafeBlock(stmts) => {
                self.buffer.push_str("    -- UNSAFE BLOCK\n");
                for s in stmts {
                    self.transpile_statement(s);
                }
            }
            Statement::NoOp => {}
            _ => self.buffer.push_str("    -- Unsupported statement\n"),
        }
    }

    fn transpile_expression(&mut self, expr: &Expression) {
        match expr {
            Expression::Integer(n) => self.buffer.push_str(&n.to_string()),
            Expression::Float(f_val) => self.buffer.push_str(&f_val.to_string()),
            Expression::Boolean(b) => self.buffer.push_str(&b.to_string().to_uppercase()),
            Expression::String(s) => self.buffer.push_str(&format!("'{}'", s)),
            Expression::Identifier(s) => self.buffer.push_str(s),
            Expression::Infix {
                left,
                operator,
                right,
            } => {
                self.transpile_expression(left);
                let op = match &operator.kind {
                    TokenKind::Plus => "+",
                    TokenKind::Minus => "-",
                    TokenKind::Star => "*",
                    TokenKind::Slash => "/",
                    TokenKind::EqEq => "=",
                    TokenKind::NotEq => "!=",
                    TokenKind::Gt => ">",
                    TokenKind::Lt => "<",
                    TokenKind::GtEq => ">=",
                    TokenKind::LtEq => "<=",
                    TokenKind::And => "AND",
                    TokenKind::Or => "OR",
                    _ => "?",
                };
                self.buffer.push_str(&format!(" {} ", op));
                self.transpile_expression(right);
            }
            Expression::Call {
                function,
                arguments,
                ..
            } => {
                self.buffer.push_str(&format!("{}(", function));
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        self.buffer.push_str(", ");
                    }
                    self.transpile_expression(arg);
                }
                self.buffer.push(')');
            }
            Expression::MemberAccess {
                receiver,
                member,
            } => {
                self.transpile_expression(receiver);
                self.buffer.push('.');
                self.buffer.push_str(member);
            }
            Expression::Cast { expr, target } => {
                self.transpile_expression(expr);
                self.buffer
                    .push_str(&format!("::{}", self.map_type(target)));
            }
            Expression::Block { statements, .. } => {
                for (i, s) in statements.iter().enumerate() {
                    if i > 0 {
                        self.buffer.push_str("; ");
                    }
                    self.transpile_statement_for_expr(s);
                }
            }
            _ => self.buffer.push_str("NULL"),
        }
    }

    fn transpile_statement_for_expr(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Return { value, .. } => {
                self.transpile_expression(value);
            }
            Statement::Let { value, .. } => {
                self.transpile_expression(value);
            }
            Statement::ExpressionStmt(expr) => {
                self.transpile_expression(expr);
            }
            _ => self.buffer.push_str("NULL"),
        }
    }
}

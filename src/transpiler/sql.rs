use crate::ast::*;
use crate::token::Token;

pub struct SqlTranspiler {
    buffer: String,
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
        
        let params: Vec<String> = f.params.iter()
            .map(|(n, t)| format!("{} {}", n, self.map_type(t)))
            .collect();
        
        self.buffer.push_str(&params.join(", "));
        self.buffer.push_str(&format!(") RETURNS {} AS $$
BEGIN
", ret_type));
        
        if let Some(body) = &f.body {
            for stmt in body {
                self.transpile_statement(stmt);
            }
        }
        
        self.buffer.push_str("END;
$$ LANGUAGE plpgsql;

");
    }

    fn map_type(&self, t: &str) -> String {
        match t {
            "i64" => "BIGINT".to_string(),
            "f64" => "DOUBLE PRECISION".to_string(),
            "bool" => "BOOLEAN".to_string(),
            "String" => "TEXT".to_string(),
            "Date" => "TIMESTAMP".to_string(),
            "Duration" => "INTERVAL".to_string(),
            "void" => "VOID".to_string(),
            _ => "JSONB".to_string(),
        }
    }

    fn transpile_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Return { value, .. } => {
                self.buffer.push_str("    RETURN ");
                self.transpile_expression(value);
                self.buffer.push_str(";
");
            },
            Statement::Let { name, value, .. } => {
                self.buffer.push_str(&format!("    {} := ", name));
                self.transpile_expression(value);
                self.buffer.push_str(";
");
            },
            Statement::If { condition, then_branch, else_branch } => {
                self.buffer.push_str("    IF ");
                self.transpile_expression(condition);
                self.buffer.push_str(" THEN
");
                for s in then_branch {
                    self.transpile_statement(s);
                }
                if let Some(eb) = else_branch {
                    self.buffer.push_str("    ELSE
");
                    for s in eb {
                        self.transpile_statement(s);
                    }
                }
                self.buffer.push_str("    END IF;
");
            },
            Statement::ExpressionStmt(expr) => {
                self.buffer.push_str("    PERFORM ");
                self.transpile_expression(expr);
                self.buffer.push_str(";
");
            },
            Statement::Match { condition, arms } => {
                self.buffer.push_str("    CASE ");
                self.transpile_expression(condition);
                self.buffer.push_str("
");
                for arm in arms {
                    self.buffer.push_str(&format!("        WHEN '{}' THEN
", arm.pattern));
                    for s in &arm.body {
                        self.transpile_statement(s);
                    }
                }
                self.buffer.push_str("    END CASE;
");
            },
            _ => self.buffer.push_str("    -- Unsupported statement
"),
        }
    }

    fn transpile_expression(&mut self, expr: &Expression) {
        match expr {
            Expression::Integer(n) => self.buffer.push_str(&n.to_string()),
            Expression::Boolean(b) => self.buffer.push_str(&b.to_string().to_uppercase()),
            Expression::Identifier(s) => self.buffer.push_str(s),
            Expression::Infix { left, operator, right } => {
                self.transpile_expression(left);
                let op = match operator {
                    Token::Plus => "+",
                    Token::Minus => "-",
                    Token::Star => "*",
                    Token::Slash => "/",
                    Token::EqEq => "=",
                    Token::NotEq => "!=",
                    Token::Gt => ">",
                    Token::Lt => "<",
                    Token::GtEq => ">=",
                    Token::LtEq => "<=",
                    _ => "?",
                };
                self.buffer.push_str(&format!(" {} ", op));
                self.transpile_expression(right);
            },
            Expression::Call { function, arguments } => {
                self.buffer.push_str(&format!("{}(", function));
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 { self.buffer.push_str(", "); }
                    self.transpile_expression(arg);
                }
                self.buffer.push_str(")");
            },
            _ => self.buffer.push_str("NULL"),
        }
    }
}

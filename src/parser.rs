use crate::token::Token;
use crate::lexer::Lexer;
use crate::ast::*;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    peek_buffer: Vec<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next_token();
        Self { lexer, current_token, peek_buffer: Vec::new() }
    }

    fn next_token(&mut self) {
        if !self.peek_buffer.is_empty() {
            self.current_token = self.peek_buffer.remove(0);
        } else {
            self.current_token = self.lexer.next_token();
        }
    }

    fn peek_at(&mut self, n: usize) -> Token {
        while self.peek_buffer.len() <= n {
            self.peek_buffer.push(self.lexer.next_token());
        }
        self.peek_buffer[n].clone()
    }

    pub fn parse_program(&mut self) -> Program {
        let mut module_name = None;
        let mut imports = Vec::new();
        let mut declarations = Vec::new();

        while self.current_token != Token::EOF {
            match self.current_token {
                Token::Identifier(ref id) if id == "module" => {
                    self.next_token();
                    let path = self.parse_path();
                    module_name = Some(path.join("."));
                    if self.current_token == Token::Semicolon { self.next_token(); }
                },
                Token::Use => imports.push(self.parse_import()),
                _ => {
                    if let Some(decl) = self.parse_declaration() {
                        declarations.push(decl);
                    } else {
                        self.next_token();
                    }
                },
            }
        }
        Program { module_name, imports, declarations }
    }

    fn parse_path(&mut self) -> Vec<String> {
        let mut path = Vec::new();
        if let Token::Identifier(id) = &self.current_token {
            path.push(id.clone());
            self.next_token();
            while self.current_token == Token::Dot {
                self.next_token();
                if let Token::Identifier(sub) = &self.current_token { 
                    path.push(sub.clone()); 
                    self.next_token(); 
                } else {
                    break;
                }
            }
        }
        path
    }

    fn parse_import(&mut self) -> Import {
        self.next_token();
        let path = self.parse_path();
        if self.current_token == Token::Semicolon { self.next_token(); }
        Import { path, alias: None }
    }

    fn parse_declaration(&mut self) -> Option<Declaration> {
        let mut attributes = Vec::new();
        while self.current_token == Token::At {
            self.next_token();
            if let Token::Identifier(name) = &self.current_token {
                let attr_name = name.clone();
                self.next_token();
                let mut attr_val = String::new();
                if self.current_token == Token::LParen {
                    self.next_token();
                    if let Token::StringLiteral(s) = &self.current_token { 
                        attr_val = s.clone(); 
                        self.next_token(); 
                    }
                    if self.current_token == Token::RParen { self.next_token(); }
                }
                attributes.push((attr_name, attr_val));
            } else {
                break;
            }
        }
        let mut modifiers = Vec::new();
        while matches!(self.current_token, Token::Pub | Token::Async | Token::Unsafe) {
            modifiers.push(self.current_token.clone());
            self.next_token();
        }
        match self.current_token {
            Token::Fn => self.parse_function(modifiers, attributes).map(Declaration::Function),
            Token::Struct => self.parse_struct(attributes).map(Declaration::Struct),
            Token::Enum => self.parse_enum().map(Declaration::Enum),
            Token::Interface => self.parse_interface().map(Declaration::Interface),
            Token::Impl => self.parse_impl().map(Declaration::Impl),
            _ => None,
        }
    }

    fn parse_impl(&mut self) -> Option<ImplBlock> {
        self.next_token();
        let name1 = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        let generic_params = self.parse_generic_params();
        
        let (interface_name, target_name) = if let Token::Identifier(ref id) = self.current_token {
            if id == "for" {
                self.next_token();
                let target = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
                self.next_token();
                (Some(name1), target)
            } else { (None, name1) }
        } else { (None, name1) };

        if self.current_token != Token::LBrace { return None; }
        self.next_token();
        let mut functions = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            let mut modifiers = Vec::new();
            while matches!(self.current_token, Token::Pub | Token::Async | Token::Unsafe) {
                modifiers.push(self.current_token.clone());
                self.next_token();
            }
            if self.current_token == Token::Fn {
                if let Some(f) = self.parse_function(modifiers, vec![]) { functions.push(f); }
            } else {
                self.next_token();
            }
        }
        if self.current_token == Token::RBrace { self.next_token(); }
        Some(ImplBlock { target_name, generic_params, interface_name, functions })
    }

    fn parse_generic_params(&mut self) -> Vec<String> {
        let mut params = Vec::new();
        if self.current_token == Token::Lt {
            self.next_token();
            while self.current_token != Token::Gt && self.current_token != Token::EOF {
                if let Token::Identifier(id) = &self.current_token { params.push(id.clone()); self.next_token(); }
                if self.current_token == Token::Comma { self.next_token(); }
            }
            if self.current_token == Token::Gt { self.next_token(); }
        }
        params
    }

    fn parse_enum(&mut self) -> Option<Enum> {
        self.next_token();
        let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        let generic_params = self.parse_generic_params();
        if self.current_token != Token::LBrace { return None; }
        self.next_token();
        let mut variants = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if let Token::Identifier(v_name) = &self.current_token {
                let v_name = v_name.clone(); self.next_token();
                let mut data_types = Vec::new();
                if self.current_token == Token::LParen {
                    self.next_token();
                    while self.current_token != Token::RParen && self.current_token != Token::EOF {
                        data_types.push(self.parse_type_name());
                        if self.current_token == Token::Comma { self.next_token(); }
                    }
                    if self.current_token == Token::RParen { self.next_token(); }
                }
                variants.push(EnumVariant { name: v_name, data_types });
            }
            if self.current_token == Token::Comma { self.next_token(); }
        }
        if self.current_token == Token::RBrace { self.next_token(); }
        Some(Enum { name, generic_params, variants })
    }

    fn parse_interface(&mut self) -> Option<Interface> {
        self.next_token();
        let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        if self.current_token != Token::LBrace { return None; }
        self.next_token();
        let mut methods = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if let Some(f) = self.parse_function(vec![], vec![]) { methods.push(f); }
            if self.current_token == Token::Semicolon { self.next_token(); }
        }
        if self.current_token == Token::RBrace { self.next_token(); }
        Some(Interface { name, methods })
    }

    fn parse_struct(&mut self, attributes: Vec<(String, String)>) -> Option<Struct> {
        self.next_token();
        let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        let generic_params = self.parse_generic_params();
        if self.current_token != Token::LBrace { return None; }
        self.next_token();
        let mut fields = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if let Token::Identifier(f_name) = &self.current_token {
                let f_name = f_name.clone(); self.next_token();
                if self.current_token == Token::Colon {
                    self.next_token();
                    fields.push((f_name, self.parse_type_name()));
                }
            }
            if self.current_token == Token::Comma { self.next_token(); }
        }
        if self.current_token == Token::RBrace { self.next_token(); }
        let attrs = attributes.into_iter().map(|(k,_)| k).collect();
        Some(Struct { name, generic_params, fields, attributes: attrs })
    }

    fn parse_type_name(&mut self) -> String {
        if self.current_token == Token::Star {
            self.next_token();
            return format!("*{}", self.parse_type_name());
        }

        let mut full_type;
        if let Token::Identifier(id) = self.current_token.clone() {
            full_type = id;
            self.next_token();
        } else {
            let tok = self.current_token.clone();
            self.next_token();
            return format!("invalid_type_{:?}", tok);
        }
        
        while self.current_token == Token::Dot || self.current_token == Token::DoubleColon {
            self.next_token();
            if let Token::Identifier(sub) = &self.current_token { 
                full_type.push('.'); full_type.push_str(sub); 
                self.next_token(); 
            } else {
                break;
            }
        }

        if self.current_token == Token::Lt {
            full_type.push('<');
            self.next_token();
            while self.current_token != Token::Gt && self.current_token != Token::EOF {
                let sub_type = self.parse_type_name();
                full_type.push_str(&sub_type);
                
                if self.current_token == Token::Comma {
                    full_type.push_str(", ");
                    self.next_token();
                }
            }
            if self.current_token == Token::Gt {
                full_type.push('>');
                self.next_token();
            }
        }

        if self.current_token == Token::Question {
            self.next_token();
            full_type.push('?');
        }
        full_type
    }

    fn parse_function(&mut self, modifiers: Vec<Token>, attributes: Vec<(String, String)>) -> Option<Function> {
        self.next_token();
        let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        let generic_params = self.parse_generic_params();
        
        let mut params = Vec::new();
        if self.current_token == Token::LParen {
            self.next_token();
            while self.current_token != Token::RParen && self.current_token != Token::EOF {
                if self.current_token == Token::SelfToken {
                    params.push(("self".to_string(), "Self".to_string()));
                    self.next_token();
                } else if self.current_token == Token::Mut {
                    self.next_token();
                    if self.current_token == Token::SelfToken {
                        params.push(("self".to_string(), "Self".to_string()));
                        self.next_token();
                    } else if let Token::Identifier(p_name) = &self.current_token {
                        let p_name = p_name.clone();
                        self.next_token();
                        if self.current_token == Token::Colon {
                            self.next_token();
                            params.push((p_name, self.parse_type_name()));
                        }
                    }
                } else if let Token::Identifier(p_name) = &self.current_token {
                    let p_name = p_name.clone();
                    self.next_token();
                    if self.current_token == Token::Colon {
                        self.next_token();
                        params.push((p_name, self.parse_type_name()));
                    }
                } else {
                    self.next_token();
                }
                
                if self.current_token == Token::Comma { self.next_token(); }
            }
            if self.current_token == Token::RParen { self.next_token(); }
        } else {
            return None; 
        }

        let mut return_type = "void".to_string();
        if self.current_token == Token::Arrow {
            self.next_token();
            return_type = self.parse_type_name();
        }
        let mut body = None;
        if self.current_token == Token::LBrace { body = Some(self.parse_block()); }
        Some(Function { name, generic_params, params, return_type, body, modifiers, attributes })
    }

    fn parse_block(&mut self) -> Vec<Statement> {
        let mut stmts = Vec::new();
        if self.current_token == Token::LBrace { self.next_token(); }
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if self.current_token == Token::Semicolon {
                self.next_token();
                continue;
            }
            if let Some(s) = self.parse_statement() { 
                stmts.push(s); 
            } else {
                self.next_token();
            }
            if self.current_token == Token::Semicolon { self.next_token(); }
        }
        if self.current_token == Token::RBrace { self.next_token(); }
        stmts
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        match self.current_token {
            Token::DoubleColon => {
                if self.peek_at(0) == Token::Intent {
                    self.next_token(); // ::
                    self.next_token(); // intent
                    if let Token::StringLiteral(_) = self.current_token {
                        self.next_token();
                    }
                    Some(Statement::NoOp)
                } else {
                    Some(Statement::ExpressionStmt(self.parse_expression()))
                }
            },
            Token::Let => {
                self.next_token();
                let is_mut = if self.current_token == Token::Mut { self.next_token(); true } else { false };
                let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
                self.next_token();
                if self.current_token == Token::Eq { 
                    self.next_token(); 
                    let value = self.parse_expression(); 
                    if self.current_token == Token::Semicolon { self.next_token(); }
                    Some(Statement::Let { name, value, intent: None, is_mut }) 
                } else { None }
            },
            Token::Return => { 
                self.next_token(); 
                let value = if self.current_token == Token::Semicolon || self.current_token == Token::RBrace {
                    Expression::Integer(0) // Return 0/void
                } else {
                    self.parse_expression()
                };
                if self.current_token == Token::Semicolon { self.next_token(); }
                let stmt = Statement::Return { value, intent: None };
                Some(stmt)
            },
            Token::If => {
                self.next_token();
                let condition = self.parse_expression();
                let then_branch = self.parse_block();
                let mut else_branch = None;
                if self.current_token == Token::Else { 
                    self.next_token(); 
                    else_branch = Some(self.parse_block()); 
                }
                Some(Statement::If { condition, then_branch, else_branch })
            },
            Token::While => {
                self.next_token();
                let condition = self.parse_expression();
                let body = self.parse_block();
                Some(Statement::While { condition, body })
            },
            Token::Match => {
                self.next_token();
                let condition = self.parse_expression();
                if self.current_token != Token::LBrace { return None; }
                self.next_token();
                let mut arms = Vec::new();
                while self.current_token != Token::RBrace && self.current_token != Token::EOF {
                    let mut pattern = String::new();
                    if let Token::Identifier(p) = &self.current_token {
                        pattern = p.clone(); 
                        self.next_token();
                        while self.current_token == Token::DoubleColon || self.current_token == Token::Dot {
                            let op = if self.current_token == Token::DoubleColon { "::" } else { "." };
                            self.next_token();
                            if let Token::Identifier(sub) = &self.current_token {
                                pattern.push_str(op);
                                pattern.push_str(sub);
                                self.next_token();
                            }
                        }
                    }

                    if !pattern.is_empty() {
                        let mut params = Vec::new();
                        
                        if self.current_token == Token::LParen {
                            self.next_token();
                            while self.current_token != Token::RParen && self.current_token != Token::EOF {
                                if let Token::Identifier(param) = &self.current_token {
                                    params.push(param.clone());
                                    self.next_token();
                                }
                                if self.current_token == Token::Comma { self.next_token(); }
                            }
                            if self.current_token == Token::RParen { self.next_token(); }
                        }

                        if self.current_token == Token::Arrow {
                            self.next_token();
                            let body = if self.current_token == Token::LBrace { self.parse_block() } else { vec![self.parse_statement().unwrap()] };
                            arms.push(MatchArm { pattern, params, body });
                        }
                    } else if self.current_token != Token::Comma {
                        self.next_token();
                    }
                    if self.current_token == Token::Comma { self.next_token(); }
                }
                if self.current_token == Token::RBrace { self.next_token(); }
                Some(Statement::Match { condition, arms })
            },
            Token::Unsafe => {
                self.next_token();
                if self.current_token == Token::LBrace {
                    Some(Statement::UnsafeBlock(self.parse_block()))
                } else {
                    None
                }
            },
            _ => {
                let expr = self.parse_expression();
                if self.current_token == Token::Eq {
                    self.next_token();
                    let value = self.parse_expression();
                    if self.current_token == Token::Semicolon { self.next_token(); }
                    Some(Statement::Assignment { target: expr, value })
                } else {
                    if self.current_token == Token::Semicolon { self.next_token(); }
                    Some(Statement::ExpressionStmt(expr))
                }
            },
        }
    }

    fn parse_expression(&mut self) -> Expression {
        self.parse_infix(0)
    }

    fn parse_infix(&mut self, precedence: i32) -> Expression {
        let mut left = self.parse_primary();

        while self.current_token != Token::EOF && self.get_precedence() > precedence {
            let op = self.current_token.clone();
            self.next_token();
                
            if op == Token::As {
                let target = self.parse_type_name();
                left = Expression::Cast { expr: Box::new(left), target };
                continue;
            }

            if op == Token::Pipeline {
                let right = self.parse_infix(1);
                left = match right {
                    Expression::Call { function, mut arguments, generic_args } => {
                        arguments.insert(0, left);
                        Expression::Call { function, generic_args, arguments }
                    },
                    Expression::Identifier(function) => {
                        Expression::Call { function, generic_args: vec![], arguments: vec![left] }
                    },
                    _ => Expression::Infix { left: Box::new(left), operator: op, right: Box::new(right) }
                };
            } else {
                let right = self.parse_infix(self.get_precedence());
                left = Expression::Infix { left: Box::new(left), operator: op, right: Box::new(right) };
            }
        }
        left
    }

    fn get_precedence(&self) -> i32 {
        match self.current_token {
            Token::Star | Token::Slash | Token::Percent => 8,
            Token::As => 7,
            Token::Plus | Token::Minus => 6,
            Token::Caret => 5,
            Token::Gt | Token::Lt | Token::GtEq | Token::LtEq | Token::EqEq | Token::NotEq | Token::Inside => 4,
            Token::And => 3,
            Token::Or => 2,
            Token::Pipeline => 1,
            _ => 0,
        }
    }

    fn is_generic_args_ahead(&mut self) -> bool {
        if self.current_token != Token::Lt { return false; }
        let mut i = 0;
        let mut angle_count = 1;
        while angle_count > 0 {
            let tok = self.peek_at(i);
            match tok {
                Token::EOF | Token::LBrace | Token::Semicolon | Token::Eq => return false,
                Token::Lt => angle_count += 1,
                Token::Gt => angle_count -= 1,
                _ => {}
            }
            i += 1;
            if i > 50 { return false; } 
        }
        true
    }

    fn parse_generic_args(&mut self) -> Vec<String> {
        let mut args = Vec::new();
        if self.is_generic_args_ahead() {
            self.next_token();
            while self.current_token != Token::Gt && self.current_token != Token::EOF {
                if let Token::Identifier(id) = &self.current_token { 
                    args.push(id.clone()); 
                    self.next_token(); 
                } else {
                    self.next_token();
                }
                if self.current_token == Token::Comma { self.next_token(); }
            }
            if self.current_token == Token::Gt { self.next_token(); }
        }
        args
    }

    fn parse_primary(&mut self) -> Expression {
        let mut expr = match self.current_token.clone() {
            Token::If => {
                self.next_token();
                let condition = self.parse_expression();
                let then_branch = self.parse_block();
                let mut else_branch = None;
                if self.current_token == Token::Else { 
                    self.next_token(); 
                    else_branch = Some(self.parse_block()); 
                }
                Expression::If { condition: Box::new(condition), then_branch, else_branch }
            },
            Token::LBrace => {
                Expression::Block { statements: self.parse_block(), is_unsafe: false }
            },
            Token::SelfToken => {
                self.next_token();
                Expression::Identifier("self".to_string())
            },
            Token::Star => {
                self.next_token();
                Expression::Deref { expr: Box::new(self.parse_primary()) }
            },
            Token::Bang => {
                self.next_token();
                let inner = self.parse_primary();
                Expression::Infix { 
                    left: Box::new(inner),
                    operator: Token::EqEq, 
                    right: Box::new(Expression::Boolean(false)) 
                }
            },
            Token::LParen => {
                self.next_token();
                let e = self.parse_expression();
                if self.current_token == Token::RParen { self.next_token(); }
                e
            },
            Token::True => { self.next_token(); Expression::Boolean(true) },
            Token::False => { self.next_token(); Expression::Boolean(false) },
            Token::IntLiteral(n) => { self.next_token(); Expression::Integer(n) },
            Token::FloatLiteral(f) => { self.next_token(); Expression::Float(f) },
            Token::StringLiteral(s) => { self.next_token(); Expression::String(s) },
            Token::FString(s) => { self.next_token(); self.parse_fstring(s) },
            Token::DurationLiteral(s, n) => { self.next_token(); Expression::Duration(s, n) },
            Token::DateLiteral(ts) => { self.next_token(); Expression::Date(ts) },
            Token::At => {
                self.next_token();
                if let Token::Identifier(name) = self.current_token.clone() {
                    self.next_token();
                    if self.current_token == Token::LParen {
                        self.next_token();
                        let mut args = Vec::new();
                        while self.current_token != Token::RParen && self.current_token != Token::EOF {
                            args.push(self.parse_expression());
                            if self.current_token == Token::Comma { self.next_token(); }
                        }
                        if self.current_token == Token::RParen { self.next_token(); }
                        Expression::Intrinsic { name, arguments: args }
                    } else {
                        Expression::Identifier(format!("invalid_attribute_{}", name))
                    }
                } else {
                    Expression::Identifier("invalid_at_usage".to_string())
                }
            },
            Token::Unsafe => {
                self.next_token();
                if self.current_token == Token::LBrace {
                    Expression::Block { statements: self.parse_block(), is_unsafe: true }
                } else {
                    Expression::Identifier("invalid_unsafe_usage".to_string())
                }
            },
            Token::Identifier(n) => {
                self.next_token();
                let mut full_name = n;
                while self.current_token == Token::Dot {
                    if let Token::Identifier(sub) = self.peek_at(0) {
                        if self.peek_at(1) != Token::LParen {
                            self.next_token(); 
                            full_name.push('.');
                            full_name.push_str(&sub);
                            self.next_token(); 
                        } else { break; }
                    } else { break; }
                }
                let generic_args = self.parse_generic_args();
                if generic_args.is_empty() {
                    Expression::Identifier(full_name)
                } else {
                    Expression::TypeRef { name: full_name, generic_args }
                }
            },
            _ => { 
                let tok = self.current_token.clone();
                self.next_token(); 
                Expression::Identifier(format!("invalid_token_{:?}", tok)) 
            },
        };

        loop {
            match self.current_token {
                Token::Dot => {
                    self.next_token();
                    if let Token::Identifier(member) = self.current_token.clone() {
                        let member_name = member;
                        self.next_token();
                        let m_generic_args = self.parse_generic_args();
                        if self.current_token == Token::LParen {
                            self.next_token();
                            let mut args = Vec::new();
                            while self.current_token != Token::RParen && self.current_token != Token::EOF {
                                args.push(self.parse_expression());
                                if self.current_token == Token::Comma { self.next_token(); }
                            }
                            if self.current_token == Token::RParen { self.next_token(); }
                            
                            if let Expression::Identifier(ref name) = expr {
                                let func_name = format!("{}.{}", name, member_name);
                                expr = Expression::Call { 
                                    function: func_name, 
                                    generic_args: if m_generic_args.is_empty() { vec![] } else { m_generic_args }, 
                                    arguments: args 
                                };
                            } else if let Expression::TypeRef { ref name, ref generic_args } = expr {
                                let func_name = format!("{}.{}", name, member_name);
                                expr = Expression::Call { 
                                    function: func_name, 
                                    generic_args: generic_args.clone(), 
                                    arguments: args 
                                };
                            } else {
                                expr = Expression::MethodCall { 
                                    receiver: Box::new(expr.clone()), 
                                    method: member_name, 
                                    generic_args: m_generic_args, 
                                    arguments: args 
                                };
                            }
                        } else {
                            if let Expression::Identifier(ref name) = expr {
                                expr = Expression::Identifier(format!("{}.{}", name, member_name));
                            } else if let Expression::TypeRef { ref name, .. } = expr {
                                expr = Expression::Identifier(format!("{}.{}", name, member_name));
                            } else {
                                expr = Expression::MemberAccess { receiver: Box::new(expr.clone()), member: member_name };
                            }
                        }
                    } else { break; }
                },
                Token::DoubleColon => {
                    self.next_token();
                    if let Token::Identifier(variant) = self.current_token.clone() {
                        let variant_name = variant;
                        self.next_token();
                        let mut args = Vec::new();
                        if self.current_token == Token::LParen {
                            self.next_token();
                            while self.current_token != Token::RParen && self.current_token != Token::EOF {
                                args.push(self.parse_expression());
                                if self.current_token == Token::Comma { self.next_token(); }
                            }
                            if self.current_token == Token::RParen { self.next_token(); }
                        }
                        let name = match expr {
                            Expression::Identifier(ref n) => n.clone(),
                            Expression::TypeRef { ref name, .. } => name.clone(),
                            _ => "unknown_enum".to_string(),
                        };
                        expr = Expression::EnumInst { name, variant: variant_name, generic_args: vec![], arguments: args };
                    } else { break; }
                },
                Token::LParen => {
                    self.next_token();
                    let mut args = Vec::new();
                    while self.current_token != Token::RParen && self.current_token != Token::EOF {
                        args.push(self.parse_expression());
                        if self.current_token == Token::Comma { self.next_token(); }
                    }
                    if self.current_token == Token::RParen { self.next_token(); }
                    
                    if let Expression::Identifier(ref name) = expr {
                        expr = Expression::Call { function: name.clone(), generic_args: vec![], arguments: args };
                    } else if let Expression::TypeRef { ref name, ref generic_args } = expr {
                        expr = Expression::Call { function: name.clone(), generic_args: generic_args.clone(), arguments: args };
                    } else { break; }
                },
                Token::LBrace => {
                    let next_tok = self.peek_at(0);
                    let is_struct = if next_tok == Token::RBrace { true } 
                        else if let Token::Identifier(_) = next_tok {
                            let next_next = self.peek_at(1);
                            matches!(next_next, Token::Colon | Token::Eq)
                        } else { false };
                    
                    if is_struct {
                        self.next_token();
                        let mut fields = Vec::new();
                        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
                            if let Token::Identifier(f_name) = self.current_token.clone() {
                                self.next_token();
                                if self.current_token == Token::Colon || self.current_token == Token::Eq {
                                    self.next_token();
                                    fields.push((f_name, self.parse_expression()));
                                }
                            }
                            if self.current_token == Token::Comma { self.next_token(); }
                        }
                        if self.current_token == Token::RBrace { self.next_token(); }
                        let (name, generic_args) = match expr {
                            Expression::Identifier(ref n) => (n.clone(), vec![]),
                            Expression::TypeRef { ref name, ref generic_args } => (name.clone(), generic_args.clone()),
                            _ => ("unknown_struct".to_string(), vec![]),
                        };
                        expr = Expression::StructInst { name, generic_args, fields };
                    } else { break; }
                },
                Token::Lt => {
                    if let Expression::Identifier(ref name) = expr {
                        let n = name.clone();
                        let generic_args = self.parse_generic_args();
                        if !generic_args.is_empty() {
                            expr = Expression::TypeRef { name: n, generic_args };
                        } else { break; }
                    } else { break; }
                },
                _ => break,
            }
        }
        expr
    }

    fn parse_fstring(&mut self, s: String) -> Expression {
        Expression::String(s)
    }
}

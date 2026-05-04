use crate::token::{Token, TokenKind};
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

    fn error(&self, message: &str) -> String {
        format!("{} at line {}, col {}", message, self.current_token.line, self.current_token.col)
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

        while self.current_token.kind != TokenKind::EOF {
            match self.current_token.kind {
                TokenKind::Identifier(ref id) if id == "module" => {
                    self.next_token();
                    let path = self.parse_path();
                    module_name = Some(path.join("."));
                    if self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
                },
                TokenKind::Use => imports.push(self.parse_import()),
                TokenKind::DoubleColon => {
                    if self.peek_at(0).kind == TokenKind::Intent {
                        self.next_token(); // consume ::
                        self.next_token(); // consume intent
                        if let TokenKind::StringLiteral(_) = self.current_token.kind {
                            self.next_token();
                        }
                    } else {
                        // If it's not an intent, we shouldn't consume it here.
                        // But wait, what else starts with :: at top level? Nothing yet.
                        // For safety, let's just break if we don't know what it is or error.
                        eprintln!("{}", self.error("Syntax Error: Unexpected :: at top level"));
                        self.next_token();
                    }
                },
                _ => {
                    if let Some(decl) = self.parse_declaration() {
                        declarations.push(decl);
                    } else {
                        eprintln!("{}", self.error("Syntax Error: Unexpected token in declaration"));
                        self.next_token();
                    }
                },
            }
        }
        Program { module_name, imports, declarations }
    }

    fn parse_path(&mut self) -> Vec<String> {
        let mut path = Vec::new();
        if let TokenKind::Identifier(id) = &self.current_token.kind {
            path.push(id.clone());
            self.next_token();
            while self.current_token.kind == TokenKind::Dot {
                self.next_token();
                if let TokenKind::Identifier(sub) = &self.current_token.kind { 
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
        if self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
        Import { path, alias: None }
    }

    fn parse_declaration(&mut self) -> Option<Declaration> {
        let mut attributes = Vec::new();
        while self.current_token.kind == TokenKind::At {
            self.next_token();
            if let TokenKind::Identifier(name) = &self.current_token.kind {
                let attr_name = name.clone();
                self.next_token();
                let mut attr_val = String::new();
                if self.current_token.kind == TokenKind::LParen {
                    self.next_token();
                    if let TokenKind::StringLiteral(s) = &self.current_token.kind { 
                        attr_val = s.clone(); 
                        self.next_token(); 
                    }
                    if self.current_token.kind == TokenKind::RParen { self.next_token(); }
                }
                attributes.push((attr_name, attr_val));
            } else {
                break;
            }
        }
        let mut modifiers = Vec::new();
        while matches!(self.current_token.kind, TokenKind::Pub | TokenKind::Async | TokenKind::Unsafe | TokenKind::Extern) {
            modifiers.push(self.current_token.clone());
            self.next_token();
        }
        match self.current_token.kind {
            TokenKind::Fn => self.parse_function(modifiers, attributes).map(Declaration::Function),
            TokenKind::Struct => self.parse_struct(attributes).map(Declaration::Struct),
            TokenKind::Enum => self.parse_enum().map(Declaration::Enum),
            TokenKind::Interface => self.parse_interface().map(Declaration::Interface),
            TokenKind::Impl => self.parse_impl().map(Declaration::Impl),
            _ => None,
        }
    }

    fn parse_impl(&mut self) -> Option<ImplBlock> {
        self.next_token();
        let name1 = match &self.current_token.kind { TokenKind::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        let generic_params = self.parse_generic_params();
        
        let (interface_name, target_name) = if self.current_token.kind == TokenKind::For {
                self.next_token();
                let target = match &self.current_token.kind { TokenKind::Identifier(n) => n.clone(), _ => return None };
                self.next_token();
                (Some(name1), target)
        } else { (None, name1) };

        if self.current_token.kind != TokenKind::LBrace { 
            eprintln!("{}", self.error("Syntax Error: Expected '{' after impl target"));
            return None; 
        }
        self.next_token();
        let mut functions = Vec::new();
        while self.current_token.kind != TokenKind::RBrace && self.current_token.kind != TokenKind::EOF {
            let mut modifiers = Vec::new();
            while matches!(self.current_token.kind, TokenKind::Pub | TokenKind::Async | TokenKind::Unsafe) {
                modifiers.push(self.current_token.clone());
                self.next_token();
            }
            if self.current_token.kind == TokenKind::Fn {
                if let Some(f) = self.parse_function(modifiers, vec![]) { functions.push(f); }
            } else {
                self.next_token();
            }
        }
        if self.current_token.kind == TokenKind::RBrace { self.next_token(); }
        Some(ImplBlock { target_name, generic_params, interface_name, functions })
    }

    fn parse_generic_params(&mut self) -> Vec<String> {
        let mut params = Vec::new();
        if self.current_token.kind == TokenKind::Lt {
            self.next_token();
            while self.current_token.kind != TokenKind::Gt && self.current_token.kind != TokenKind::EOF {
                if let TokenKind::Identifier(id) = &self.current_token.kind { params.push(id.clone()); self.next_token(); }
                if self.current_token.kind == TokenKind::Comma { self.next_token(); }
            }
            if self.current_token.kind == TokenKind::Gt { self.next_token(); }
        }
        params
    }

    fn parse_enum(&mut self) -> Option<Enum> {
        self.next_token();
        let name = match &self.current_token.kind { TokenKind::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        let generic_params = self.parse_generic_params();
        if self.current_token.kind != TokenKind::LBrace { return None; }
        self.next_token();
        let mut variants = Vec::new();
        while self.current_token.kind != TokenKind::RBrace && self.current_token.kind != TokenKind::EOF {
            if let TokenKind::Identifier(v_name) = &self.current_token.kind {
                let v_name = v_name.clone(); self.next_token();
                let mut data_types = Vec::new();
                if self.current_token.kind == TokenKind::LParen {
                    self.next_token();
                    while self.current_token.kind != TokenKind::RParen && self.current_token.kind != TokenKind::EOF {
                        data_types.push(self.parse_type_name());
                        if self.current_token.kind == TokenKind::Comma { self.next_token(); }
                    }
                    if self.current_token.kind == TokenKind::RParen { self.next_token(); }
                }
                variants.push(EnumVariant { name: v_name, data_types });
            } else if self.current_token.kind != TokenKind::Comma {
                self.next_token();
            }
            if self.current_token.kind == TokenKind::Comma { self.next_token(); }
        }
        if self.current_token.kind == TokenKind::RBrace { self.next_token(); }
        Some(Enum { name, generic_params, variants })
    }

    fn parse_interface(&mut self) -> Option<Interface> {
        self.next_token();
        let name = match &self.current_token.kind { TokenKind::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        if self.current_token.kind != TokenKind::LBrace { return None; }
        self.next_token();
        let mut methods = Vec::new();
        while self.current_token.kind != TokenKind::RBrace && self.current_token.kind != TokenKind::EOF {
            if let Some(f) = self.parse_function(vec![], vec![]) { methods.push(f); }
            if self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
        }
        if self.current_token.kind == TokenKind::RBrace { self.next_token(); }
        Some(Interface { name, methods })
    }

    fn parse_struct(&mut self, attributes: Vec<(String, String)>) -> Option<Struct> {
        self.next_token();
        let name = match &self.current_token.kind { TokenKind::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        let generic_params = self.parse_generic_params();
        if self.current_token.kind != TokenKind::LBrace { return None; }
        self.next_token();
        let mut fields = Vec::new();
        while self.current_token.kind != TokenKind::RBrace && self.current_token.kind != TokenKind::EOF {
            if let TokenKind::Identifier(f_name) = &self.current_token.kind {
                let f_name = f_name.clone(); self.next_token();
                if self.current_token.kind == TokenKind::Colon {
                    self.next_token();
                    fields.push((f_name, self.parse_type_name()));
                }
            }
            if self.current_token.kind == TokenKind::Comma { self.next_token(); }
        }
        if self.current_token.kind == TokenKind::RBrace { self.next_token(); }
        let attrs = attributes.into_iter().map(|(k,_)| k).collect();
        Some(Struct { name, generic_params, fields, attributes: attrs })
    }

    fn parse_type_name(&mut self) -> String {
        if self.current_token.kind == TokenKind::Star {
            self.next_token();
            return format!("*{}", self.parse_type_name());
        }

        let mut full_type;
        if let TokenKind::Identifier(id) = self.current_token.clone().kind {
            full_type = id;
            self.next_token();
        } else {
            let tok = self.current_token.clone();
            self.next_token();
            return format!("invalid_type_{:?}", tok);
        }
        
        while self.current_token.kind == TokenKind::Dot || self.current_token.kind == TokenKind::DoubleColon {
            if let TokenKind::Identifier(_) = self.peek_at(0).kind {
                self.next_token();
                if let TokenKind::Identifier(sub) = &self.current_token.kind { 
                    full_type.push('.'); full_type.push_str(sub); 
                    self.next_token(); 
                }
            } else {
                break;
            }
        }

        if self.current_token.kind == TokenKind::Lt {
            full_type.push('<');
            self.next_token();
            while self.current_token.kind != TokenKind::Gt && self.current_token.kind != TokenKind::EOF {
                let sub_type = self.parse_type_name();
                full_type.push_str(&sub_type);
                
                if self.current_token.kind == TokenKind::Comma {
                    full_type.push_str(", ");
                    self.next_token();
                }
            }
            if self.current_token.kind == TokenKind::Gt {
                full_type.push('>');
                self.next_token();
            }
        }

        if self.current_token.kind == TokenKind::Question {
            self.next_token();
            full_type.push('?');
        }
        full_type
    }

    fn parse_function(&mut self, modifiers: Vec<Token>, attributes: Vec<(String, String)>) -> Option<Function> {
        self.next_token();
        let name = match &self.current_token.kind { TokenKind::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        let generic_params = self.parse_generic_params();
        
        let mut params = Vec::new();
        if self.current_token.kind == TokenKind::LParen {
            self.next_token();
            while self.current_token.kind != TokenKind::RParen && self.current_token.kind != TokenKind::EOF {
                if self.current_token.kind == TokenKind::SelfToken {
                    params.push(("self".to_string(), "Self".to_string(), None));
                    self.next_token();
                } else if self.current_token.kind == TokenKind::Mut {
                    self.next_token();
                    if self.current_token.kind == TokenKind::SelfToken {
                        params.push(("self".to_string(), "Self".to_string(), None));
                        self.next_token();
                    } else if let TokenKind::Identifier(p_name) = &self.current_token.kind {
                        let p_name = p_name.clone();
                        self.next_token();
                        if self.current_token.kind == TokenKind::Colon {
                            self.next_token();
                            params.push((p_name, self.parse_type_name(), None));
                        }
                    }
                } else if let TokenKind::Identifier(p_name) = &self.current_token.kind {
                    let p_name = p_name.clone();
                    self.next_token();
                    let mut default_val: Option<Box<Expression>> = None;
                    if self.current_token.kind == TokenKind::Colon {
                        self.next_token();
                        let param_type = self.parse_type_name();
                        // Check for default value
                        if self.current_token.kind == TokenKind::Eq {
                            self.next_token();
                            default_val = Some(Box::new(self.parse_expression()));
                        }
                        params.push((p_name, param_type, default_val));
                    } else {
                        params.push((p_name, "i64".to_string(), default_val));  // Default type i64
                    }
                } else {
                    self.next_token();
                }
                
                if self.current_token.kind == TokenKind::Comma { self.next_token(); }
            }
            if self.current_token.kind == TokenKind::RParen { self.next_token(); }
        } else {
            return None; 
        }

        let mut return_type = "void".to_string();
        if self.current_token.kind == TokenKind::Arrow {
            self.next_token();
            return_type = self.parse_type_name();
        }
        let mut body = None;
        if self.current_token.kind == TokenKind::LBrace { 
            body = Some(self.parse_block()); 
        } else {
            let is_extern = modifiers.iter().any(|m| m.kind == TokenKind::Extern);
            let is_intrinsic = attributes.iter().any(|(k, _)| k == "intrinsic");
            if (is_extern || is_intrinsic)
                && self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
        }
        Some(Function { name, generic_params, params, return_type, body, modifiers, attributes })
    }

    fn parse_block(&mut self) -> Vec<Statement> {
        let mut stmts = Vec::new();
        if self.current_token.kind == TokenKind::LBrace { self.next_token(); }
        while self.current_token.kind != TokenKind::RBrace && self.current_token.kind != TokenKind::EOF {
            if self.current_token.kind == TokenKind::Semicolon {
                self.next_token();
                continue;
            }
            if let Some(s) = self.parse_statement() { 
                stmts.push(s); 
            } else {
                self.next_token();
            }
            if self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
        }
        if self.current_token.kind == TokenKind::RBrace { self.next_token(); }
        stmts
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        match self.current_token.kind {
            TokenKind::DoubleColon => {
                if self.peek_at(0).kind == TokenKind::Intent {
                    self.next_token(); // ::
                    self.next_token(); // intent
                    if let TokenKind::StringLiteral(_) = self.current_token.kind {
                        self.next_token();
                    }
                    Some(Statement::NoOp)
                } else {
                    Some(Statement::ExpressionStmt(self.parse_expression()))
                }
            },
            TokenKind::Let => {
                self.next_token();
                let is_mut = if self.current_token.kind == TokenKind::Mut { self.next_token(); true } else { false };
                let name = match &self.current_token.kind { TokenKind::Identifier(n) => n.clone(), _ => return None };
                self.next_token();
                if self.current_token.kind == TokenKind::Eq { 
                    self.next_token(); 
                    let value = self.parse_expression(); 
                    if self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
                    Some(Statement::Let { name, value, intent: None, is_mut }) 
                } else { None }
            },
            TokenKind::Return => { 
                self.next_token(); 
                let value = if self.current_token.kind == TokenKind::Semicolon || self.current_token.kind == TokenKind::RBrace {
                    Expression::Integer(0) // Return 0/void
                } else {
                    self.parse_expression()
                };
                if self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
                let stmt = Statement::Return { value, intent: None };
                Some(stmt)
            },
            TokenKind::If => {
                self.next_token();
                let condition = self.parse_expression();
                let then_branch = self.parse_block();
                let mut else_branch = None;
                if self.current_token.kind == TokenKind::Else {
                    self.next_token();
                    if self.current_token.kind == TokenKind::If {
                        if let Some(if_stmt) = self.parse_statement() {
                            else_branch = Some(vec![if_stmt]);
                        }
                    } else {
                        else_branch = Some(self.parse_block());
                    }
                }                Some(Statement::If { condition, then_branch, else_branch })
            },
            TokenKind::While => {
                self.next_token();
                let condition = self.parse_expression();
                let body = self.parse_block();
                Some(Statement::While { condition, body })
            },
            TokenKind::Match => {
                self.next_token();
                let condition = self.parse_expression();
                if self.current_token.kind != TokenKind::LBrace { return None; }
                self.next_token();
                let mut arms = Vec::new();
                while self.current_token.kind != TokenKind::RBrace && self.current_token.kind != TokenKind::EOF {
                    let mut pattern = String::new();
                    let mut is_valid_pattern = false;
                    
                    match &self.current_token.kind {
                        TokenKind::Identifier(p) => {
                            pattern = p.clone(); 
                            self.next_token();
                            // Check for struct pattern: Point { x: a, y: b }
                            let mut struct_fields: Vec<(String, String)> = Vec::new();
                            if self.current_token.kind == TokenKind::LBrace {
                                self.next_token();
                                while self.current_token.kind != TokenKind::RBrace && self.current_token.kind != TokenKind::EOF {
                                    if let TokenKind::Identifier(field_name) = &self.current_token.kind {
                                        let fn_str = field_name.clone();
                                        self.next_token();
                                        if self.current_token.kind == TokenKind::Colon {
                                            self.next_token();
                                            if let TokenKind::Identifier(var_name) = &self.current_token.kind {
                                                struct_fields.push((fn_str, var_name.clone()));
                                                self.next_token();
                                            }
                                        }
                                    }
                                    if self.current_token.kind == TokenKind::Comma {
                                        self.next_token();
                                    }
                                }
                                if self.current_token.kind == TokenKind::RBrace {
                                    self.next_token();
                                }
                            }
                            if !struct_fields.is_empty() {
                                let fields_str = struct_fields.iter()
                                    .map(|(f, v)| format!("{}:{}", f, v))
                                    .collect::<Vec<_>>()
                                    .join(",");
                                pattern = format!("{}_{{{}}}", pattern, fields_str);
                            }
                            
                            while self.current_token.kind == TokenKind::DoubleColon || self.current_token.kind == TokenKind::Dot {
                                let op = if self.current_token.kind == TokenKind::DoubleColon { "::" } else { "." };
                                self.next_token();
                                if let TokenKind::Identifier(sub) = &self.current_token.kind {
                                    pattern.push_str(op);
                                    pattern.push_str(sub);
                                    self.next_token();
                                } else { break; }
                            }
                            is_valid_pattern = true;
                        },
                        TokenKind::IntLiteral(n) => {
                            pattern = n.to_string();
                            self.next_token();
                            // Check if this is a range pattern (e.g., 1..5)
                            if self.current_token.kind == TokenKind::Range {
                                self.next_token();
                                if let TokenKind::IntLiteral(end_n) = &self.current_token.kind {
                                    pattern = format!("{}..{}", pattern, end_n);
                                    self.next_token();
                                }
                            }
                            is_valid_pattern = true;
                        },
                        TokenKind::StringLiteral(s) => {
                            pattern = format!("\"{}\"", s);
                            self.next_token();
                            is_valid_pattern = true;
                        },
                        TokenKind::Range => {
                            // This shouldn't happen here - ranges are handled in expression parsing
                            self.next_token();
                        },
                        _ => {
                            // Skip unexpected token to avoid infinite loop
                            self.next_token();
                        }
                    }

                    if is_valid_pattern {
                        // Support multiple patterns with |
                        let mut patterns = vec![pattern.clone()];
                        while self.current_token.kind == TokenKind::Pipe {
                            self.next_token();
                            match &self.current_token.kind {
                                TokenKind::Identifier(p) => { patterns.push(p.clone()); self.next_token(); },
                                TokenKind::IntLiteral(n) => { patterns.push(n.to_string()); self.next_token(); },
                                TokenKind::StringLiteral(s) => { patterns.push(format!("\"{}\"", s)); self.next_token(); },
                                _ => { break; }
                            }
                        }
                        
                        let mut params = Vec::new();
                        
                        // If pattern is a lowercase identifier and followed by guard or arrow,
                        // treat it as a binding variable (not a pattern to match)
                        if patterns.len() == 1 {
                            let pat = &patterns[0];
                            let is_lower = !pat.is_empty() && pat.chars().next().unwrap().is_lowercase();
                            let is_identifier = pat.chars().all(|c| c.is_alphanumeric() || c == '_');
                            let is_not_underscore = pat != "_";
                            let followed_by_guard_or_arrow = self.current_token.kind == TokenKind::If || self.current_token.kind == TokenKind::Arrow;
                            
                            if is_lower && is_identifier && is_not_underscore && followed_by_guard_or_arrow {
                                params.push(pat.clone());
                                patterns.clear();
                            }
                        }
                        
                        if self.current_token.kind == TokenKind::LParen {
                            self.next_token();
                            while self.current_token.kind != TokenKind::RParen && self.current_token.kind != TokenKind::EOF {
                                if let TokenKind::Identifier(param) = &self.current_token.kind {
                                    params.push(param.clone());
                                    self.next_token();
                                } else {
                                    self.next_token();
                                }
                                if self.current_token.kind == TokenKind::Comma { self.next_token(); }
                            }
                            if self.current_token.kind == TokenKind::RParen { self.next_token(); }
                        }

                        // Parse guard (if condition)
                        let guard = if self.current_token.kind == TokenKind::If {
                            self.next_token();
                            Some(Box::new(self.parse_expression()))
                        } else {
                            None
                        };

                        if self.current_token.kind == TokenKind::Arrow {
                            self.next_token();
                            let body = if self.current_token.kind == TokenKind::LBrace { self.parse_block() } else {
                                match self.parse_statement() {
                                    Some(s) => vec![s],
                                    None => vec![],
                                }
                            };
                            arms.push(MatchArm { pattern, patterns, guard, params, body });
                        }
                    }
                    if self.current_token.kind == TokenKind::Comma { self.next_token(); }
                }
                if self.current_token.kind == TokenKind::RBrace { self.next_token(); }
                Some(Statement::Match { condition, arms })
            },
            TokenKind::Unsafe => {
                self.next_token();
                if self.current_token.kind == TokenKind::LBrace {
                    Some(Statement::UnsafeBlock(self.parse_block()))
                } else {
                    None
                }
            },
            TokenKind::Spawn => {
                self.next_token();
                if self.current_token.kind == TokenKind::LBrace {
                    Some(Statement::Spawn(self.parse_block()))
                } else {
                    None
                }
            },
            _ => {
                let expr = self.parse_expression();
                if self.current_token.kind == TokenKind::Eq {
                    self.next_token();
                    let value = self.parse_expression();
                    if self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
                    Some(Statement::Assignment { target: expr, value })
                } else {
                    if self.current_token.kind == TokenKind::Semicolon { self.next_token(); }
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

        while self.current_token.kind != TokenKind::EOF && self.get_precedence() > precedence {
            let op = self.current_token.clone();
            let op_prec = self.get_precedence();
            self.next_token();
                
            if op.kind == TokenKind::As {
                let target = self.parse_type_name();
                left = Expression::Cast { expr: Box::new(left), target };
                continue;
            }

            if op.kind == TokenKind::Pipeline {
                let right = self.parse_infix(1);
                left = match right {
                    Expression::Call { function, mut arguments, generic_args, .. } => {
                        arguments.insert(0, left);
                        Expression::Call { function, generic_args, arguments, line: op.line, col: op.col }
                    },
                    Expression::Identifier(function) => {
                        Expression::Call { function, generic_args: vec![], arguments: vec![left], line: op.line, col: op.col }
                    },
                    _ => Expression::Infix { left: Box::new(left), operator: op, right: Box::new(right) }
                };
            } else {
                let right = self.parse_infix(op_prec);
                left = Expression::Infix { left: Box::new(left), operator: op, right: Box::new(right) };
            }
        }
        left
    }

    fn get_precedence(&self) -> i32 {
        match self.current_token.kind {
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => 8,
            TokenKind::As => 7,
            TokenKind::Plus | TokenKind::Minus => 6,
            TokenKind::Range => 5,
            TokenKind::Caret => 5,
            TokenKind::Gt | TokenKind::Lt | TokenKind::GtEq | TokenKind::LtEq | TokenKind::EqEq | TokenKind::NotEq | TokenKind::Inside => 4,
            TokenKind::And => 3,
            TokenKind::Or => 2,
            TokenKind::Pipeline => 1,
            _ => 0,
        }
    }

        fn is_generic_args_ahead(&mut self) -> bool {
            if self.current_token.kind != TokenKind::Lt { return false; }
            let mut i = 0;
            let mut angle_count = 1;
            
            while angle_count > 0 {
                let tok = self.peek_at(i);
                match tok.kind {
                    TokenKind::EOF | TokenKind::LBrace | TokenKind::Semicolon | TokenKind::Eq | 
                    TokenKind::Or | TokenKind::And | TokenKind::Plus | TokenKind::Minus | 
                    TokenKind::RParen | TokenKind::Comma => return false,
                    TokenKind::Lt => angle_count += 1,
                    TokenKind::Gt => angle_count -= 1,
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
            while self.current_token.kind != TokenKind::Gt && self.current_token.kind != TokenKind::EOF {
                if let TokenKind::Identifier(id) = &self.current_token.kind { 
                    args.push(id.clone()); 
                    self.next_token(); 
                } else {
                    self.next_token();
                }
                if self.current_token.kind == TokenKind::Comma { self.next_token(); }
            }
            if self.current_token.kind == TokenKind::Gt { self.next_token(); }
        }
        args
    }

    fn parse_primary(&mut self) -> Expression {
        let mut expr = match self.current_token.clone().kind {
            TokenKind::If => {
                self.next_token();
                let condition = self.parse_expression();
                let then_branch = self.parse_block();
                let mut else_branch = None;
                if self.current_token.kind == TokenKind::Else {
                    self.next_token();
                    if self.current_token.kind == TokenKind::If {
                        if let Some(if_stmt) = self.parse_statement() {
                            else_branch = Some(vec![if_stmt]);
                        }
                    } else {
                        else_branch = Some(self.parse_block());
                    }
                }                Expression::If { condition: Box::new(condition), then_branch, else_branch }
            },
            TokenKind::LBrace => {
                Expression::Block { statements: self.parse_block(), is_unsafe: false }
            },
            TokenKind::SelfToken => {
                self.next_token();
                Expression::Identifier("self".to_string())
            },
            TokenKind::Star => {
                self.next_token();
                Expression::Deref { expr: Box::new(self.parse_primary()) }
            },
            TokenKind::Bang => {
                self.next_token();
                let inner = self.parse_primary();
                Expression::Infix { 
                    left: Box::new(inner),
                    operator: Token::new(TokenKind::EqEq, self.current_token.line, self.current_token.col),
                    right: Box::new(Expression::Boolean(false)) 
                }
            },
            TokenKind::LParen => {
                self.next_token();
                let e = self.parse_expression();
                if self.current_token.kind == TokenKind::RParen { self.next_token(); }
                e
            },
            TokenKind::True => { self.next_token(); Expression::Boolean(true) },
            TokenKind::False => { self.next_token(); Expression::Boolean(false) },
            TokenKind::Minus => {
                self.next_token();
                if let TokenKind::IntLiteral(n) = self.current_token.clone().kind {
                    self.next_token();
                    Expression::Integer(-n)
                } else if let TokenKind::FloatLiteral(f) = self.current_token.clone().kind {
                    self.next_token();
                    Expression::Float(-f)
                } else {
                    Expression::Infix {
                        left: Box::new(Expression::Integer(0)),
                        operator: Token::new(TokenKind::Minus, self.current_token.line, self.current_token.col),
                        right: Box::new(self.parse_primary())
                    }
                }
            },
            TokenKind::IntLiteral(n) => { self.next_token(); Expression::Integer(n) },
            TokenKind::FloatLiteral(f) => { self.next_token(); Expression::Float(f) },
            TokenKind::StringLiteral(s) => { self.next_token(); Expression::String(s) },
            TokenKind::FString(s) => { self.next_token(); self.parse_fstring(s) },
            TokenKind::DurationLiteral(s, n) => { self.next_token(); Expression::Duration(s, n) },
            TokenKind::DateLiteral(ts) => { self.next_token(); Expression::Date(ts) },
            TokenKind::At => {
                self.next_token();
                if let TokenKind::Identifier(name) = self.current_token.clone().kind {
                    self.next_token();
                    if self.current_token.kind == TokenKind::LParen {
                        self.next_token();
                        let mut args = Vec::new();
                        while self.current_token.kind != TokenKind::RParen && self.current_token.kind != TokenKind::EOF {
                            args.push(self.parse_expression());
                            if self.current_token.kind == TokenKind::Comma { self.next_token(); }
                        }
                        if self.current_token.kind == TokenKind::RParen { self.next_token(); }
                        Expression::Intrinsic { name, arguments: args }
                    } else {
                        Expression::Identifier(format!("invalid_attribute_{}", name))
                    }
                } else {
                    Expression::Identifier("invalid_at_usage".to_string())
                }
            },
            TokenKind::Unsafe => {
                self.next_token();
                if self.current_token.kind == TokenKind::LBrace {
                    Expression::Block { statements: self.parse_block(), is_unsafe: true }
                } else {
                    Expression::Identifier("invalid_unsafe_usage".to_string())
                }
            },
            TokenKind::Identifier(n) => {
                self.next_token();
                let mut full_name = n;
                while self.current_token.kind == TokenKind::Dot {
                    if let TokenKind::Identifier(sub) = self.peek_at(0).kind {
                        self.next_token(); 
                        full_name.push('.');
                        full_name.push_str(&sub);
                        self.next_token(); 
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
            match self.current_token.kind {
                TokenKind::Dot => {
                    let dot_line = self.current_token.line;
                    let dot_col = self.current_token.col;
                    self.next_token();
                    if let TokenKind::Identifier(member) = self.current_token.clone().kind {
                        let member_name = member;
                        self.next_token();
                        let m_generic_args = self.parse_generic_args();
                        if self.current_token.kind == TokenKind::LParen {
                            self.next_token();
                            let mut args = Vec::new();
                            while self.current_token.kind != TokenKind::RParen && self.current_token.kind != TokenKind::EOF {
                                args.push(self.parse_expression());
                                if self.current_token.kind == TokenKind::Comma { self.next_token(); }
                            }
                            if self.current_token.kind == TokenKind::RParen { self.next_token(); }
                            
                            expr = Expression::MethodCall { 
                                receiver: Box::new(expr.clone()), 
                                method: member_name, 
                                generic_args: m_generic_args, 
                                arguments: args,
                                line: dot_line,
                                col: dot_col,
                            };
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
                TokenKind::DoubleColon => {
                    if self.peek_at(0).kind == TokenKind::Intent { break; }
                    self.next_token();
                    if let TokenKind::Identifier(variant) = self.current_token.clone().kind {
                        let variant_name = variant;
                        self.next_token();
                        
                        let mut m_generic_args = Vec::new();
                        if self.current_token.kind == TokenKind::Lt {
                            m_generic_args = self.parse_generic_args();
                        }

                        let mut args = Vec::new();
                        if self.current_token.kind == TokenKind::LParen {
                            self.next_token();
                            while self.current_token.kind != TokenKind::RParen && self.current_token.kind != TokenKind::EOF {
                                args.push(self.parse_expression());
                                if self.current_token.kind == TokenKind::Comma { self.next_token(); }
                            }
                            if self.current_token.kind == TokenKind::RParen { self.next_token(); }
                        }
                        let (name, mut combined_generic_args) = match expr {
                            Expression::Identifier(ref n) => (n.clone(), vec![]),
                            Expression::TypeRef { ref name, ref generic_args } => (name.clone(), generic_args.clone()),
                            Expression::EnumInst { ref name, ref variant, ref generic_args, .. } => {
                                (format!("{}.{}", name, variant), generic_args.clone())
                            }
                            _ => ("unknown_enum".to_string(), vec![]),
                        };
                        combined_generic_args.extend(m_generic_args);
                        expr = Expression::EnumInst { name, variant: variant_name, generic_args: combined_generic_args, arguments: args };
                    } else { break; }
                },
                TokenKind::LParen => {
                    let call_line = self.current_token.line;
                    let call_col = self.current_token.col;
                    self.next_token();
                    let mut args = Vec::new();
                    while self.current_token.kind != TokenKind::RParen && self.current_token.kind != TokenKind::EOF {
                        args.push(self.parse_expression());
                        if self.current_token.kind == TokenKind::Comma { self.next_token(); }
                    }
                    if self.current_token.kind == TokenKind::RParen { self.next_token(); }
                    
                    if let Expression::Identifier(ref name) = expr {
                        expr = Expression::Call { function: name.clone(), generic_args: vec![], arguments: args, line: call_line, col: call_col };
                    } else if let Expression::TypeRef { ref name, ref generic_args } = expr {
                        expr = Expression::Call { function: name.clone(), generic_args: generic_args.clone(), arguments: args, line: call_line, col: call_col };
                    } else { break; }
                },
                TokenKind::LBrace => {
                    let is_type_like = matches!(expr, Expression::Identifier(_) | Expression::TypeRef { .. });
                    if !is_type_like { break; }

                    let next_tok = self.peek_at(0);
                    let is_struct = if next_tok.kind == TokenKind::RBrace { true } 
                        else if let TokenKind::Identifier(_) = next_tok.kind {
                            let next_next = self.peek_at(1);
                            next_next.kind == TokenKind::Colon
                        } else { false };
                    
                    if is_struct {
                        self.next_token();
                        let mut fields = Vec::new();
                        while self.current_token.kind != TokenKind::RBrace && self.current_token.kind != TokenKind::EOF {
                            if let TokenKind::Identifier(f_name) = self.current_token.clone().kind {
                                self.next_token();
                                if self.current_token.kind == TokenKind::Colon {
                                    self.next_token();
                                    fields.push((f_name, self.parse_expression()));
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                            if self.current_token.kind == TokenKind::Comma { 
                                self.next_token(); 
                            } else if self.current_token.kind != TokenKind::RBrace {
                                break;
                            }
                        }
                        if self.current_token.kind == TokenKind::RBrace { self.next_token(); }
                        let (name, generic_args) = match expr {
                            Expression::Identifier(ref n) => (n.clone(), vec![]),
                            Expression::TypeRef { ref name, ref generic_args } => (name.clone(), generic_args.clone()),
                            _ => ("unknown_struct".to_string(), vec![]),
                        };
                        expr = Expression::StructInst { name, generic_args, fields };
                    } else { break; }
                },
                TokenKind::Lt => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_expression() {
        let input = "1 + 2 * 3";
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let expr = parser.parse_expression();
        
        if let Expression::Infix { left: _, operator, right } = expr {
            assert_eq!(operator.kind, TokenKind::Plus);
            assert!(matches!(*right, Expression::Infix { .. }), "Expected right to be an infix expression");
            if let Expression::Infix { operator: op2, .. } = *right {
                assert_eq!(op2.kind, TokenKind::Star);
            }
        } else { panic!("Expected infix expression (+)"); }
    }

    #[test]
    fn test_parse_method_call() {
        // Use Vector.new().len() to force MethodCall
        let input = "Vector.new().len()";
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let expr = parser.parse_expression();
        
        let is_method_call = matches!(expr, Expression::MethodCall { .. });
        assert!(is_method_call, "Expected method call, got {:?}", expr);
    }

    #[test]
    fn test_parse_function() {
        let input = "fn add(a: i64, b: i64) -> i64 { return a + b; }";
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program();
        
        assert_eq!(program.declarations.len(), 1);
        let is_fn = matches!(program.declarations[0], Declaration::Function(_));
        assert!(is_fn, "Expected function declaration");
        if let Declaration::Function(f) = &program.declarations[0] {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.return_type, "i64");
        }
    }
}

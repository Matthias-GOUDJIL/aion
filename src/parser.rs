use crate::token::Token;
use crate::lexer::Lexer;
use crate::ast::*;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    peek_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next_token();
        let peek = lexer.next_token();
        Self { lexer, current_token: current, peek_token: peek }
    }

    fn next_token(&mut self) {
        self.current_token = self.peek_token.clone();
        self.peek_token = self.lexer.next_token();
    }

    pub fn parse_program(&mut self) -> Program {
        let mut module_name = None;
        let mut imports = Vec::new();
        let mut declarations = Vec::new();

        if let Token::Identifier(id) = &self.current_token {
            if id == "module" {
                self.next_token();
                module_name = Some(self.parse_path().join("."));
            }
        }

        while self.current_token != Token::EOF {
            match &self.current_token {
                Token::Use => imports.push(self.parse_import()),
                Token::Pub | Token::Async | Token::Unsafe | Token::Fn | Token::At | Token::Struct | Token::Enum | Token::Interface | Token::Impl => {
                    if let Some(decl) = self.parse_declaration() { declarations.push(decl); }
                },
                _ => self.next_token(),
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
                if let Token::Identifier(sub) = &self.current_token { path.push(sub.clone()); self.next_token(); }
            }
        }
        path
    }

    fn parse_import(&mut self) -> Import {
        self.next_token();
        let path = self.parse_path();
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
                    if let Token::StringLiteral(s) = &self.current_token { attr_val = s.clone(); self.next_token(); }
                    self.next_token();
                }
                attributes.push((attr_name, attr_val));
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

    fn parse_interface(&mut self) -> Option<Interface> {
        self.next_token(); // interface
        let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        if self.current_token != Token::LBrace { return None; }
        self.next_token();
        let mut methods = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if let Some(f) = self.parse_function(vec![], vec![]) { methods.push(f); }
            if self.current_token == Token::Semicolon { self.next_token(); }
        }
        self.next_token();
        Some(Interface { name, methods })
    }

    fn parse_impl(&mut self) -> Option<ImplBlock> {
        self.next_token(); // impl
        let name1 = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        
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
            if let Some(f) = self.parse_function(vec![], vec![]) { functions.push(f); }
        }
        self.next_token();
        Some(ImplBlock { target_name, interface_name, functions })
    }

    fn parse_generic_params(&mut self) -> Vec<String> {
        let mut params = Vec::new();
        if self.current_token == Token::Lt {
            self.next_token();
            while self.current_token != Token::Gt && self.current_token != Token::EOF {
                if let Token::Identifier(id) = &self.current_token { params.push(id.clone()); self.next_token(); }
                if self.current_token == Token::Comma { self.next_token(); }
            }
            self.next_token();
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
                        if let Token::Identifier(t) = &self.current_token { data_types.push(self.parse_type_name(t.clone())); self.next_token(); }
                        if self.current_token == Token::Comma { self.next_token(); }
                    }
                    self.next_token();
                }
                variants.push(EnumVariant { name: v_name, data_types });
            }
            if self.current_token == Token::Comma { self.next_token(); }
        }
        self.next_token();
        Some(Enum { name, generic_params, variants })
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
                    if let Token::Identifier(f_type) = &self.current_token {
                        fields.push((f_name, self.parse_type_name(f_type.clone())));
                        self.next_token();
                    }
                }
            }
            if self.current_token == Token::Comma { self.next_token(); }
        }
        self.next_token();
        Some(Struct { name, generic_params, fields, attributes: attributes.into_iter().map(|(k,_)| k).collect() })
    }

    // Gère le suffixe ? pour les types optionnels
    fn parse_type_name(&mut self, base: String) -> String {
        let mut full_type = base;
        if self.peek_token == Token::Question {
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
        if self.current_token != Token::LParen { return None; }
        self.next_token();
        while self.current_token != Token::RParen && self.current_token != Token::EOF { self.next_token(); }
        self.next_token();
        let mut return_type = "void".to_string();
        if self.current_token == Token::Arrow {
            self.next_token();
            if let Token::Identifier(t) = &self.current_token { return_type = self.parse_type_name(t.clone()); self.next_token(); }
        }
        let mut body = None;
        if self.current_token == Token::LBrace { body = Some(self.parse_block()); }
        Some(Function { name, generic_params, params: vec![], return_type, body, modifiers, attributes })
    }

    fn parse_block(&mut self) -> Vec<Statement> {
        let mut stmts = Vec::new();
        if self.current_token == Token::LBrace { self.next_token(); }
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if let Some(s) = self.parse_statement() { stmts.push(s); }
            if self.current_token == Token::Semicolon { self.next_token(); }
        }
        if self.current_token == Token::RBrace { self.next_token(); }
        stmts
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        let mut intent = None;
        if self.current_token == Token::DoubleColon {
            self.next_token();
            if self.current_token == Token::Intent {
                self.next_token();
                if let Token::StringLiteral(s) = &self.current_token { intent = Some(s.clone()); self.next_token(); }
            }
        }
        match self.current_token {
            Token::Let => {
                self.next_token();
                let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
                self.next_token();
                if self.current_token == Token::Eq { self.next_token(); let value = self.parse_expression(); Some(Statement::Let { name, value, intent, is_mut: false }) } else { None }
            },
            Token::Return => { self.next_token(); let value = self.parse_expression(); Some(Statement::Return { value, intent }) },
            Token::If => {
                self.next_token();
                let condition = self.parse_expression();
                let then_branch = self.parse_block();
                let mut else_branch = None;
                if self.current_token == Token::Else { self.next_token(); else_branch = Some(self.parse_block()); }
                Some(Statement::If { condition, then_branch, else_branch })
            },
            Token::Match => {
                self.next_token();
                let condition = self.parse_expression();
                if self.current_token != Token::LBrace { return None; }
                self.next_token();
                let mut arms = Vec::new();
                while self.current_token != Token::RBrace && self.current_token != Token::EOF {
                    if let Token::Identifier(p) = &self.current_token {
                        let pattern = p.clone(); self.next_token();
                        if self.current_token == Token::Arrow {
                            self.next_token();
                            let body = if self.current_token == Token::LBrace { self.parse_block() } else { vec![self.parse_statement().unwrap()] };
                            arms.push(MatchArm { pattern, body });
                        }
                    }
                    if self.current_token == Token::Comma { self.next_token(); }
                }
                self.next_token();
                Some(Statement::Match { condition, arms })
            },
            Token::For => {
                self.next_token();
                let var = match &self.current_token { Token::Identifier(n) => n.clone(), _ => "i".to_string() };
                self.next_token();
                if self.current_token == Token::In { self.next_token(); }
                let range = self.parse_expression();
                let body = self.parse_block();
                Some(Statement::For { var, range, body })
            },
            _ => Some(Statement::ExpressionStmt(self.parse_expression())),
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
            if op == Token::Range {
                let right = self.parse_infix(10); 
                left = Expression::Range { start: Box::new(left), end: Box::new(right) };
            } else {
                let right = self.parse_infix(self.get_precedence_for_op(&op));
                left = Expression::Infix { left: Box::new(left), operator: op, right: Box::new(right) };
            }
        }
        left
    }

    fn get_precedence(&self) -> i32 {
        self.get_precedence_for_op(&self.current_token)
    }

    fn get_precedence_for_op(&self, token: &Token) -> i32 {
        match token {
            Token::Range => 10,
            Token::Plus | Token::Minus => 5,
            Token::EqEq | Token::NotEq | Token::Inside => 3,
            _ => 0,
        }
    }

    fn parse_primary(&mut self) -> Expression {
        match self.current_token.clone() {
            Token::IntLiteral(n) => { self.next_token(); Expression::Integer(n) },
            Token::StringLiteral(s) => { self.next_token(); Expression::String(s) },
            Token::Identifier(n) => {
                let mut full_name = n;
                self.next_token();
                while self.current_token == Token::Dot {
                    self.next_token();
                    if let Token::Identifier(sub) = &self.current_token { full_name.push('.'); full_name.push_str(sub); self.next_token(); }
                }
                let mut generic_args = Vec::new();
                if self.current_token == Token::Lt {
                    self.next_token();
                    while self.current_token != Token::Gt && self.current_token != Token::EOF {
                        if let Token::Identifier(t) = &self.current_token { generic_args.push(t.clone()); self.next_token(); }
                        if self.current_token == Token::Comma { self.next_token(); }
                    }
                    self.next_token();
                }
                if self.current_token == Token::LBrace {
                    self.next_token();
                    let mut fields = Vec::new();
                    while self.current_token != Token::RBrace && self.current_token != Token::EOF {
                        if let Token::Identifier(f_name) = &self.current_token {
                            let f_name = f_name.clone(); self.next_token();
                            if self.current_token == Token::Colon || self.current_token == Token::Eq { self.next_token(); let val = self.parse_expression(); fields.push((f_name, val)); }
                        }
                        if self.current_token == Token::Comma { self.next_token(); }
                    }
                    self.next_token();
                    Expression::StructInst { name: full_name, generic_args, fields }
                } else if self.current_token == Token::LParen {
                    self.next_token();
                    let mut args = Vec::new();
                    while self.current_token != Token::RParen && self.current_token != Token::EOF {
                        args.push(self.parse_expression());
                        if self.current_token == Token::Comma { self.next_token(); }
                    }
                    self.next_token();
                    Expression::Call { function: full_name, arguments: args }
                } else {
                    Expression::Identifier(full_name)
                }
            },
            _ => { self.next_token(); Expression::Identifier("unknown".to_string()) },
        }
    }
}

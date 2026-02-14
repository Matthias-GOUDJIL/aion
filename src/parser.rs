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
                Token::Pub | Token::Async | Token::Unsafe | Token::Fn | Token::At | Token::Struct => {
                    if let Some(decl) = self.parse_declaration() {
                        declarations.push(decl);
                    }
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
                if let Token::Identifier(sub) = &self.current_token {
                    path.push(sub.clone());
                    self.next_token();
                }
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
                    if let Token::StringLiteral(s) = &self.current_token {
                        attr_val = s.clone();
                        self.next_token();
                    }
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
            _ => None,
        }
    }

    fn parse_struct(&mut self, attributes: Vec<(String, String)>) -> Option<Struct> {
        self.next_token();
        let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        if self.current_token != Token::LBrace { return None; }
        self.next_token();
        let mut fields = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if let Token::Identifier(f_name) = &self.current_token {
                let f_name = f_name.clone();
                self.next_token();
                if self.current_token == Token::Colon {
                    self.next_token();
                    if let Token::Identifier(f_type) = &self.current_token {
                        fields.push((f_name, f_type.clone()));
                        self.next_token();
                    }
                }
            }
            if self.current_token == Token::Comma { self.next_token(); }
        }
        self.next_token();
        Some(Struct { name, fields, attributes: attributes.into_iter().map(|(k,_)| k).collect() })
    }

    fn parse_function(&mut self, modifiers: Vec<Token>, attributes: Vec<(String, String)>) -> Option<Function> {
        self.next_token();
        let name = match &self.current_token { Token::Identifier(n) => n.clone(), _ => return None };
        self.next_token();
        if self.current_token != Token::LParen { return None; }
        self.next_token();
        while self.current_token != Token::RParen && self.current_token != Token::EOF { self.next_token(); }
        self.next_token();
        let mut return_type = "void".to_string();
        if self.current_token == Token::Arrow {
            self.next_token();
            if let Token::Identifier(t) = &self.current_token { return_type = t.clone(); self.next_token(); }
        }
        let mut body = None;
        if self.current_token == Token::LBrace { body = Some(self.parse_block()); }
        Some(Function { name, params: vec![], return_type, body, modifiers, attributes })
    }

    fn parse_block(&mut self) -> Vec<Statement> {
        let mut stmts = Vec::new();
        self.next_token();
        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if let Some(s) = self.parse_statement() { stmts.push(s); }
            if self.current_token == Token::Semicolon { self.next_token(); }
            else if self.current_token != Token::RBrace { self.next_token(); }
        }
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
                if self.current_token == Token::Eq {
                    self.next_token();
                    let value = self.parse_expression();
                    Some(Statement::Let { name, value, intent, is_mut: false })
                } else { None }
            },
            Token::Return => {
                self.next_token();
                let value = self.parse_expression();
                Some(Statement::Return { value, intent })
            },
            _ => Some(Statement::ExpressionStmt(self.parse_expression())),
        }
    }

    fn parse_expression(&mut self) -> Expression {
        match &self.current_token {
            Token::IntLiteral(n) => Expression::Integer(*n),
            Token::StringLiteral(s) => Expression::String(s.clone()),
            Token::Identifier(n) => {
                let name = n.clone();
                // Check for dot notation: io.println
                let mut full_name = name;
                while self.peek_token == Token::Dot {
                    self.next_token(); // consume prev
                    self.next_token(); // consume .
                    if let Token::Identifier(sub) = &self.current_token {
                        full_name.push('.');
                        full_name.push_str(sub);
                    }
                }

                if self.peek_token == Token::LBrace {
                    self.next_token(); // consume name/sub
                    self.next_token(); // consume {
                    let mut fields = Vec::new();
                    while self.current_token != Token::RBrace && self.current_token != Token::EOF {
                        if let Token::Identifier(f_name) = &self.current_token {
                            let f_name = f_name.clone();
                            self.next_token();
                            if self.current_token == Token::Eq || self.current_token == Token::Colon {
                                self.next_token();
                                let val = self.parse_expression();
                                fields.push((f_name, val));
                            }
                        }
                        if self.current_token == Token::Comma { self.next_token(); }
                        else { self.next_token(); }
                    }
                    Expression::StructInst { name: full_name, fields }
                } else if self.peek_token == Token::LParen {
                    self.next_token(); // consume name/sub
                    self.next_token(); // consume (
                    let mut args = Vec::new();
                    while self.current_token != Token::RParen && self.current_token != Token::EOF {
                        args.push(self.parse_expression());
                        self.next_token();
                        if self.current_token == Token::Comma { self.next_token(); }
                    }
                    Expression::Call { function: full_name, arguments: args }
                } else {
                    Expression::Identifier(full_name)
                }
            },
            _ => Expression::Identifier("unknown".to_string()),
        }
    }
}

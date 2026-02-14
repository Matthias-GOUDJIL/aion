use crate::token::Token;
use std::iter::Peekable;
use std::str::Chars;

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input: input.chars().peekable() }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        match self.input.next() {
            Some(ch) => match ch {
                '(' => Token::LParen,
                ')' => Token::RParen,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                ',' => Token::Comma,
                ';' => Token::Semicolon,
                '*' => Token::Star,
                '+' => Token::Plus,
                '.' => {
                    if self.peek_char() == '.' {
                        self.input.next();
                        Token::Range
                    } else {
                        Token::Dot
                    }
                }
                '-' => {
                    if self.peek_char() == '>' {
                        self.input.next();
                        Token::Arrow
                    } else {
                        Token::Minus
                    }
                }
                '=' => {
                    if self.peek_char() == '=' {
                        self.input.next();
                        Token::EqEq
                    } else if self.peek_char() == '>' {
                        self.input.next();
                        Token::Arrow
                    } else {
                        Token::Eq
                    }
                }
                '!' => {
                    if self.peek_char() == '=' {
                        self.input.next();
                        Token::NotEq
                    } else {
                        Token::Illegal('!')
                    }
                }
                '>' => {
                    if self.peek_char() == '=' {
                        self.input.next();
                        Token::GtEq
                    } else {
                        Token::Gt
                    }
                }
                '<' => {
                    if self.peek_char() == '=' {
                        self.input.next();
                        Token::LtEq
                    } else {
                        Token::Lt
                    }
                }
                ':' => {
                    if self.peek_char() == ':' {
                        self.input.next();
                        Token::DoubleColon
                    } else {
                        Token::Colon
                    }
                }
                '"' => self.read_string(),
                c if c.is_alphabetic() || c == '_' => {
                    let ident = self.read_identifier(c);
                    match ident.as_str() {
                        "fn" => Token::Fn,
                        "let" => Token::Let,
                        "mut" => Token::Mut,
                        "struct" => Token::Struct,
                        "enum" => Token::Enum,
                        "return" => Token::Return,
                        "if" => Token::If,
                        "else" => Token::Else,
                        "match" => Token::Match,
                        "use" => Token::Use,
                        "pub" => Token::Pub,
                        "async" => Token::Async,
                        "unsafe" => Token::Unsafe,
                        "require" => Token::Require,
                        "intent" => Token::Intent,
                        "inside" => Token::Inside,
                        "spawn" => Token::Identifier("spawn".to_string()), // Force spawn as identifier for parser
                        _ => Token::Identifier(ident),
                    }
                }
                c if c.is_numeric() => self.read_number(c),
                _ => Token::Illegal(ch),
            },
            None => Token::EOF,
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&ch) = self.input.peek() {
            if !ch.is_whitespace() { break; }
            self.input.next();
        }
    }

    fn peek_char(&mut self) -> char {
        *self.input.peek().unwrap_or(&'\0')
    }

    fn read_identifier(&mut self, first: char) -> String {
        let mut ident = String::from(first);
        while let Some(&ch) = self.input.peek() {
            if !ch.is_alphanumeric() && ch != '_' { break; }
            ident.push(self.input.next().unwrap());
        }
        ident
    }

    fn read_string(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch == '"' {
                self.input.next();
                break;
            }
            s.push(self.input.next().unwrap());
        }
        Token::StringLiteral(s)
    }

    fn read_number(&mut self, first: char) -> Token {
        let mut num = String::from(first);
        let mut is_float = false;
        while let Some(&ch) = self.input.peek() {
            if ch == '.' {
                let mut iter = self.input.clone();
                iter.next();
                if let Some(&next) = iter.peek() {
                    if next == '.' { break; }
                }
                is_float = true;
                num.push(self.input.next().unwrap());
            } else if ch.is_numeric() {
                num.push(self.input.next().unwrap());
            } else {
                break;
            }
        }
        if is_float {
            Token::FloatLiteral(num.parse().unwrap_or(0.0))
        } else {
            Token::IntLiteral(num.parse().unwrap_or(0))
        }
    }
}

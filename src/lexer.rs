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
                '%' => Token::Percent,
                '^' => Token::Caret,
                '/' => {
                    if self.peek_char() == '/' {
                        self.input.next();
                        self.read_line_comment();
                        self.next_token()
                    } else if self.peek_char() == '*' {
                        self.input.next();
                        self.read_block_comment();
                        self.next_token()
                    } else {
                        Token::Slash
                    }
                },
                '?' => Token::Question,
                '.' => {
                    if self.peek_char() == '.' { self.input.next(); Token::Range } 
                    else { Token::Dot }
                }
                '-' => {
                    if self.peek_char() == '>' { self.input.next(); Token::Arrow } 
                    else { Token::Minus }
                }
                '=' => {
                    if self.peek_char() == '=' { self.input.next(); Token::EqEq } 
                    else if self.peek_char() == '>' { self.input.next(); Token::Arrow }
                    else { Token::Eq }
                }
                '!' => {
                    if self.peek_char() == '=' { self.input.next(); Token::NotEq } 
                    else { Token::Bang }
                }
                '&' => {
                    if self.peek_char() == '&' { self.input.next(); Token::And }
                    else { Token::Illegal('&') }
                },
                '|' => {
                    if self.peek_char() == '>' { self.input.next(); Token::Pipeline } 
                    else if self.peek_char() == '|' { self.input.next(); Token::Or }
                    else { Token::Illegal('|') }
                },
                '>' => {
                    if self.peek_char() == '=' { self.input.next(); Token::GtEq } 
                    else { Token::Gt }
                }
                '<' => {
                    if self.peek_char() == '=' { self.input.next(); Token::LtEq } 
                    else if self.peek_char() == '-' { self.input.next(); Token::LArrow }
                    else { Token::Lt }
                }
                ':' => {
                    if self.peek_char() == ':' { self.input.next(); Token::DoubleColon } 
                    else { Token::Colon }
                }
                '@' => Token::At,
                '"' => self.read_string(),
                c if c.is_alphabetic() || c == '_' => {
                    if c == 'f' && self.peek_char() == '"' {
                        self.input.next(); 
                        self.read_fstring()
                    } else if c == 'D' && self.peek_char().is_numeric() {
                        self.read_date()
                    } else {
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
                            "while" => Token::While,
                            "match" => Token::Match,
                            "as" => Token::As,
                            "use" => Token::Use,
                            "pub" => Token::Pub,
                            "async" => Token::Async,
                            "unsafe" => Token::Unsafe,
                            "require" => Token::Require,
                            "intent" => Token::Intent,
                            "invariant" => Token::Invariant,
                            "inside" => Token::Inside,
                            "interface" => Token::Interface,
                            "impl" => Token::Impl,
                            "channel" => Token::Channel,
                            "for" => Token::For,
                            "in" => Token::In,
                            "type" => Token::Type,
                            "self" => Token::SelfToken,
                            "spawn" => Token::Spawn,
                            "true" => Token::True,
                            "false" => Token::False,
                            _ => Token::Identifier(ident),
                        }
                    }
                }
                c if c.is_numeric() => self.read_number(c),
                _ => Token::Illegal(ch),
            },
            None => Token::EOF,
        }
    }

    fn read_line_comment(&mut self) {
        while let Some(&ch) = self.input.peek() {
            if ch == '\n' { break; }
            self.input.next();
        }
    }

    fn read_block_comment(&mut self) {
        while let Some(ch) = self.input.next() {
            if ch == '*' {
                if let Some(&next) = self.input.peek() {
                    if next == '/' {
                        self.input.next();
                        break;
                    }
                }
            }
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
            if ch == '"' { self.input.next(); break; }
            s.push(self.input.next().unwrap());
        }
        Token::StringLiteral(s)
    }

    fn read_fstring(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch == '"' { self.input.next(); break; }
            s.push(self.input.next().unwrap());
        }
        Token::FString(s)
    }

    fn read_date(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch.is_numeric() || ch == '-' {
                s.push(self.input.next().unwrap());
            } else {
                break;
            }
        }
        
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 3 {
            let y: i64 = parts[0].parse().unwrap_or(0);
            let m: i64 = parts[1].parse().unwrap_or(1);
            let d: i64 = parts[2].parse().unwrap_or(1);
            
            let mut total_days = 0;
            for year in 1970..y {
                if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                    total_days += 366;
                } else {
                    total_days += 365;
                }
            }
            
            let days_in_month = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            for month in 1..m {
                total_days += days_in_month[month as usize];
                if month == 2 && ((y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)) {
                    total_days += 1;
                }
            }
            
            total_days += d - 1;
            
            let ts = total_days * 86400;
            Token::DateLiteral(ts)
        } else {
            Token::Illegal('D')
        }
    }

    fn read_number(&mut self, first: char) -> Token {
        let mut num_str = String::from(first);
        let mut is_float = false;
        
        while let Some(&ch) = self.input.peek() {
            if ch == '.' {
                let mut lookahead = self.input.clone();
                lookahead.next();
                if let Some(&next) = lookahead.peek() {
                    if next == '.' { break; }
                }
                
                is_float = true;
                num_str.push(self.input.next().unwrap());
            } else if ch.is_numeric() {
                num_str.push(self.input.next().unwrap());
            } else {
                break;
            }
        }

        let mut suffix = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch.is_alphabetic() {
                suffix.push(self.input.next().unwrap());
            } else {
                break;
            }
        }

        if !suffix.is_empty() {
            let val: f64 = num_str.parse().unwrap_or(0.0);
            let (secs, nanos) = match suffix.as_str() {
                "s" => (val as u64, (val.fract() * 1_000_000_000.0) as u32),
                "ms" => ((val / 1000.0) as u64, ((val % 1000.0) * 1_000_000.0) as u32),
                "us" => ((val / 1_000_000.0) as u64, ((val % 1_000_000.0) * 1000.0) as u32),
                "ns" => ((val / 1_000_000_000.0) as u64, (val % 1_000_000_000.0) as u32),
                "m" => ((val * 60.0) as u64, 0),
                "h" => ((val * 3600.0) as u64, 0),
                "d" => ((val * 86400.0) as u64, 0),
                _ => return Token::Illegal('?'),
            };
            return Token::DurationLiteral(secs, nanos);
        }

        if is_float { 
            Token::FloatLiteral(num_str.parse().unwrap_or(0.0)) 
        } else { 
            Token::IntLiteral(num_str.parse().unwrap_or(0)) 
        }
    }
}

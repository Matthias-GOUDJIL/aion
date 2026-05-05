use crate::token::{Token, TokenKind};
use std::iter::Peekable;
use std::str::Chars;

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { 
            input: input.chars().peekable(),
            line: 1,
            col: 1,
        }
    }

    fn read_char(&mut self) -> Option<char> {
        let ch = self.input.next();
        if let Some(c) = ch {
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        ch
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let line = self.line;
        let col = self.col;

        let kind = match self.read_char() {
            Some(ch) => match ch {
                '(' => TokenKind::LParen,
                ')' => TokenKind::RParen,
                '{' => TokenKind::LBrace,
                '}' => TokenKind::RBrace,
                '[' => TokenKind::LBracket,
                ']' => TokenKind::RBracket,
                ',' => TokenKind::Comma,
                ';' => TokenKind::Semicolon,
                '*' => TokenKind::Star,
                '+' => TokenKind::Plus,
                '%' => TokenKind::Percent,
                '^' => TokenKind::Caret,
                '/' => {
                    if self.peek_char() == '/' {
                        self.read_char(); // consume first /
                        if self.peek_char() == '/' {
                            self.read_char(); // consume second /
                            return self.read_doc_comment(line, col);
                        }
                        self.read_line_comment();
                        return self.next_token();
                    } else if self.peek_char() == '*' {
                        self.read_char(); // consume *
                        self.read_block_comment();
                        return self.next_token();
                    } else {
                        TokenKind::Slash
                    }
                },
                '?' => TokenKind::Question,
                '.' => {
                    if self.peek_char() == '.' { self.read_char(); TokenKind::Range } 
                    else { TokenKind::Dot }
                }
                '-' => {
                    if self.peek_char() == '>' { self.read_char(); TokenKind::Arrow } 
                    else { TokenKind::Minus }
                }
                '=' => {
                    if self.peek_char() == '=' { self.read_char(); TokenKind::EqEq } 
                    else if self.peek_char() == '>' { self.read_char(); TokenKind::Arrow }
                    else { TokenKind::Eq }
                }
                '!' => {
                    if self.peek_char() == '=' { self.read_char(); TokenKind::NotEq } 
                    else { TokenKind::Bang }
                }
                '&' => {
                    if self.peek_char() == '&' { self.read_char(); TokenKind::And }
                    else { TokenKind::Illegal('&') }
                },
'|' => {
                    if self.peek_char() == '|' { self.read_char(); TokenKind::Or }
                    else if self.peek_char() == '>' { self.read_char(); TokenKind::Pipeline }
                    else { TokenKind::Pipe }
                },
                '>' => {
                    if self.peek_char() == '=' { self.read_char(); TokenKind::GtEq } 
                    else { TokenKind::Gt }
                }
                '<' => {
                    if self.peek_char() == '=' { self.read_char(); TokenKind::LtEq } 
                    else if self.peek_char() == '-' { self.read_char(); TokenKind::LArrow }
                    else { TokenKind::Lt }
                }
                ':' => {
                    if self.peek_char() == ':' { self.read_char(); TokenKind::DoubleColon } 
                    else { TokenKind::Colon }
                }
                '@' => TokenKind::At,
                '"' => self.read_string(),
                c if c.is_alphabetic() || c == '_' => {
                    if c == 'f' && self.peek_char() == '"' {
                        self.read_char(); 
                        self.read_fstring()
                    } else if c == 'D' && self.peek_char().is_numeric() {
                        self.read_date()
                    } else {
                        let ident = self.read_identifier(c);
                        match ident.as_str() {
                            "fn" => TokenKind::Fn,
                            "let" => TokenKind::Let,
                            "mut" => TokenKind::Mut,
                            "struct" => TokenKind::Struct,
                            "enum" => TokenKind::Enum,
                            "return" => TokenKind::Return,
                            "if" => TokenKind::If,
                            "else" => TokenKind::Else,
                            "while" => TokenKind::While,
                            "match" => TokenKind::Match,
                            "as" => TokenKind::As,
                            "use" => TokenKind::Use,
                            "pub" => TokenKind::Pub,
                            "async" => TokenKind::Async,
                            "unsafe" => TokenKind::Unsafe,
                            "extern" => TokenKind::Extern,
                            "require" => TokenKind::Require,
                            "intent" => TokenKind::Intent,
                            "invariant" => TokenKind::Invariant,
                            "inside" => TokenKind::Inside,
                            "interface" => TokenKind::Interface,
                            "impl" => TokenKind::Impl,
                            "channel" => TokenKind::Channel,
                            "for" => TokenKind::For,
                            "in" => TokenKind::In,
                            "type" => TokenKind::Type,
                            "self" => TokenKind::SelfToken,
                            "spawn" => TokenKind::Spawn,
                            "true" => TokenKind::True,
                            "false" => TokenKind::False,
                            _ => TokenKind::Identifier(ident),
                        }
                    }
                }
                c if c.is_numeric() => self.read_number(c),
                _ => TokenKind::Illegal(ch),
            },
            None => TokenKind::EOF,
        };

        Token::new(kind, line, col)
    }

    fn read_line_comment(&mut self) {
        while let Some(&ch) = self.input.peek() {
            if ch == '\n' { break; }
            self.read_char();
        }
    }

    fn read_doc_comment(&mut self, line: usize, col: usize) -> Token {
        let mut text = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch == '\n' { break; }
            if let Some(c) = self.read_char() { text.push(c); }
        }
        Token::new(TokenKind::DocComment(text.trim().to_string()), line, col)
    }

    fn read_block_comment(&mut self) {
        while let Some(ch) = self.read_char() {
            if ch == '*'
                && let Some(&next) = self.input.peek()
                    && next == '/' {
                        self.read_char();
                        break;
                    }
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&ch) = self.input.peek() {
            if !ch.is_whitespace() { break; }
            self.read_char();
        }
    }

    fn peek_char(&mut self) -> char {
        *self.input.peek().unwrap_or(&'\0')
    }

    fn read_identifier(&mut self, first: char) -> String {
        let mut ident = String::from(first);
        while let Some(&ch) = self.input.peek() {
            if !ch.is_alphanumeric() && ch != '_' { break; }
            if let Some(c) = self.read_char() { ident.push(c); }
        }
        ident
    }

    fn read_string(&mut self) -> TokenKind {
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch == '"' { self.read_char(); break; }
            if let Some(c) = self.read_char() { s.push(c); }
        }
        TokenKind::StringLiteral(s)
    }

    fn read_fstring(&mut self) -> TokenKind {
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch == '"' { self.read_char(); break; }
            if let Some(c) = self.read_char() { s.push(c); }
        }
        TokenKind::FString(s)
    }

    fn read_date(&mut self) -> TokenKind {
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch.is_numeric() || ch == '-' {
                s.push(self.read_char().unwrap());
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
            TokenKind::DateLiteral(ts)
        } else {
            TokenKind::Illegal('D')
        }
    }

    fn read_number(&mut self, first: char) -> TokenKind {
        let mut num_str = String::from(first);
        let mut is_float = false;
        
        while let Some(&ch) = self.input.peek() {
            if ch == '.' {
                let mut lookahead = self.input.clone();
                lookahead.next();
                if let Some(&next) = lookahead.peek()
                    && next == '.' { break; }
                
                is_float = true;
                if let Some(c) = self.read_char() { num_str.push(c); }
            } else if ch.is_numeric() {
                if let Some(c) = self.read_char() { num_str.push(c); }
            } else {
                break;
            }
        }

        let mut suffix = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch.is_alphabetic() {
                if let Some(c) = self.read_char() { suffix.push(c); }
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
                _ => return TokenKind::Illegal('?'),
            };
            return TokenKind::DurationLiteral(secs, nanos);
        }

        if is_float { 
            TokenKind::FloatLiteral(num_str.parse().unwrap_or(0.0)) 
        } else { 
            TokenKind::IntLiteral(num_str.parse().unwrap_or(0)) 
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenKind;

    #[test]
    fn test_basic_tokens() {
        let input = "fn main() { let x = 10; }";
        let mut lexer = Lexer::new(input);
        
        let expected = vec![
            (TokenKind::Fn, 1, 1),
            (TokenKind::Identifier("main".into()), 1, 4),
            (TokenKind::LParen, 1, 8),
            (TokenKind::RParen, 1, 9),
            (TokenKind::LBrace, 1, 11),
            (TokenKind::Let, 1, 13),
            (TokenKind::Identifier("x".into()), 1, 17),
            (TokenKind::Eq, 1, 19),
            (TokenKind::IntLiteral(10), 1, 21),
            (TokenKind::Semicolon, 1, 23),
            (TokenKind::RBrace, 1, 25),
        ];

        for (kind, line, col) in expected {
            let tok = lexer.next_token();
            assert_eq!(tok.kind, kind);
            assert_eq!(tok.line, line);
            assert_eq!(tok.col, col);
        }
    }

    #[test]
    fn test_durations() {
        let input = "10s 500ms 1.5ms";
        let mut lexer = Lexer::new(input);
        
        let tok1 = lexer.next_token();
        assert_eq!(tok1.kind, TokenKind::DurationLiteral(10, 0));
        
        let tok2 = lexer.next_token();
        assert_eq!(tok2.kind, TokenKind::DurationLiteral(0, 500_000_000));
        
        let tok3 = lexer.next_token();
        assert_eq!(tok3.kind, TokenKind::DurationLiteral(0, 1_500_000));
    }

    #[test]
    fn test_multiline_position() {
        let input = "let a = 1\nlet b = 2";
        let mut lexer = Lexer::new(input);
        
        lexer.next_token(); // let
        lexer.next_token(); // a
        lexer.next_token(); // =
        lexer.next_token(); // 1
        
        let tok = lexer.next_token(); // second 'let'
        assert_eq!(tok.kind, TokenKind::Let);
        assert_eq!(tok.line, 2);
        assert_eq!(tok.col, 1);
    }

    #[test]
    fn test_comments() {
        let input = "// line comment\nlet x = 1 /* block\ncomment */ let y = 2";
        let mut lexer = Lexer::new(input);
        
        let tok1 = lexer.next_token();
        assert_eq!(tok1.kind, TokenKind::Let);
        assert_eq!(tok1.line, 2);
        
        lexer.next_token(); // x
        lexer.next_token(); // =
        lexer.next_token(); // 1
        
        let tok2 = lexer.next_token(); // y 'let'
        assert_eq!(tok2.kind, TokenKind::Let);
        assert_eq!(tok2.line, 3);
    }
}

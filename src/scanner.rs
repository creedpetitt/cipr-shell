use std::collections::HashMap;

use crate::token::{Token, TokenType, Value};

pub struct Scanner {
    source: Vec<char>,
    tokens: Vec<Token>,
    keywords: HashMap<&'static str, TokenType>,
    start: usize,
    current: usize,
    line: usize,
    pub had_error: bool,
}

impl Scanner {
    pub fn new(source: &str) -> Self {
        let mut keywords = HashMap::new();
        keywords.insert("and", TokenType::And);
        keywords.insert("class", TokenType::Class);
        keywords.insert("else", TokenType::Else);
        keywords.insert("false", TokenType::False);
        keywords.insert("fn", TokenType::Fn);
        keywords.insert("for", TokenType::For);
        keywords.insert("if", TokenType::If);
        keywords.insert("null", TokenType::Null);
        keywords.insert("or", TokenType::Or);
        keywords.insert("return", TokenType::Return);
        keywords.insert("super", TokenType::Super);
        keywords.insert("this", TokenType::This);
        keywords.insert("true", TokenType::True);
        keywords.insert("let", TokenType::Let);
        keywords.insert("while", TokenType::While);

        Self {
            source: source.chars().collect(),
            tokens: Vec::new(),
            keywords,
            start: 0,
            current: 0,
            line: 1,
            had_error: false,
        }
    }

    pub fn scan_tokens(mut self) -> (Vec<Token>, bool) {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token();
        }

        self.tokens.push(Token::new(
            TokenType::EofToken,
            String::new(),
            Value::Null,
            self.line,
        ));

        let had_error = self.had_error;
        (self.tokens, had_error)
    }

    fn scan_token(&mut self) {
        let c = self.advance();
        match c {
            '(' => self.add_token(TokenType::LeftParen),
            ')' => self.add_token(TokenType::RightParen),
            '{' => self.add_token(TokenType::LeftBrace),
            '}' => self.add_token(TokenType::RightBrace),
            '[' => self.add_token(TokenType::LeftBracket),
            ']' => self.add_token(TokenType::RightBracket),
            '$' => self.add_token(TokenType::Dollar),
            ',' => self.add_token(TokenType::Comma),
            '.' => self.add_token(TokenType::Dot),
            '-' => self.add_token(TokenType::Minus),
            '+' => self.add_token(TokenType::Plus),
            ';' => self.add_token(TokenType::Semicolon),
            ':' => self.add_token(TokenType::Colon),
            '*' => self.add_token(TokenType::Star),

            '!' => {
                let tt = if self.match_char('=') {
                    TokenType::BangEqual
                } else {
                    TokenType::Bang
                };
                self.add_token(tt);
            }
            '=' => {
                let tt = if self.match_char('=') {
                    TokenType::EqualEqual
                } else {
                    TokenType::Equal
                };
                self.add_token(tt);
            }
            '<' => {
                let tt = if self.match_char('=') {
                    TokenType::LessEqual
                } else {
                    TokenType::Less
                };
                self.add_token(tt);
            }
            '>' => {
                let tt = if self.match_char('=') {
                    TokenType::GreaterEqual
                } else {
                    TokenType::Greater
                };
                self.add_token(tt);
            }

            '/' => {
                if self.match_char('/') {
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                } else {
                    self.add_token(TokenType::Slash);
                }
            }

            '"' => self.string('"'),
            '\'' => self.string('\''),

            ' ' | '\r' | '\t' => {}

            '\n' => self.line += 1,

            _ => {
                if c.is_ascii_digit() {
                    self.number();
                } else if is_alpha(c) {
                    self.identifier();
                } else {
                    self.error("Unexpected character.");
                }
            }
        }
    }

    fn string(&mut self, delimiter: char) {
        let mut value = String::new();

        while self.peek() != delimiter && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
            }

            if self.peek() == '\\' {
                self.advance();
                if self.is_at_end() {
                    break;
                }
                match self.peek() {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    '\'' => value.push('\''),
                    other => {
                        value.push('\\');
                        value.push(other);
                    }
                }
            } else {
                value.push(self.peek());
            }
            self.advance();
        }

        if self.is_at_end() {
            self.error("Unterminated string.");
            return;
        }

        self.advance(); // closing delimiter
        self.add_token_lit(TokenType::Str, Value::Str(value));
    }

    fn number(&mut self) {
        while self.peek().is_ascii_digit() {
            self.advance();
        }

        let mut is_float = false;
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        let text: String = self.source[self.start..self.current].iter().collect();
        if is_float {
            match text.parse::<f64>() {
                Ok(n) => self.add_token_lit(TokenType::Float, Value::Float(n)),
                Err(_) => self.error("Numeric literal is invalid or too large for a 64-bit float."),
            }
        } else {
            match text.parse::<i64>() {
                Ok(n) => self.add_token_lit(TokenType::Int, Value::Int(n)),
                Err(_) => self.error("Numeric literal is too large for a 64-bit integer."),
            }
        }
    }

    fn identifier(&mut self) {
        while is_alpha_numeric(self.peek()) {
            self.advance();
        }

        let text: String = self.source[self.start..self.current].iter().collect();
        let token_type = self
            .keywords
            .get(text.as_str())
            .copied()
            .unwrap_or(TokenType::Identifier);
        self.add_token(token_type);
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        c
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.source[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.source[self.current]
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.source.len() {
            '\0'
        } else {
            self.source[self.current + 1]
        }
    }

    fn add_token(&mut self, token_type: TokenType) {
        self.add_token_lit(token_type, Value::Null);
    }

    fn add_token_lit(&mut self, token_type: TokenType, literal: Value) {
        let text: String = self.source[self.start..self.current].iter().collect();
        self.tokens
            .push(Token::new(token_type, text, literal, self.line));
    }

    fn error(&mut self, message: &str) {
        eprintln!("[line {}] Error: {}", self.line, message);
        self.had_error = true;
    }
}

fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_alpha_numeric(c: char) -> bool {
    is_alpha(c) || c.is_ascii_digit()
}

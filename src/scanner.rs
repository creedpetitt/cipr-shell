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
        keywords.insert("extern", TokenType::Extern);
        keywords.insert("false", TokenType::False);
        keywords.insert("fn", TokenType::Fn);
        keywords.insert("for", TokenType::For);
        keywords.insert("if", TokenType::If);
        keywords.insert("include", TokenType::Include);
        keywords.insert("null", TokenType::Null);
        keywords.insert("or", TokenType::Or);
        keywords.insert("new", TokenType::New);
        keywords.insert("delete", TokenType::Delete);
        keywords.insert("return", TokenType::Return);
        keywords.insert("struct", TokenType::Struct);
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
            ',' => self.add_token(TokenType::Comma),
            '.' => self.add_token(TokenType::Dot),
            '-' => self.add_token(TokenType::Minus),
            '+' => self.add_token(TokenType::Plus),
            ';' => self.add_token(TokenType::Semicolon),
            ':' => self.add_token(TokenType::Colon),
            '*' => self.add_token(TokenType::Star),
            '@' => self.add_token(TokenType::At),

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::{TokenType, Value};

    fn scan(src: &str) -> (Vec<Token>, bool) {
        Scanner::new(src).scan_tokens()
    }

    /// Collect just the token types from a source string.
    fn token_types(src: &str) -> Vec<TokenType> {
        let (tokens, _) = scan(src);
        tokens.iter().map(|t| t.token_type).collect()
    }

    // ── Keywords ─────────────────────────────────────────────────────────────

    #[test]
    fn all_keywords_recognised() {
        let cases = [
            ("fn",      TokenType::Fn),
            ("let",     TokenType::Let),
            ("if",      TokenType::If),
            ("else",    TokenType::Else),
            ("while",   TokenType::While),
            ("for",     TokenType::For),
            ("return",  TokenType::Return),
            ("struct",  TokenType::Struct),
            ("and",     TokenType::And),
            ("or",      TokenType::Or),
            ("true",    TokenType::True),
            ("false",   TokenType::False),
            ("null",    TokenType::Null),
            ("new",     TokenType::New),
            ("delete",  TokenType::Delete),
            ("extern",  TokenType::Extern),
            ("include", TokenType::Include),
            ("class",   TokenType::Class),
            ("super",   TokenType::Super),
            ("this",    TokenType::This),
        ];
        for (src, expected) in cases {
            let ts = token_types(src);
            assert_eq!(ts[0], expected, "expected keyword token for '{}'", src);
        }
    }

    #[test]
    fn type_names_are_identifiers_not_keywords() {
        // int/float/bool/str/void are NOT keywords in the scanner; the
        // parser resolves them as type annotations from regular Identifiers.
        for name in ["int", "float", "bool", "str", "void"] {
            let ts = token_types(name);
            assert_eq!(
                ts[0],
                TokenType::Identifier,
                "'{}' should scan as Identifier, not a keyword",
                name
            );
        }
    }

    // ── Single-character tokens ───────────────────────────────────────────────

    #[test]
    fn all_single_char_tokens() {
        let cases = [
            ('(', TokenType::LeftParen),
            (')', TokenType::RightParen),
            ('{', TokenType::LeftBrace),
            ('}', TokenType::RightBrace),
            ('[', TokenType::LeftBracket),
            (']', TokenType::RightBracket),
            (',', TokenType::Comma),
            ('.', TokenType::Dot),
            ('-', TokenType::Minus),
            ('+', TokenType::Plus),
            (';', TokenType::Semicolon),
            (':', TokenType::Colon),
            ('*', TokenType::Star),
            ('@', TokenType::At),
        ];
        for (ch, expected) in cases {
            let ts = token_types(&ch.to_string());
            assert_eq!(ts[0], expected, "single char '{}'", ch);
        }
    }

    #[test]
    fn slash_without_following_slash_is_slash_token() {
        let ts = token_types("/");
        assert_eq!(ts[0], TokenType::Slash);
    }

    // ── Two-character tokens ──────────────────────────────────────────────────

    #[test]
    fn two_char_equal_equal() {
        assert_eq!(token_types("==")[0], TokenType::EqualEqual);
    }

    #[test]
    fn two_char_bang_equal() {
        assert_eq!(token_types("!=")[0], TokenType::BangEqual);
    }

    #[test]
    fn two_char_less_equal() {
        assert_eq!(token_types("<=")[0], TokenType::LessEqual);
    }

    #[test]
    fn two_char_greater_equal() {
        assert_eq!(token_types(">=")[0], TokenType::GreaterEqual);
    }

    #[test]
    fn single_char_fallbacks_when_not_followed_by_equal() {
        assert_eq!(token_types("=")[0],  TokenType::Equal);
        assert_eq!(token_types("!")[0],  TokenType::Bang);
        assert_eq!(token_types("<")[0],  TokenType::Less);
        assert_eq!(token_types(">")[0],  TokenType::Greater);
    }

    // ── String literals ───────────────────────────────────────────────────────

    #[test]
    fn double_quoted_string() {
        let (tokens, had_error) = scan(r#""hello""#);
        assert!(!had_error);
        assert_eq!(tokens[0].token_type, TokenType::Str);
        assert_eq!(tokens[0].literal, Value::Str("hello".into()));
    }

    #[test]
    fn single_quoted_string() {
        let (tokens, had_error) = scan("'world'");
        assert!(!had_error);
        assert_eq!(tokens[0].token_type, TokenType::Str);
        assert_eq!(tokens[0].literal, Value::Str("world".into()));
    }

    #[test]
    fn empty_string() {
        let (tokens, had_error) = scan(r#""""#);
        assert!(!had_error);
        assert_eq!(tokens[0].literal, Value::Str(String::new()));
    }

    #[test]
    fn string_escape_newline_and_tab() {
        let (tokens, had_error) = scan(r#""\n\t""#);
        assert!(!had_error);
        assert_eq!(tokens[0].literal, Value::Str("\n\t".into()));
    }

    #[test]
    fn string_escape_backslash() {
        let (tokens, had_error) = scan(r#""\\""#);
        assert!(!had_error);
        assert_eq!(tokens[0].literal, Value::Str("\\".into()));
    }

    #[test]
    fn string_escape_carriage_return() {
        let (tokens, had_error) = scan(r#""\r""#);
        assert!(!had_error);
        assert_eq!(tokens[0].literal, Value::Str("\r".into()));
    }

    #[test]
    fn string_escape_double_quote_inside_double_quoted() {
        let (tokens, had_error) = scan(r#""say \"hi\"""#);
        assert!(!had_error);
        assert_eq!(tokens[0].literal, Value::Str(r#"say "hi""#.into()));
    }

    #[test]
    fn unterminated_string_sets_had_error() {
        let (_, had_error) = scan(r#""oops"#);
        assert!(had_error, "unterminated string must set had_error");
    }

    #[test]
    fn unterminated_single_quoted_string_sets_had_error() {
        let (_, had_error) = scan("'oops");
        assert!(had_error);
    }

    // ── Integer literals ──────────────────────────────────────────────────────

    #[test]
    fn integer_zero() {
        let (tokens, had_error) = scan("0");
        assert!(!had_error);
        assert_eq!(tokens[0].token_type, TokenType::Int);
        assert_eq!(tokens[0].literal, Value::Int(0));
    }

    #[test]
    fn integer_positive() {
        let (tokens, had_error) = scan("42");
        assert!(!had_error);
        assert_eq!(tokens[0].literal, Value::Int(42));
    }

    #[test]
    fn integer_large() {
        let (tokens, had_error) = scan("1000000");
        assert!(!had_error);
        assert_eq!(tokens[0].literal, Value::Int(1_000_000));
    }

    #[test]
    fn negative_is_minus_token_then_int() {
        // The scanner has no concept of negative literals; negation is unary
        // in the parser.
        let ts = token_types("-10");
        assert_eq!(ts[0], TokenType::Minus);
        assert_eq!(ts[1], TokenType::Int);
    }

    // ── Float literals ────────────────────────────────────────────────────────

    #[test]
    fn float_basic() {
        let (tokens, had_error) = scan("3.14");
        assert!(!had_error);
        assert_eq!(tokens[0].token_type, TokenType::Float);
        assert_eq!(tokens[0].literal, Value::Float(3.14));
    }

    #[test]
    fn float_zero_point_something() {
        let (tokens, _) = scan("0.5");
        assert_eq!(tokens[0].literal, Value::Float(0.5));
    }

    #[test]
    fn trailing_dot_without_digit_is_int_then_dot() {
        // "3." has no digit after '.', so the scanner produces Int(3) + Dot,
        // not a float token.
        let ts = token_types("3.");
        assert_eq!(ts[0], TokenType::Int);
        assert_eq!(ts[1], TokenType::Dot);
    }

    // ── Line tracking ─────────────────────────────────────────────────────────

    #[test]
    fn line_number_starts_at_one() {
        let (tokens, _) = scan("x");
        assert_eq!(tokens[0].line, 1);
    }

    #[test]
    fn newline_increments_line_counter() {
        let (tokens, _) = scan("a\nb\nc");
        assert_eq!(tokens[0].line, 1, "a on line 1");
        assert_eq!(tokens[1].line, 2, "b on line 2");
        assert_eq!(tokens[2].line, 3, "c on line 3");
    }

    #[test]
    fn eof_token_has_correct_line() {
        let (tokens, _) = scan("a\nb");
        let eof = tokens.last().unwrap();
        assert_eq!(eof.token_type, TokenType::EofToken);
        assert_eq!(eof.line, 2);
    }

    #[test]
    fn multiline_string_tracks_line() {
        // A string literal containing a `\n` increments the line counter so
        // the Str token (and any token on the same line as the closing `"`)
        // is reported as line 2.  Use a space after the closing `"` so no
        // additional newline bumps the counter a second time.
        let (tokens, _) = scan("\"a\nb\" x");
        assert_eq!(tokens[0].line, 2, "multiline string token ends on line 2");
        assert_eq!(tokens[1].line, 2, "identifier on same line as closing '\"' is line 2");
    }

    // ── Comments ──────────────────────────────────────────────────────────────

    #[test]
    fn line_comment_skipped_entirely() {
        let ts = token_types("// this is a comment\n42");
        assert_eq!(ts[0], TokenType::Int);
        assert_eq!(ts[1], TokenType::EofToken);
    }

    #[test]
    fn comment_at_end_of_file_skipped() {
        let ts = token_types("// eof comment");
        assert_eq!(ts[0], TokenType::EofToken);
    }

    #[test]
    fn comment_after_code_on_same_line() {
        let ts = token_types("42 // the answer");
        assert_eq!(ts[0], TokenType::Int);
        assert_eq!(ts[1], TokenType::EofToken);
    }

    // ── Whitespace ────────────────────────────────────────────────────────────

    #[test]
    fn whitespace_and_tabs_are_ignored() {
        let ts = token_types("  \t  42  \t  ");
        assert_eq!(ts[0], TokenType::Int);
        assert_eq!(ts[1], TokenType::EofToken);
    }

    // ── Identifiers ───────────────────────────────────────────────────────────

    #[test]
    fn identifier_simple() {
        let (tokens, _) = scan("my_var");
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].lexeme, "my_var");
    }

    #[test]
    fn identifier_with_digits() {
        let (tokens, _) = scan("var123");
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].lexeme, "var123");
    }

    #[test]
    fn identifier_underscore_prefix() {
        let (tokens, _) = scan("_private");
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    // ── Error handling ────────────────────────────────────────────────────────

    #[test]
    fn invalid_char_sets_had_error() {
        let (_, had_error) = scan("$");
        assert!(had_error, "'$' is not a valid token");
    }

    #[test]
    fn invalid_char_does_not_stop_scanning_rest() {
        // The scanner should continue after an invalid char and still emit
        // subsequent valid tokens.
        let (tokens, had_error) = scan("$ 42");
        assert!(had_error);
        assert!(
            tokens.iter().any(|t| t.token_type == TokenType::Int),
            "42 should still be scanned after the invalid char"
        );
    }

    // ── EOF token ─────────────────────────────────────────────────────────────

    #[test]
    fn eof_always_appended() {
        let (tokens, _) = scan("");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::EofToken);
    }

    #[test]
    fn eof_appended_after_tokens() {
        let (tokens, _) = scan("42");
        assert_eq!(tokens.last().unwrap().token_type, TokenType::EofToken);
    }

    // ── Realistic sequences ───────────────────────────────────────────────────

    #[test]
    fn let_declaration_sequence() {
        // `let x: int = 42;` — verifies correct ordering and that `int` is
        // an Identifier (not a keyword).
        let ts = token_types("let x: int = 42;");
        assert_eq!(ts[0], TokenType::Let);
        assert_eq!(ts[1], TokenType::Identifier); // x
        assert_eq!(ts[2], TokenType::Colon);
        assert_eq!(ts[3], TokenType::Identifier); // int  ← NOT a keyword
        assert_eq!(ts[4], TokenType::Equal);
        assert_eq!(ts[5], TokenType::Int);
        assert_eq!(ts[6], TokenType::Semicolon);
        assert_eq!(ts[7], TokenType::EofToken);
    }

    #[test]
    fn function_signature_sequence() {
        let ts = token_types("fn add(a: int, b: int): int");
        assert_eq!(ts[0], TokenType::Fn);
        assert_eq!(ts[1], TokenType::Identifier); // add
        assert_eq!(ts[2], TokenType::LeftParen);
        assert_eq!(ts[3], TokenType::Identifier); // a
        assert_eq!(ts[4], TokenType::Colon);
        assert_eq!(ts[5], TokenType::Identifier); // int
        assert_eq!(ts[6], TokenType::Comma);
    }

    #[test]
    fn comparison_operators_sequence() {
        let ts = token_types("a == b != c <= d >= e");
        assert_eq!(ts[1], TokenType::EqualEqual);
        assert_eq!(ts[3], TokenType::BangEqual);
        assert_eq!(ts[5], TokenType::LessEqual);
        assert_eq!(ts[7], TokenType::GreaterEqual);
    }
}

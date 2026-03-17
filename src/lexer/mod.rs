pub mod token;

use token::{Span, Token, TokenKind};

/// 字句解析エラーの種類
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexErrorKind {
    /// 不正な数値リテラル (値, 基数名)
    InvalidNumber { literal: String, base: &'static str },
    /// プレフィックス後に数字がない
    ExpectedDigitsAfterPrefix { prefix: &'static str },
    /// 未知の文字
    UnexpectedCharacter(char),
}

/// 字句解析エラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

impl LexError {
    /// テスト互換のためのメッセージ文字列
    pub fn message(&self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match &self.kind {
            LexErrorKind::InvalidNumber { literal, base } => {
                format!("invalid {base} number: {literal}")
            }
            LexErrorKind::ExpectedDigitsAfterPrefix { prefix } => {
                format!("expected {prefix} digits after {prefix}")
            }
            LexErrorKind::UnexpectedCharacter(ch) => {
                format!("unexpected character: '{ch}'")
            }
        };
        write!(f, "{}:{}: {}", self.span.line, self.span.column, msg)
    }
}

/// 字句解析器
pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    /// 全トークンを解析して返す
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn current_span(&self) -> Span {
        Span {
            line: self.line,
            column: self.column,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.input.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_comment(&mut self) {
        // -- から行末まで
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        loop {
            self.skip_whitespace();

            // コメントチェック
            if self.peek() == Some('-') && self.peek_next() == Some('-') {
                self.skip_comment();
                continue;
            }

            break;
        }

        let span = self.current_span();

        let ch = match self.peek() {
            Some(ch) => ch,
            None => return Ok(Token::new(TokenKind::Eof, span)),
        };

        // 数値リテラル
        if ch.is_ascii_digit() {
            return self.read_number();
        }

        // 識別子・キーワード
        if ch.is_ascii_alphabetic() || ch == '_' {
            return Ok(self.read_ident());
        }

        // 演算子・区切り文字
        self.read_punct()
    }

    fn read_number(&mut self) -> Result<Token, LexError> {
        let span = self.current_span();

        if self.peek() == Some('0') {
            match self.peek_next() {
                Some('x') | Some('X') => {
                    self.advance(); // '0'
                    self.advance(); // 'x'
                    return self.read_hex_number(span);
                }
                Some('b') | Some('B') => {
                    self.advance(); // '0'
                    self.advance(); // 'b'
                    return self.read_binary_number(span);
                }
                _ => {}
            }
        }

        self.read_decimal_number(span)
    }

    fn read_decimal_number(&mut self, span: Span) -> Result<Token, LexError> {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let value = s.parse::<u64>().map_err(|_| LexError {
            kind: LexErrorKind::InvalidNumber {
                literal: s.clone(),
                base: "decimal",
            },
            span,
        })?;
        Ok(Token::new(TokenKind::IntLiteral(value), span))
    }

    fn read_hex_number(&mut self, span: Span) -> Result<Token, LexError> {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_hexdigit() {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        if s.is_empty() {
            return Err(LexError {
                kind: LexErrorKind::ExpectedDigitsAfterPrefix { prefix: "hex" },
                span,
            });
        }
        let value = u64::from_str_radix(&s, 16).map_err(|_| LexError {
            kind: LexErrorKind::InvalidNumber {
                literal: format!("0x{s}"),
                base: "hex",
            },
            span,
        })?;
        Ok(Token::new(TokenKind::IntLiteral(value), span))
    }

    fn read_binary_number(&mut self, span: Span) -> Result<Token, LexError> {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch == '0' || ch == '1' {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        if s.is_empty() {
            return Err(LexError {
                kind: LexErrorKind::ExpectedDigitsAfterPrefix { prefix: "binary" },
                span,
            });
        }
        let value = u64::from_str_radix(&s, 2).map_err(|_| LexError {
            kind: LexErrorKind::InvalidNumber {
                literal: format!("0b{s}"),
                base: "binary",
            },
            span,
        })?;
        Ok(Token::new(TokenKind::IntLiteral(value), span))
    }

    fn read_ident(&mut self) -> Token {
        let span = self.current_span();
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let kind = match s.as_str() {
            "let" => TokenKind::Let,
            "fn" => TokenKind::Fn,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "loop" => TokenKind::Loop,
            "break" => TokenKind::Break,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "match" => TokenKind::Match,
            "enum" => TokenKind::Enum,
            _ => TokenKind::Ident(s),
        };

        Token::new(kind, span)
    }

    fn read_punct(&mut self) -> Result<Token, LexError> {
        let span = self.current_span();
        let ch = self.advance().unwrap();

        let kind = match ch {
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            ':' => {
                if self.peek() == Some(':') {
                    self.advance();
                    TokenKind::ColonColon
                } else {
                    TokenKind::Colon
                }
            }
            '-' => {
                if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::EqEq
                } else if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::FatArrow
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::NotEq
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    TokenKind::AndAnd
                } else {
                    return Err(LexError {
                        kind: LexErrorKind::UnexpectedCharacter(ch),
                        span,
                    });
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    TokenKind::OrOr
                } else if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::Pipe
                } else {
                    return Err(LexError {
                        kind: LexErrorKind::UnexpectedCharacter(ch),
                        span,
                    });
                }
            }
            _ => {
                return Err(LexError {
                    kind: LexErrorKind::UnexpectedCharacter(ch),
                    span,
                });
            }
        };

        Ok(Token::new(kind, span))
    }
}

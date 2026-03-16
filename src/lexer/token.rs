/// ソースコード中の位置情報
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

/// トークンの種類
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // キーワード
    Let,
    Fn,
    If,
    Else,
    Loop,
    Break,
    Return,
    True,
    False,

    // リテラル
    IntLiteral(u64),
    BinaryLiteral(Vec<u8>),

    // 識別子
    Ident(String),

    // 演算子
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    AndAnd,
    OrOr,
    Bang,
    Eq,

    // 区切り文字
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    Arrow, // ->

    // 特殊
    Eof,
}

/// トークン
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

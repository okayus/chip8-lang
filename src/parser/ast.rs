use crate::lexer::token::Span;

/// 型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    U8,
    Bool,
    Unit,
    Array(Box<Type>, usize),
    Sprite(usize),
}

/// 二項演算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
}

/// 単項演算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// 式
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    IntLiteral(u64),
    BoolLiteral(bool),
    Ident(String),
    BinaryOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
    If {
        cond: Box<Expr>,
        then_block: Box<Expr>,
        else_block: Option<Box<Expr>>,
    },
    Loop {
        body: Box<Expr>,
    },
    Block {
        stmts: Vec<Stmt>,
        /// 末尾の式 (ブロックの値)
        expr: Option<Box<Expr>>,
    },
    ArrayLiteral(Vec<Expr>),
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
    },
}

/// 文
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StmtKind {
    Let { name: String, ty: Type, value: Expr },
    Assign { name: String, value: Expr },
    Expr(Expr),
    Return(Option<Expr>),
    Break,
}

/// 関数の引数
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

/// トップレベル定義
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopLevel {
    FnDef {
        name: String,
        params: Vec<Param>,
        return_type: Type,
        body: Expr,
        span: Span,
    },
    LetDef {
        name: String,
        ty: Type,
        value: Expr,
        span: Span,
    },
}

/// プログラム全体
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub top_levels: Vec<TopLevel>,
}

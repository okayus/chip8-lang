use crate::lexer::token::Span;

/// 型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    U8,
    Bool,
    Unit,
    Array(Box<Type>, usize),
    Sprite(usize),
    UserType(String),
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
    BuiltinCall {
        builtin: BuiltinFunction,
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
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    EnumVariant {
        enum_name: String,
        variant: String,
    },
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr)>,
        base: Option<Box<Expr>>,
    },
    FieldAccess {
        expr: Box<Expr>,
        field: String,
    },
}

/// match 式のアーム
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Expr,
    pub body: Expr,
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

/// struct のフィールド定義
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
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
    EnumDef {
        name: String,
        variants: Vec<String>,
        span: Span,
    },
    StructDef {
        name: String,
        fields: Vec<StructField>,
        span: Span,
    },
}

/// プログラム全体
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub top_levels: Vec<TopLevel>,
}

/// 言語組み込み関数
///
/// CHIP-8 ハードウェアの機能に直接対応する関数群。
/// パーサーが関数名から解決し、codegen が各バリアントに対応する命令を生成する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunction {
    /// clear() - 画面クリア (00E0)
    Clear,
    /// draw(sprite, x, y) -> bool - スプライト描画 (DXYN)
    Draw,
    /// wait_key() -> u8 - キー入力待ち (FX0A)
    WaitKey,
    /// is_key_pressed(k: u8) -> bool - キー押下判定 (EX9E)
    IsKeyPressed,
    /// delay() -> u8 - ディレイタイマー読み取り (FX07)
    Delay,
    /// set_delay(v: u8) - ディレイタイマー設定 (FX15)
    SetDelay,
    /// set_sound(v: u8) - サウンドタイマー設定 (FX18)
    SetSound,
    /// random(mask: u8) -> u8 - 乱数生成 (CXKK)
    Random,
    /// bcd(v: u8) - BCD 変換 (FX33)
    Bcd,
    /// draw_digit(v: u8, x: u8, y: u8) - フォント数字描画 (FX29 + DXYN)
    DrawDigit,
    /// random_enum(EnumName) -> EnumName - enum のランダム生成
    RandomEnum,
}

impl BuiltinFunction {
    /// 関数名から組み込み関数を解決する
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "clear" => Some(Self::Clear),
            "draw" => Some(Self::Draw),
            "wait_key" => Some(Self::WaitKey),
            "is_key_pressed" => Some(Self::IsKeyPressed),
            "delay" => Some(Self::Delay),
            "set_delay" => Some(Self::SetDelay),
            "set_sound" => Some(Self::SetSound),
            "random" => Some(Self::Random),
            "bcd" => Some(Self::Bcd),
            "draw_digit" => Some(Self::DrawDigit),
            "random_enum" => Some(Self::RandomEnum),
            _ => None,
        }
    }

    /// 組み込み関数の名前
    pub fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Draw => "draw",
            Self::WaitKey => "wait_key",
            Self::IsKeyPressed => "is_key_pressed",
            Self::Delay => "delay",
            Self::SetDelay => "set_delay",
            Self::SetSound => "set_sound",
            Self::Random => "random",
            Self::Bcd => "bcd",
            Self::DrawDigit => "draw_digit",
            Self::RandomEnum => "random_enum",
        }
    }

    /// この組み込み関数のシグネチャ (引数型リスト, 戻り値型)
    pub fn signature(self) -> (Vec<Type>, Type) {
        match self {
            Self::Clear => (vec![], Type::Unit),
            Self::Draw => (vec![Type::Sprite(0), Type::U8, Type::U8], Type::Bool),
            Self::WaitKey => (vec![], Type::U8),
            Self::IsKeyPressed => (vec![Type::U8], Type::Bool),
            Self::Delay => (vec![], Type::U8),
            Self::SetDelay => (vec![Type::U8], Type::Unit),
            Self::SetSound => (vec![Type::U8], Type::Unit),
            Self::Random => (vec![Type::U8], Type::U8),
            Self::Bcd => (vec![Type::U8], Type::Unit),
            Self::DrawDigit => (vec![Type::U8, Type::U8, Type::U8], Type::Unit),
            // RandomEnum は analyzer で特殊処理されるためプレースホルダ
            Self::RandomEnum => (vec![Type::U8], Type::U8),
        }
    }
}
